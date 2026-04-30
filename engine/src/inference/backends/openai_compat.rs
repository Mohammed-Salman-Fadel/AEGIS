use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
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
        let builder = self
            .client
            .post(self.chat_completions_url())
            .header(CONTENT_TYPE, "application/json");

        match &self.api_key {
            Some(api_key) => builder.header(AUTHORIZATION, format!("Bearer {api_key}")),
            None => builder,
        }
    }

    fn models_url(&self) -> String {
        format!("{}/v1/models", self.base_url)
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
        let mut builder = self
            .client
            .get(self.models_url())
            .header(CONTENT_TYPE, "application/json");

        if let Some(api_key) = &self.api_key {
            builder = builder.header(AUTHORIZATION, format!("Bearer {api_key}"));
        }

        let response = builder.send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("openai-compatible models error {status}: {body}");
        }

        let response = response.json::<ModelListResponse>().await?;
        Ok(response.data.into_iter().map(|m| m.id).collect())
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
}
