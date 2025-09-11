use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ExtractError>;

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ollama non-OK status: {0}")]
    OllamaStatus(reqwest::StatusCode),
    #[error("ollama response decode: {0}")]
    OllamaDecode(String),
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
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    images: Vec<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions<'a>>,
}

#[derive(Serialize, Default)]
struct OllamaOptions<'a> {
    temperature: f32,
    top_p: f32,
    top_k: i32,
    num_ctx: i32,
    num_predict: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<&'a str>>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

static TIME_STRICT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d):([0-5]\d)\.(\d{3})$").unwrap());

static TIME_FINDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)\b(\d):([0-5]\d)\.(\d{3})\b").unwrap());

pub async fn extract_time(image_bytes: &[u8]) -> Result<Duration> {
    extract_time_with_model("qwen2.5vl:7b", image_bytes).await
}

pub async fn extract_time_with_model(model: &str, image_bytes: &[u8]) -> Result<Duration> {
    let b64 = general_purpose::STANDARD.encode(image_bytes);
    let client = Client::new();

    let req_body = GenerateRequest {
        model,
        prompt: include_str!("prompt.txt"),
        images: vec![b64],
        stream: false,
        options: Some(OllamaOptions {
            temperature: 0.0,
            top_p: 0.1,
            top_k: 20,
            num_ctx: 512,
            num_predict: 16,
            stop: Some(vec!["\n"]),
        }),
    };

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&req_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(ExtractError::OllamaStatus(resp.status()));
    }

    let raw: GenerateResponse = resp
        .json()
        .await
        .map_err(|e| ExtractError::OllamaDecode(e.to_string()))?;

    let text = raw.response.trim();

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
