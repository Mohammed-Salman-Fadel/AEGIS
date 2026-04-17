use std::env;

#[derive(Clone)]
pub struct ModelProfile {
    pub name:           String,
    pub context_window: usize,
    pub output_reserve: usize,
}

impl ModelProfile {
    pub fn usable_context(&self) -> usize {
        self.context_window - self.output_reserve
    }
}

pub struct ModelRegistry;

impl ModelRegistry {
    pub fn new() -> Self { Self }

    pub fn get_active(&self) -> ModelProfile {
        ModelProfile {
            name:           env::var("AEGIS_MODEL").unwrap_or_else(|_| "qwen2.5-coder:3b".to_string()),
            context_window: 8192,
            output_reserve: 512,
        }
    }
}
