use tokio::sync::mpsc;

use crate::config::{AppConfig, InferenceProvider};
use crate::inference::InferenceBackend;

use super::backends::ollama::OllamaBackend;
use super::backends::openai_compat::OpenAiCompatBackend;

#[derive(Clone, Debug)]
pub struct ProviderInfo {
    pub name: String,
    pub description: String,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct ProviderSwitchResult {
    pub previous: String,
    pub current: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct ManagedModel {
    pub name: String,
    pub provider: String,
    pub description: String,
    pub loaded: bool,
    pub instance_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ModelDownloadResult {
    pub model: String,
    pub status: String,
    pub message: String,
    pub job_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ModelRunResult {
    pub model: String,
    pub message: String,
    pub instance_id: Option<String>,
}

pub struct RuntimeManager {
    ollama: OllamaBackend,
    lmstudio: OpenAiCompatBackend,
    openai_compat: OpenAiCompatBackend,
}

impl RuntimeManager {
    pub fn new(config: &AppConfig) -> Self {
        let ollama_url = std::env::var("AEGIS_OLLAMA_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
        let lmstudio_url = std::env::var("AEGIS_LM_STUDIO_URL")
            .or_else(|_| std::env::var("AEGIS_LMSTUDIO_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());
        let openai_compat_url = std::env::var("AEGIS_OPENAI_COMPAT_URL")
            .unwrap_or_else(|_| config.inference.base_url.clone());

        Self {
            ollama: OllamaBackend::new(ollama_url),
            lmstudio: OpenAiCompatBackend::new(lmstudio_url, None),
            openai_compat: OpenAiCompatBackend::new(
                openai_compat_url,
                config.inference.api_key.clone(),
            ),
        }
    }

    pub fn list_providers(&self, active: &InferenceProvider) -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                name: "ollama".to_string(),
                description: "Run local models through the Ollama server.".to_string(),
                active: matches!(active, InferenceProvider::Ollama),
            },
            ProviderInfo {
                name: "lmstudio".to_string(),
                description:
                    "Run, download, and manage models through LM Studio's local server.".to_string(),
                active: matches!(active, InferenceProvider::LmStudio),
            },
            ProviderInfo {
                name: "openai-compat".to_string(),
                description:
                    "Use a generic OpenAI-compatible local server for inference only.".to_string(),
                active: matches!(active, InferenceProvider::OpenAiCompatible),
            },
        ]
    }

    pub fn select_provider(
        &self,
        current: &InferenceProvider,
        next_name: &str,
    ) -> anyhow::Result<(InferenceProvider, ProviderSwitchResult)> {
        let next = InferenceProvider::from_env_value(next_name)?;
        let result = ProviderSwitchResult {
            previous: provider_name(current).to_string(),
            current: provider_name(&next).to_string(),
            message: if *current == next {
                format!("Provider `{}` is already active.", provider_name(&next))
            } else {
                format!(
                    "Switched inference provider from {} to {}.",
                    provider_name(current),
                    provider_name(&next)
                )
            },
        };

        Ok((next, result))
    }

    pub async fn call(
        &self,
        provider: &InferenceProvider,
        prompt: &str,
        model: &str,
    ) -> anyhow::Result<String> {
        match provider {
            InferenceProvider::Ollama => self.ollama.call(prompt, model).await,
            InferenceProvider::LmStudio => self.lmstudio.call(prompt, model).await,
            InferenceProvider::OpenAiCompatible => self.openai_compat.call(prompt, model).await,
        }
    }

    pub async fn stream(
        &self,
        provider: &InferenceProvider,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        match provider {
            InferenceProvider::Ollama => self.ollama.stream(prompt, model, tx).await,
            InferenceProvider::LmStudio => self.lmstudio.stream(prompt, model, tx).await,
            InferenceProvider::OpenAiCompatible => {
                self.openai_compat.stream(prompt, model, tx).await
            }
        }
    }

    pub async fn list_models(
        &self,
        provider: &InferenceProvider,
    ) -> anyhow::Result<Vec<ManagedModel>> {
        match provider {
            InferenceProvider::Ollama => self.ollama.list_managed_models().await,
            InferenceProvider::LmStudio => self.lmstudio.list_lmstudio_models().await,
            InferenceProvider::OpenAiCompatible => self.openai_compat.list_openai_models().await,
        }
    }

    pub async fn download_model(
        &self,
        provider: &InferenceProvider,
        model: &str,
        quantization: Option<&str>,
    ) -> anyhow::Result<ModelDownloadResult> {
        match provider {
            InferenceProvider::LmStudio => {
                self.lmstudio.download_lmstudio_model(model, quantization).await
            }
            InferenceProvider::Ollama => anyhow::bail!(
                "Model download is currently supported for the LM Studio provider only. Use `ollama pull {model}` for Ollama."
            ),
            InferenceProvider::OpenAiCompatible => anyhow::bail!(
                "Generic OpenAI-compatible providers do not expose a standard download API."
            ),
        }
    }

    pub async fn load_model(
        &self,
        provider: &InferenceProvider,
        model: &str,
        context_length: Option<usize>,
    ) -> anyhow::Result<ModelRunResult> {
        match provider {
            InferenceProvider::LmStudio => {
                self.lmstudio.load_lmstudio_model(model, context_length).await
            }
            InferenceProvider::Ollama => self.ollama.preload_model(model).await,
            InferenceProvider::OpenAiCompatible => anyhow::bail!(
                "Generic OpenAI-compatible providers do not expose a standard load API."
            ),
        }
    }

    pub async fn unload_model(
        &self,
        provider: &InferenceProvider,
        model_or_instance: &str,
    ) -> anyhow::Result<ModelRunResult> {
        match provider {
            InferenceProvider::LmStudio => {
                self.lmstudio.unload_lmstudio_model(model_or_instance).await
            }
            InferenceProvider::Ollama => self.ollama.unload_model(model_or_instance).await,
            InferenceProvider::OpenAiCompatible => anyhow::bail!(
                "Generic OpenAI-compatible providers do not expose a standard unload API."
            ),
        }
    }
}

pub fn provider_name(provider: &InferenceProvider) -> &'static str {
    match provider {
        InferenceProvider::Ollama => "ollama",
        InferenceProvider::LmStudio => "lmstudio",
        InferenceProvider::OpenAiCompatible => "openai-compat",
    }
}
