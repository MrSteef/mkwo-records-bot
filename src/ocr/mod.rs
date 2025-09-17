use base64::Engine as _;
use image::ImageEncoder;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};
use thiserror::Error;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType as PngCompression, FilterType as PngFilter, PngEncoder};
use image::{
    DynamicImage, ExtendedColorType, GenericImageView, imageops::FilterType as ResizeFilter,
};

pub type Result<T> = std::result::Result<T, ExtractError>;

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("provider {0} non-OK status: {1}")]
    ProviderStatus(&'static str, StatusCode),

    #[error("provider {0} decode: {1}")]
    ProviderDecode(&'static str, String),

    #[error("rate limited by provider {0}")]
    RateLimited(&'static str),

    #[error("no yellow time found")]
    YellowMissing,

    #[error("invalid time format: {0}")]
    InvalidFormat(String),

    #[error("minutes parse: {0}")]
    MinutesParse(String),

    #[error("seconds parse: {0}")]
    SecondsParse(String),

    #[error("milliseconds parse: {0}")]
    MillisParse(String),

    #[error("no providers configured or available")]
    NoProviders,

    // NEW: image pipeline errors
    #[error("image decode: {0}")]
    ImageDecode(String),

    #[error("image encode: {0}")]
    ImageEncode(String),

    #[error("image size still too large after downscaling")]
    ImageTooLarge,
}

#[derive(Serialize)]
struct OAChatRequest<'a> {
    model: &'a str,
    messages: Vec<OAMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<&'a str>>,
}

#[derive(Serialize)]
struct OAMessage<'a> {
    role: &'a str,
    content: Vec<OAContent<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OAContent<'a> {
    Text {
        text: &'a str,
    },
    #[serde(rename_all = "snake_case")]
    ImageUrl {
        image_url: ImageUrl<'a>,
    },
}

#[derive(Serialize)]
struct ImageUrl<'a> {
    url: &'a str,
}

#[derive(Deserialize)]
struct OAChatResponse {
    choices: Vec<OAChoice>,
}

#[derive(Deserialize)]
struct OAChoice {
    message: OAMessageResp,
}

#[derive(Deserialize)]
struct OAMessageResp {
    content: String,
}

static TIME_STRICT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d):([0-5]\d)\.(\d{3})$").unwrap());

static TIME_FINDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)\b(\d):([0-5]\d)\.(\d{3})\b").unwrap());

pub async fn extract_time(image_bytes: &[u8]) -> Result<Duration> {
    extract_time_with_model("llama-4-vision", image_bytes).await
}

/// Main entry with provider failover (OpenRouter -> Groq by default),
/// now with image downscaling & JPEG recompression to respect provider limits.
pub async fn extract_time_with_model(model: &str, image_bytes: &[u8]) -> Result<Duration> {
    let providers = read_provider_order();
    if providers.is_empty() {
        return Err(ExtractError::NoProviders);
    }

    // Downscale + recompress and wrap as data URL.
    let image_data_url = prepare_image_data_url(image_bytes)?;

    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    let user_text = include_str!("prompt.txt");

    let mut last_err: Option<ExtractError> = None;
    for p in providers {
        match p {
            Provider::OpenRouter => {
                match call_openrouter(&client, model, &image_data_url, user_text).await {
                    Ok(text) => return post_process_to_duration(&text),
                    Err(e) => {
                        let retryable = matches!(
                            e,
                            ExtractError::RateLimited(_)
                                | ExtractError::Http(_)
                                | ExtractError::ProviderStatus(_, StatusCode::TOO_MANY_REQUESTS)
                                | ExtractError::ProviderStatus(_, StatusCode::BAD_GATEWAY)
                                | ExtractError::ProviderStatus(_, StatusCode::SERVICE_UNAVAILABLE)
                                | ExtractError::ProviderStatus(_, StatusCode::GATEWAY_TIMEOUT)
                                | ExtractError::ProviderStatus(
                                    _,
                                    StatusCode::INTERNAL_SERVER_ERROR
                                )
                        );
                        last_err = Some(e);
                        if retryable {
                            continue;
                        } else {
                            break;
                        }
                    }
                }
            }
            Provider::Groq => match call_groq(&client, model, &image_data_url, user_text).await {
                Ok(text) => return post_process_to_duration(&text),
                Err(e) => {
                    let retryable = matches!(
                        e,
                        ExtractError::RateLimited(_)
                            | ExtractError::Http(_)
                            | ExtractError::ProviderStatus(_, StatusCode::TOO_MANY_REQUESTS)
                            | ExtractError::ProviderStatus(_, StatusCode::BAD_GATEWAY)
                            | ExtractError::ProviderStatus(_, StatusCode::SERVICE_UNAVAILABLE)
                            | ExtractError::ProviderStatus(_, StatusCode::GATEWAY_TIMEOUT)
                            | ExtractError::ProviderStatus(_, StatusCode::INTERNAL_SERVER_ERROR)
                    );
                    last_err = Some(e);
                    if retryable {
                        continue;
                    } else {
                        break;
                    }
                }
            },
        }
    }

    Err(last_err.unwrap_or(ExtractError::NoProviders))
}

