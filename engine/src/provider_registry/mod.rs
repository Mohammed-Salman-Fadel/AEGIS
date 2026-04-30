use std::env;
use std::sync::RwLock;

use crate::config::InferenceProvider;

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
}

fn default_provider() -> InferenceProvider {
    match env::var("AEGIS_INFERENCE_PROVIDER")
        .unwrap_or_else(|_| "ollama".to_string())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "lmstudio" | "lm-studio" | "lm_studio" => InferenceProvider::LmStudio,
        "openai-compatible" | "openai_compatible" | "openai-compat" | "openai_compat" => {
            InferenceProvider::OpenAiCompatible
        }
        _ => InferenceProvider::Ollama,
    }
}
