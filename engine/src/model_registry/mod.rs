use std::collections::HashMap;
use std::env;
use std::sync::RwLock;

const DEFAULT_MODEL: &str = "llama3.2:latest";
pub(crate) const DEFAULT_CONTEXT_WINDOW: usize = 4096;

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
    context_windows: RwLock<HashMap<String, usize>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            active_model: RwLock::new(
                env::var("AEGIS_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            ),
            context_windows: RwLock::new(HashMap::new()),
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
        let name = self.current_model_name();
        ModelProfile {
            context_window: self.current_context_window(&name),
            name,
            output_reserve: 512,
        }
    }

    /// Store the context window for a specific model.
    /// Subsequent lookups for this model return the cached value instantly.
    pub fn set_context_window(&self, model: &str, window: usize) {
        if let Ok(mut map) = self.context_windows.write() {
            map.insert(model.to_string(), window);
        }
    }

    /// Look up a model's cached context window, or return the default if unseen.
    pub fn current_context_window(&self, model: &str) -> usize {
        self.context_windows
            .read()
            .map(|map| map.get(model).copied().unwrap_or(DEFAULT_CONTEXT_WINDOW))
            .unwrap_or(DEFAULT_CONTEXT_WINDOW)
    }

    pub fn seed_active_model(&self, name: impl Into<String>) {
        if let Ok(mut active_model) = self.active_model.write() {
            *active_model = name.into();
        }
    }
}
