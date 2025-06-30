use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    images: Vec<String>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    // …other fields if you need them
}

/// Sends the image bytes to Ollama’s `/api/generate` endpoint and returns the model’s reply.
pub async fn run_pipeline_from_bytes(bytes: &[u8], _debug: bool, _msg_id: u64) -> Result<Duration> {
    // 1) Base64-encode the image
    let b64 = general_purpose::STANDARD.encode(bytes);

    // 2) Build the JSON payload
    let req_body = GenerateRequest {
        model: "gemma3:4b".into(),
        prompt: "The attached image is the result of a Time Trial. \
            The driver's time is the one in the yellow box. \
            Please tell me what this time is. \
            It is formatted as '0:00.000' (m:ss.ms). \
            Your response should match this exact format, and may not include any other text."
            .into(),
        stream: false,
        images: vec![b64],
    };

    // 3) Send the POST request
    let client = Client::new();
    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&req_body)
        .send()
        .await
        .context("failed to send request to Ollama")?
        .error_for_status()
        .context("Ollama returned error status")?
        .json::<GenerateResponse>()
        .await
        .context("failed to parse Ollama JSON response")?;

    parse_duration(resp.response.trim())
}

pub fn parse_duration(s: &str) -> Result<Duration> {
    // split into ["M", "SS.mmm"]
    let mut parts = s.split(':');
    let minutes_str = parts
        .next()
        .ok_or_else(|| anyhow!("Missing minutes in '{}'", s))?;
    let sec_ms_str = parts
        .next()
        .ok_or_else(|| anyhow!("Missing seconds in '{}'", s))?;
    // there should be no extra ':'
    if parts.next().is_some() {
        return Err(anyhow!("Unexpected extra ':' in '{}'", s));
    }

    // parse minutes
    let minutes: u64 = minutes_str
        .parse()
        .map_err(|e| anyhow!("Invalid minutes '{}' in '{}': {}", minutes_str, s, e))?;

    // split seconds and milliseconds
    let mut sec_parts = sec_ms_str.split('.');
    let seconds_str = sec_parts
        .next()
        .ok_or_else(|| anyhow!("Missing seconds before '.' in '{}'", s))?;
    let millis_str = sec_parts
        .next()
        .ok_or_else(|| anyhow!("Missing milliseconds after '.' in '{}'", s))?;
    // no extra '.'
    if sec_parts.next().is_some() {
        return Err(anyhow!("Unexpected extra '.' in seconds part of '{}'", s));
    }

    let seconds: u64 = seconds_str
        .parse()
        .map_err(|e| anyhow!("Invalid seconds '{}' in '{}': {}", seconds_str, s, e))?;
    let millis: u64 = match millis_str.len() {
        1..=3 => {
            // e.g. "5" -> "500", "45" -> "450", "123" -> "123"
            let scale = 10_u64.pow(3 - millis_str.len() as u32);
            let raw: u64 = millis_str
                .parse()
                .map_err(|e| anyhow!("Invalid millis '{}' in '{}': {}", millis_str, s, e))?;
            raw * scale
        }
        _ => return Err(anyhow!("Milliseconds must be 1–3 digits in '{}'", s)),
    };

    Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
}
