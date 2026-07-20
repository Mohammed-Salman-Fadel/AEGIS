use anyhow::Context;
use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::inference::{InferenceBackend, InferenceResponse, InferenceUsage};

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
    prompt_eval_count: Option<usize>,
    eval_count: Option<usize>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

#[derive(Deserialize)]
struct OllamaPsResponse {
    models: Vec<OllamaRunningModel>,
}

#[derive(Deserialize)]
struct OllamaRunningModel {
    name: String,
    model: String,
    context_length: Option<usize>,
}

#[derive(Serialize)]
struct ShowRequest<'a> {
    model: &'a str,
}

#[derive(Deserialize)]
struct ShowResponse {
    parameters: Option<String>,
    model_info: Option<HashMap<String, serde_json::Value>>,
}

fn same_model(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
        || format!("{left}:latest").eq_ignore_ascii_case(right)
        || left.eq_ignore_ascii_case(&format!("{right}:latest"))
}

fn parse_num_ctx(parameters: &str) -> Option<usize> {
    parameters.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("num_ctx"), Some(value)) => value.parse::<usize>().ok(),
            _ => None,
        }
    })
}

fn context_length_from_model_info(
    model_info: &HashMap<String, serde_json::Value>,
) -> Option<usize> {
    model_info
        .iter()
        .filter_map(|(key, value)| {
            if key == "context_length" || key.ends_with(".context_length") {
                value
                    .as_u64()
                    .and_then(|number| usize::try_from(number).ok())
            } else {
                None
            }
        })
        .max()
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama tags endpoint at {}/api/tags. Make sure Ollama is running, or switch AEGIS to LM Studio/OpenAI-compatible in Settings.",
                    self.base_url
                )
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama tags error {status}: {body}");
        }

        let response = response.json::<OllamaTagsResponse>().await?;
        Ok(response.models.into_iter().map(|m| m.name).collect())
    }

    async fn call(&self, prompt: &str, model: &str) -> anyhow::Result<String> {
        Ok(self.call_with_usage(prompt, model).await?.text)
    }

    async fn call_with_usage(
        &self,
        prompt: &str,
        model: &str,
    ) -> anyhow::Result<InferenceResponse> {
        let response = self
            .generate_request(model, prompt, false, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama generate endpoint at {}/api/generate for model `{model}`. Make sure Ollama is running, or switch AEGIS to LM Studio/OpenAI-compatible in Settings.",
                    self.base_url
                )
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama error {status} for model `{model}`: {body}");
        }

        let response = response.json::<GenerateChunk>().await?;

        if let Some(error) = response.error {
            anyhow::bail!("ollama error: {error}");
        }

        Ok(InferenceResponse {
            text: response.response.unwrap_or_default(),
            usage: InferenceUsage {
                prompt_tokens: response.prompt_eval_count,
                completion_tokens: response.eval_count,
            },
        })
    }

    async fn stream(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        Ok(self.stream_with_usage(prompt, model, tx).await?.text)
    }

    async fn stream_with_usage(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<InferenceResponse> {
        let response = self
            .generate_request(model, prompt, true, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama streaming endpoint at {}/api/generate for model `{model}`. Make sure Ollama is running, or switch AEGIS to LM Studio/OpenAI-compatible in Settings.",
                    self.base_url
                )
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ollama error {status} for model `{model}`: {body}");
        }

        let mut full_response = String::new();
        let mut pending = String::new();
        let mut stream = response.bytes_stream();
        let mut usage = InferenceUsage::default();

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
                        return Ok(InferenceResponse {
                            text: full_response,
                            usage,
                        });
                    }
                }

                if parsed.done.unwrap_or(false) {
                    usage.prompt_tokens = parsed.prompt_eval_count;
                    usage.completion_tokens = parsed.eval_count;
                    return Ok(InferenceResponse {
                        text: full_response,
                        usage,
                    });
                }
            }
        }

        Ok(InferenceResponse {
            text: full_response,
            usage,
        })
    }

    async fn context_window(&self, model: &str) -> anyhow::Result<Option<usize>> {
        let running_response = self
            .client
            .get(format!("{}/api/ps", self.base_url))
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama process endpoint at {}/api/ps while checking `{model}`",
                    self.base_url
                )
            })?;
        if running_response.status().is_success() {
            let running = running_response.json::<OllamaPsResponse>().await?;
            if let Some(context_length) = running.models.into_iter().find_map(|running_model| {
                if same_model(&running_model.name, model) || same_model(&running_model.model, model)
                {
                    running_model.context_length
                } else {
                    None
                }
            }) {
                return Ok(Some(context_length));
            }
        }

        let show_response = self
            .client
            .post(format!("{}/api/show", self.base_url))
            .json(&ShowRequest { model })
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama show endpoint at {}/api/show for model `{model}`",
                    self.base_url
                )
            })?;

        let status = show_response.status();
        if !status.is_success() {
            let body = show_response.text().await.unwrap_or_default();
            anyhow::bail!("ollama show error {status} for model `{model}`: {body}");
        }

        let details = show_response.json::<ShowResponse>().await?;
        if let Some(parameters) = details.parameters.as_deref() {
            if let Some(num_ctx) = parse_num_ctx(parameters) {
                return Ok(Some(num_ctx));
            }
        }

        Ok(details
            .model_info
            .as_ref()
            .and_then(context_length_from_model_info))
    }

    async fn warm_model(&self, model: &str) -> anyhow::Result<()> {
        let response = self
            .generate_request(model, "", false, Some(KEEP_ALIVE_FOREVER))
            .send()
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama warmup endpoint at {}/api/generate for model `{model}`. Make sure Ollama is running before warming this model.",
                    self.base_url
                )
            })?;

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
            .await
            .with_context(|| {
                format!(
                    "could not reach Ollama unload endpoint at {}/api/generate for model `{model}`",
                    self.base_url
                )
            })?;

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
