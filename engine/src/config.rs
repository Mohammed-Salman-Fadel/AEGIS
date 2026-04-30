use std::env;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InferenceProvider {
    Ollama,
    LmStudio,
    OpenAiCompatible,
}

impl InferenceProvider {
    pub fn from_env_value(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "lmstudio" | "lm-studio" | "lm_studio" => Ok(Self::LmStudio),
            "openai-compatible" | "openai_compatible" | "openai-compat" | "openai_compat" => {
                Ok(Self::OpenAiCompatible)
            }
            unknown => anyhow::bail!(
                "unsupported inference provider `{unknown}`; expected ollama, lmstudio, or openai-compatible"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceConfig {
    pub provider: InferenceProvider,
    pub base_url: String,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    pub host: String,
    pub port: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub inference: InferenceConfig,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let provider = InferenceProvider::from_env_value(
            &env::var("AEGIS_INFERENCE_PROVIDER").unwrap_or_else(|_| "ollama".to_string()),
        )?;

        Ok(Self {
            server: ServerConfig {
                host: env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string()),
            },
            inference: InferenceConfig {
                base_url: inference_base_url(&provider),
                provider,
                api_key: non_empty_env("AEGIS_OPENAI_COMPAT_API_KEY"),
            },
        })
    }
}

fn inference_base_url(provider: &InferenceProvider) -> String {
    match provider {
        InferenceProvider::Ollama => {
            env::var("AEGIS_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
        }
        InferenceProvider::LmStudio => non_empty_env("AEGIS_LM_STUDIO_URL")
            .or_else(|| non_empty_env("AEGIS_LMSTUDIO_URL"))
            .or_else(|| non_empty_env("AEGIS_OPENAI_COMPAT_URL"))
            .unwrap_or_else(|| "http://127.0.0.1:1234".to_string()),
        InferenceProvider::OpenAiCompatible => non_empty_env("AEGIS_OPENAI_COMPAT_URL")
            .unwrap_or_else(|| "http://127.0.0.1:1234".to_string()),
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