/* ---------- Provider plumbing ---------- */

#[derive(Copy, Clone)]
enum Provider {
    OpenRouter,
    Groq,
}

fn read_provider_order() -> Vec<Provider> {
    let default = "openrouter,groq".to_string();
    let raw = env::var("PROVIDER_ORDER").unwrap_or(default);

    raw.split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter_map(|s| match s.as_str() {
            "openrouter" => Some(Provider::OpenRouter),
            "groq" => Some(Provider::Groq),
            _ => None,
        })
        .collect()
}

fn build_payload<'a>(model: &'a str, data_url: &'a str, user_text: &'a str) -> OAChatRequest<'a> {
    OAChatRequest {
        model,
        messages: vec![
            OAMessage {
                role: "system",
                content: vec![OAContent::Text {
                    text: "You are a precise OCR assistant. Extract the yellow timer in m:ss.mmm.",
                }],
            },
            OAMessage {
                role: "user",
                content: vec![
                    OAContent::Text { text: user_text },
                    OAContent::ImageUrl {
                        image_url: ImageUrl { url: data_url },
                    },
                ],
            },
        ],
        max_tokens: Some(16),
        temperature: Some(0.0),
        top_p: Some(0.1),
        stop: Some(vec!["\n"]),
    }
}

/* ----- OpenRouter ----- */

async fn call_openrouter(
    client: &Client,
    model_arg_fallback: &str,
    image_data_url: &str,
    user_text: &str,
) -> Result<String> {
    let base = env::var("OPENROUTER_BASE_URL")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        ExtractError::ProviderDecode("openrouter", "missing OPENROUTER_API_KEY".into())
    })?;
    let model = env::var("OPENROUTER_MODEL").unwrap_or_else(|_| model_arg_fallback.to_string());

    let url = format!("{}/chat/completions", base);
    let payload = build_payload(&model, image_data_url, user_text);

    let mut req = client.post(&url).bearer_auth(api_key).json(&payload);

    if let Ok(referer) = env::var("OPENROUTER_REFERRER") {
        req = req.header("HTTP-Referer", referer);
    }
    if let Ok(title) = env::var("OPENROUTER_TITLE") {
        req = req.header("X-Title", title);
    }

    let resp = req.send().await?;

    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ExtractError::RateLimited("openrouter"));
    }
    if !resp.status().is_success() {
        return Err(ExtractError::ProviderStatus("openrouter", resp.status()));
    }

    let parsed: OAChatResponse = resp
        .json()
        .await
        .map_err(|e| ExtractError::ProviderDecode("openrouter", e.to_string()))?;

    let text = parsed
        .choices
        .get(0)
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    Ok(text)
}

/* ----- Groq ----- */

async fn call_groq(
    client: &Client,
    model_arg_fallback: &str,
    image_data_url: &str,
    user_text: &str,
) -> Result<String> {
    let base =
        env::var("GROQ_BASE_URL").unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());
    let api_key = env::var("GROQ_API_KEY")
        .map_err(|_| ExtractError::ProviderDecode("groq", "missing GROQ_API_KEY".into()))?;
    let model = env::var("GROQ_MODEL").unwrap_or_else(|_| model_arg_fallback.to_string());

    let url = format!("{}/chat/completions", base);
    let payload = build_payload(&model, image_data_url, user_text);

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await?;

    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(ExtractError::RateLimited("groq"));
    }
    if !resp.status().is_success() {
        return Err(ExtractError::ProviderStatus("groq", resp.status()));
    }

    let parsed: OAChatResponse = resp
        .json()
        .await
        .map_err(|e| ExtractError::ProviderDecode("groq", e.to_string()))?;

    let text = parsed
        .choices
        .get(0)
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    Ok(text)
}

/* ---------- Image downscale + data URL ---------- */

