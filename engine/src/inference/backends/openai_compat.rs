use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::inference::InferenceBackend;

pub struct OpenAiCompatBackend {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatBackend {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn chat_completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn request_builder(&self) -> reqwest::RequestBuilder {
        self.request_builder_for(self.chat_completions_url())
    }

    fn request_builder_for(&self, url: String) -> reqwest::RequestBuilder {
        let builder = self
            .client
            .post(url)
            .header(CONTENT_TYPE, "application/json");

        match &self.api_key {
            Some(api_key) => builder.header(AUTHORIZATION, format!("Bearer {api_key}")),
            None => builder,
        }
    }

    fn models_urls(&self) -> [String; 3] {
        [
            format!("{}/v1/models", self.base_url),
            format!("{}/api/v0/models", self.base_url),
            format!("{}/api/v1/models", self.base_url),
        ]
    }

    fn model_management_url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    async fn post_model_management<T: Serialize + ?Sized>(
        &self,
        paths: &[&str],
        payload: &T,
        error_prefix: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let mut last_error: Option<anyhow::Error> = None;

        for path in paths {
            let response = self
                .request_builder_for(self.model_management_url(path))
                .json(payload)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(error.into());
                    continue;
                }
            };

            if response.status().is_success() {
                return Ok(());
            }

            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 404 || status.as_u16() == 405 {
                last_error = Some(anyhow::anyhow!(
                    "{error_prefix} endpoint `{path}` was not available for model `{model}`: {body}"
                ));
                continue;
            }

            anyhow::bail!("{error_prefix} error {status} for model `{model}`: {body}");
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("{error_prefix} failed for model `{model}`.")))
    }

    async fn post_model_management_variants(
        &self,
        paths: &[&str],
        model: &str,
        error_prefix: &str,
    ) -> anyhow::Result<()> {
        let payload_variants = [
            serde_json::json!({ "model": model }),
            serde_json::json!({ "name": model }),
            serde_json::json!({ "id": model }),
            serde_json::json!({ "identifier": model }),
        ];

        let mut last_error: Option<anyhow::Error> = None;
        for payload in payload_variants {
            match self
                .post_model_management(paths, &payload, error_prefix, model)
                .await
            {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("{error_prefix} failed for model `{model}`.")))
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
    error: Option<OpenAiCompatError>,
}

#[derive(Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Deserialize)]
struct ChatCompletionMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChatCompletionChunkChoice>,
    error: Option<OpenAiCompatError>,
}

#[derive(Deserialize)]
struct ChatCompletionChunkChoice {
    delta: ChatCompletionDelta,
}

#[derive(Deserialize)]
struct ChatCompletionDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiCompatError {
    message: String,
}

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelData>,
}

#[derive(Deserialize)]
struct ModelData {
    id: String,
}

#[async_trait]
impl InferenceBackend for OpenAiCompatBackend {
    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let mut last_error: Option<anyhow::Error> = None;

        for url in self.models_urls() {
            let mut builder = self
                .client
                .get(&url)
                .header(CONTENT_TYPE, "application/json")
                .timeout(Duration::from_secs(5));

            if let Some(api_key) = &self.api_key {
                builder = builder.header(AUTHORIZATION, format!("Bearer {api_key}"));
            }

            let response = match builder.send().await {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(error.into());
                    continue;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                last_error = Some(anyhow::anyhow!(
                    "openai-compatible models error {status}: {body}"
                ));
                continue;
            }

            match response.json::<ModelListResponse>().await {
                Ok(response) => return Ok(response.data.into_iter().map(|m| m.id).collect()),
                Err(error) => last_error = Some(error.into()),
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("provider returned no model list")))
    }

    async fn call(&self, prompt: &str, model: &str) -> anyhow::Result<String> {
        let response = self
            .request_builder()
            .json(&ChatCompletionRequest {
                model,
                messages: vec![ChatMessage {
                    role: "user",
                    content: prompt,
                }],
                stream: false,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<ChatCompletionResponse>()
            .await?;

        if let Some(error) = response.error {
            anyhow::bail!("openai-compatible backend error: {}", error.message);
        }

        response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow::anyhow!("openai-compatible backend returned no content"))
    }

    async fn stream(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let response = self
            .request_builder()
            .json(&ChatCompletionRequest {
                model,
                messages: vec![ChatMessage {
                    role: "user",
                    content: prompt,
                }],
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

                if !line.starts_with("data:") {
                    continue;
                }

                let data = line.trim_start_matches("data:").trim();
                if data.is_empty() {
                    continue;
                }

                if data == "[DONE]" {
                    return Ok(full_response);
                }

                let parsed: ChatCompletionChunk = serde_json::from_str(data)?;
                if let Some(error) = parsed.error {
                    anyhow::bail!("openai-compatible backend error: {}", error.message);
                }

                for choice in parsed.choices {
                    if let Some(token) = choice.delta.content {
                        full_response.push_str(&token);
                        if tx.send(token).await.is_err() {
                            return Ok(full_response);
                        }
                    }
                }
            }
        }

        Ok(full_response)
    }

    async fn warm_model(&self, model: &str) -> anyhow::Result<()> {
        if let Err(error) = self
            .post_model_management_variants(
                &[
                    "/api/v1/models/load",
                    "/api/v0/models/load",
                    "/api/v1/model/load",
                    "/api/v0/model/load",
                ],
                model,
                "LM Studio load",
            )
            .await
        {
            tracing::warn!(
                "LM Studio management load for `{model}` failed, falling back to chat warmup: {error}"
            );
        }

        // LM Studio may expose a load endpoint but still delay GPU/CPU
        // residency until the first completion. Pay that cost during model
        // switch, not on the user's first prompt.
        let _ = self.call("warm up", model).await?;
        Ok(())
    }

    async fn unload_model(&self, model: &str) -> anyhow::Result<()> {
        self.post_model_management_variants(
            &[
                "/api/v1/models/unload",
                "/api/v0/models/unload",
                "/api/v1/model/unload",
                "/api/v0/model/unload",
            ],
            model,
            "LM Studio unload",
        )
        .await
    }
}
