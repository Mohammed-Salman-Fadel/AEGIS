use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::inference::InferenceBackend;

pub struct OllamaBackend {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateChunk {
    response: Option<String>,
    done: Option<bool>,
    error: Option<String>,
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    async fn call(&self, prompt: &str, _model: &str) -> anyhow::Result<String> {
        // Keep the current behavior of forcing a single local model for now,
        // but align it with the model that is actually installed on this machine.
        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&GenerateRequest {
                model: "qwen3:4b",
                prompt,
                stream: false,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<GenerateChunk>()
            .await?;

        if let Some(error) = response.error {
            anyhow::bail!("ollama error: {error}");
        }

        Ok(response.response.unwrap_or_default())
    }

    async fn stream(
        &self,
        prompt: &str,
        _model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        // Keep streaming and non-streaming calls on the same default local model.
        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&GenerateRequest {
                model: "qwen3:4b",
                prompt,
                stream: true,
            })
            .send()
            .await?
            .error_for_status()?;

        let mut full_response = String::new();
        let mut pending = String::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            pending.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_index) = pending.find('\n') {
                let line = pending[..newline_index].trim().to_string();
                pending = pending[newline_index + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                let parsed: GenerateChunk = serde_json::from_str(&line)?;
                if let Some(error) = parsed.error {
                    anyhow::bail!("ollama error: {error}");
                }

                if let Some(token) = parsed.response {
                    full_response.push_str(&token);
                    if tx.send(token).await.is_err() {
                        return Ok(full_response);
                    }
                }

                if parsed.done.unwrap_or(false) {
                    return Ok(full_response);
                }
            }
        }

        Ok(full_response)
    }
}
