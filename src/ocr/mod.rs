use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct GenerateRequest {
    model: &'static str,
    prompt: &'static str,
    stream: bool,
    images: Vec<String>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

pub async fn extract_time(bytes: &[u8]) -> Result<Duration> {
    let b64 = general_purpose::STANDARD.encode(bytes);

    let req_body = GenerateRequest {
        model: "gemma3:4b",
        prompt: include_str!("prompt.txt"),
        stream: false,
        images: vec![b64],
    };

    let client = Client::new();
    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request to Ollama")?
        .error_for_status()
        .context("Ollama returned an error status")?
        .json::<GenerateResponse>()
        .await
        .context("Failed to parse Ollama JSON response")?;

    parse_duration(resp.response.trim())
}

static TIME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d+):(\d{2})\.(\d{3})$").unwrap());

pub fn parse_duration(s: &str) -> Result<Duration> {
    let caps = TIME_RE
        .captures(s)
        .ok_or_else(|| anyhow!("Invalid input format: '{}'", s))?;

    let minutes = caps[1]
        .parse::<u64>()
        .map_err(|e| anyhow!("Invalid minutes '{}': {}", &caps[1], e))?;

    let seconds = caps[2]
        .parse::<u64>()
        .map_err(|e| anyhow!("Invalid seconds '{}': {}", &caps[2], e))?;

    if seconds > 59 {
        return Err(anyhow!(
            "Invalid seconds '{}': must be between 00 and 59",
            &caps[2]
        ));
    }

    let millis = caps[3]
        .parse::<u64>()
        .map_err(|e| anyhow!("Invalid milliseconds '{}': {}", &caps[3], e))?;

    Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
}
