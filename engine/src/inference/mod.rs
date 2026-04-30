pub mod backends;

use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait InferenceBackend {
    async fn call(&self, prompt: &str, model: &str) -> anyhow::Result<String>;

    async fn stream(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String>;

    async fn warm_model(&self, _model: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn unload_model(&self, _model: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
