use std::env;
use std::sync::RwLock;

const DEFAULT_MODEL: &str = "llama3.2:latest";

#[derive(Clone)]
pub struct ModelProfile {
    pub name: String,
    pub context_window: usize,
    pub output_reserve: usize,
}

impl ModelProfile {
    pub fn usable_context(&self) -> usize {
        self.context_window - self.output_reserve
    }
}

pub struct ModelRegistry {
    active_model: RwLock<String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            active_model: RwLock::new(
                env::var("AEGIS_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            ),
        }
    }

    pub fn current_model_name(&self) -> String {
        self.active_model
            .read()
            .map(|model| model.clone())
            .unwrap_or_else(|_| DEFAULT_MODEL.to_string())
    }

    pub fn set_active_model(&self, name: impl Into<String>) -> String {
        let name = name.into();
        match self.active_model.write() {
            Ok(mut active_model) => {
                let previous = active_model.clone();
                *active_model = name;
                previous
            }
            Err(_) => DEFAULT_MODEL.to_string(),
        }
    }

    pub fn get_active(&self) -> ModelProfile {
        ModelProfile {
            name: self.current_model_name(),
            context_window: 8192,
            output_reserve: 512,
        }
    }

    pub fn seed_active_model(&self, name: impl Into<String>) {
        if let Ok(mut active_model) = self.active_model.write() {
            *active_model = name.into();
        }
    }
}