/// Convert arbitrary input bytes into a downscaled data URL (PNG or JPEG),
/// choosing the smallest that still looks good and stays under ~3.9 MB base64.
fn prepare_image_data_url(bytes: &[u8]) -> Result<String> {
    let mut img =
        image::load_from_memory(bytes).map_err(|e| ExtractError::ImageDecode(e.to_string()))?;

    // Prefer PNG if the source has alpha (transparency).
    let mut prefer_png = has_alpha(&img);

    // Initial downscale cap (long side). Timers/overlays don't need UHD.
    const INITIAL_MAX_SIDE: u32 = 1280;
    img = resize_long_side(img, INITIAL_MAX_SIDE);

    // Iteratively recompress until base64 ≤ ~3.9 MB (safe under Groq base64 limit)
    const SAFE_BASE64_MAX: usize = 3_900_000;
    const MIN_SIDE: u32 = 512;

    let mut side_cap = INITIAL_MAX_SIDE;
    let mut current = img;
    let mut jpeg_quality = 85u8;

    for _ in 0..10 {
        // Try preferred format first, then the other, pick the smaller that fits
        let mut candidates: Vec<(String, usize)> = Vec::new();

        // Encode PNG (good for transparency / UI text)
        if prefer_png {
            if let Ok(png) = encode_png(&current) {
                let b64_len = estimate_base64_len(png.len());
                if b64_len <= SAFE_BASE64_MAX {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
                    return Ok(format!("data:image/png;base64,{}", b64));
                }
                candidates.push(("png".into(), b64_len));
            }
        }

        // Encode JPEG at current quality (good for photos; often smaller)
        if let Ok(jpg) = encode_jpeg(&current, jpeg_quality) {
            let b64_len = estimate_base64_len(jpg.len());
            if b64_len <= SAFE_BASE64_MAX {
                let b64 = base64::engine::general_purpose::STANDARD.encode(jpg);
                return Ok(format!("data:image/jpeg;base64,{}", b64));
            }
            candidates.push(("jpeg".into(), b64_len));
        }

        // If PNG wasn’t preferred initially, try it as an alternative and see if it’s smaller.
        if !prefer_png {
            if let Ok(png) = encode_png(&current) {
                let b64_len = estimate_base64_len(png.len());
                if b64_len <= SAFE_BASE64_MAX {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
                    return Ok(format!("data:image/png;base64,{}", b64));
                }
                candidates.push(("png".into(), b64_len));
            }
        }

        // Neither fit: adjust strategy
        // If JPEG path is in play and quality > 55, reduce quality first (smaller size, minimal loss)
        if jpeg_quality > 55 {
            jpeg_quality = jpeg_quality.saturating_sub(10);
            continue;
        }

        // Otherwise, downscale dimensions by ~15%
        side_cap = ((side_cap as f32) * 0.85) as u32;
        if side_cap < MIN_SIDE {
            break;
        }
        current = resize_long_side(current, side_cap);

        // If PNG kept being closer to the target than JPEG, prefer it next iterations.
        if !candidates.is_empty() {
            // prefer the one with smaller base64 estimate on last attempt
            candidates.sort_by_key(|(_, len)| *len);
            prefer_png = candidates
                .first()
                .map(|(fmt, _)| fmt == "png")
                .unwrap_or(prefer_png);
        }
    }

    Err(ExtractError::ImageTooLarge)
}

fn resize_long_side(img: DynamicImage, max_side: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let long = w.max(h);
    if long <= max_side {
        return img;
    }
    let scale = max_side as f32 / long as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    img.resize(nw, nh, ResizeFilter::Lanczos3)
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    let mut buf = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
    enc.encode(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
        .map_err(|e| ExtractError::ImageEncode(e.to_string()))?;
    Ok(buf)
}

fn encode_png(img: &DynamicImage) -> Result<Vec<u8>> {
    // If image has alpha, keep it; otherwise PNG still may win for flat UI.
    let has_alpha = has_alpha(img);
    let (w, h) = img.dimensions();

    let mut buf = Vec::new();
    let mut enc = PngEncoder::new_with_quality(&mut buf, PngCompression::Best, PngFilter::Adaptive);

    if has_alpha {
        let rgba = img.to_rgba8();
        enc.write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
            .map_err(|e| ExtractError::ImageEncode(e.to_string()))?;
    } else {
        let rgb = img.to_rgb8();
        enc.write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| ExtractError::ImageEncode(e.to_string()))?;
    }

    Ok(buf)
}

fn has_alpha(img: &DynamicImage) -> bool {
    // DynamicImage::color() returns ExtendedColorType; use its alpha property
    img.color().has_alpha()
}

#[inline]
fn estimate_base64_len(raw_bytes: usize) -> usize {
    // base64 expands by ~4/3; round up to nearest 4
    ((raw_bytes + 2) / 3) * 4
}

/* ---------- Post-processing (unchanged parsing) ---------- */

fn post_process_to_duration(text: &str) -> Result<Duration> {
    let text = text.trim();

    if text.eq_ignore_ascii_case("null") {
        return Err(ExtractError::YellowMissing);
    }

    if TIME_STRICT_RE.is_match(text) {
        return parse_duration(text);
    }

    if let Some(m) = TIME_FINDER_RE.find(text) {
        return parse_duration(m.as_str());
    }

    Err(ExtractError::YellowMissing)
}

pub fn parse_duration(s: &str) -> Result<Duration> {
    let caps = TIME_STRICT_RE
        .captures(s)
        .ok_or_else(|| ExtractError::InvalidFormat(s.to_string()))?;

    let minutes = caps[1]
        .parse::<u64>()
        .map_err(|e| ExtractError::MinutesParse(e.to_string()))?;

    let seconds = caps[2]
        .parse::<u64>()
        .map_err(|e| ExtractError::SecondsParse(e.to_string()))?;
    if seconds > 59 {
        return Err(ExtractError::InvalidFormat(caps[2].to_string()));
    }

    let millis = caps[3]
        .parse::<u64>()
        .map_err(|e| ExtractError::MillisParse(e.to_string()))?;

    Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
}
