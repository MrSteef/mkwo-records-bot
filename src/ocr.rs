use anyhow::{Context, Result};
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
pub async fn run_pipeline_from_bytes(bytes: &[u8], _debug: bool, _msg_id: u64) -> Result<String> {
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

    Ok(resp.response.trim().to_string())
}
