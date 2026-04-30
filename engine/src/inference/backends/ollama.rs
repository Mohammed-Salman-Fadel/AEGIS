use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::inference::InferenceBackend;

const KEEP_ALIVE_FOREVER: i64 = -1;
const KEEP_ALIVE_UNLOAD: i64 = 0;

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

    fn generate_request<'a>(
        &'a self,
        model: &'a str,
        prompt: &'a str,
        stream: bool,
        keep_alive: Option<i64>,
    ) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&GenerateRequest {
                model,
                prompt,
                stream,
                keep_alive,
            })
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<i64>,
}

#[derive(Deserialize)]
struct GenerateChunk {
    response: Option<String>,
    done: Option<bool>,
    error: Option<String>,
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    async fn call(&self, prompt: &str, model: &str) -> anyhow::Result<String> {
        let response = self
            .generate_request(model, prompt, false, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama error {status} for model `{model}`: {body}");
        }

        let response = response.json::<GenerateChunk>().await?;

        if let Some(error) = response.error {
            anyhow::bail!("ollama error: {error}");
        }

        Ok(response.response.unwrap_or_default())
    }

    async fn stream(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let response = self
            .generate_request(model, prompt, true, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama error {status} for model `{model}`: {body}");
        }

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

    async fn warm_model(&self, model: &str) -> anyhow::Result<()> {
        let response = self
            .generate_request(model, "", false, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama warmup error {status} for model `{model}`: {body}");
        }

        let response = response.json::<GenerateChunk>().await?;
        if let Some(error) = response.error {
            anyhow::bail!("ollama warmup error for model `{model}`: {error}");
        }

        Ok(())
    }

    async fn unload_model(&self, model: &str) -> anyhow::Result<()> {
        let response = self
            .generate_request(model, "", false, Some(KEEP_ALIVE_UNLOAD))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama unload error {status} for model `{model}`: {body}");
        }

        let response = response.json::<GenerateChunk>().await?;
        if let Some(error) = response.error {
            anyhow::bail!("ollama unload error for model `{model}`: {error}");
        }

        Ok(())
    }
}
