use std::env;
use std::sync::RwLock;

use crate::config::InferenceProvider;

#[derive(Clone, Debug)]
pub struct ProviderDescriptor {
    pub name: String,
    pub description: String,
    pub active: bool,
    pub capabilities: ProviderCapabilities,
}

#[derive(Clone, Debug)]
pub struct ProviderCapabilities {
    pub chat: bool,
    pub streaming: bool,
    pub model_listing: bool,
    pub model_download: bool,
    pub model_unload: bool,
    pub context_window_detection: bool,
    pub requires_external_app: bool,
    pub notes: Vec<String>,
}

pub struct ProviderRegistry {
    active_provider: RwLock<InferenceProvider>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            active_provider: RwLock::new(default_provider()),
        }
    }

    pub fn current_provider(&self) -> InferenceProvider {
        self.active_provider
            .read()
            .map(|provider| provider.clone())
            .unwrap_or_else(|_| default_provider())
    }

    pub fn set_active_provider(&self, provider: InferenceProvider) -> InferenceProvider {
        match self.active_provider.write() {
            Ok(mut active_provider) => {
                let previous = active_provider.clone();
                *active_provider = provider;
                previous
            }
            Err(_) => default_provider(),
        }
    }

    pub fn current_provider_name(&self) -> String {
        self.current_provider().as_str().to_string()
    }

    pub fn list_descriptors(&self) -> Vec<ProviderDescriptor> {
        let current = self.current_provider();
        [
            InferenceProvider::Ollama,
            InferenceProvider::LmStudio,
            InferenceProvider::OpenAiCompatible,
        ]
        .into_iter()
        .map(|provider| ProviderDescriptor {
            name: provider.as_str().to_string(),
            description: provider.description().to_string(),
            active: provider == current,
            capabilities: provider.capabilities(),
        })
        .collect()
    }
}

fn default_provider() -> InferenceProvider {
    env::var("AEGIS_INFERENCE_PROVIDER")
        .ok()
        .and_then(|value| InferenceProvider::from_env_value(&value).ok())
        .unwrap_or(InferenceProvider::Ollama)
}

impl InferenceProvider {
    fn description(&self) -> &'static str {
        match self {
            Self::Ollama => "Local Ollama provider",
            Self::LmStudio => "LM Studio OpenAI-compatible provider",
            Self::OpenAiCompatible => "Generic OpenAI-compatible provider",
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::Ollama => ProviderCapabilities {
                chat: true,
                streaming: true,
                model_listing: true,
                model_download: true,
                model_unload: true,
                context_window_detection: true,
                requires_external_app: true,
                notes: vec![
                    "Best default for local-first AEGIS installs.".to_string(),
                    "Supports pull, warm, unload, tags, and context detection.".to_string(),
                ],
            },
            Self::LmStudio => ProviderCapabilities {
                chat: true,
                streaming: true,
                model_listing: true,
                model_download: true,
                model_unload: true,
                context_window_detection: false,
                requires_external_app: true,
                notes: vec![
                    "Uses LM Studio's OpenAI-compatible server for chat.".to_string(),
                    "Model download depends on LM Studio management API availability.".to_string(),
                ],
            },
            Self::OpenAiCompatible => ProviderCapabilities {
                chat: true,
                streaming: true,
                model_listing: true,
                model_download: false,
                model_unload: false,
                context_window_detection: false,
                requires_external_app: false,
                notes: vec![
                    "Good future extension point for remote or custom OpenAI-compatible APIs."
                        .to_string(),
                    "Model lifecycle is intentionally not managed by AEGIS.".to_string(),
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_descriptors_expose_core_capabilities() {
        let registry = ProviderRegistry::new();
        let descriptors = registry.list_descriptors();

        let ollama = descriptors
            .iter()
            .find(|provider| provider.name == "ollama")
            .expect("ollama descriptor");
        assert!(ollama.capabilities.model_download);
        assert!(ollama.capabilities.context_window_detection);

        let openai_compatible = descriptors
            .iter()
            .find(|provider| provider.name == "openai-compatible")
            .expect("openai-compatible descriptor");
        assert!(openai_compatible.capabilities.chat);
        assert!(!openai_compatible.capabilities.model_download);
    }
}
