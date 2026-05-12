pub mod backends;

use async_trait::async_trait;
use tokio::sync::mpsc;

#[derive(Clone, Debug, Default)]
pub struct InferenceUsage {
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct InferenceResponse {
    pub text: String,
    pub usage: InferenceUsage,
}

#[async_trait]
pub trait InferenceBackend {
    async fn call(&self, prompt: &str, model: &str) -> anyhow::Result<String>;

    async fn stream(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String>;

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn call_with_usage(
        &self,
        prompt: &str,
        model: &str,
    ) -> anyhow::Result<InferenceResponse> {
        Ok(InferenceResponse {
            text: self.call(prompt, model).await?,
            usage: InferenceUsage::default(),
        })
    }

    async fn stream_with_usage(
        &self,
        prompt: &str,
        model: &str,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<InferenceResponse> {
        Ok(InferenceResponse {
            text: self.stream(prompt, model, tx).await?,
            usage: InferenceUsage::default(),
        })
    }

    async fn context_window(&self, _model: &str) -> anyhow::Result<Option<usize>> {
        Ok(None)
    }

    async fn warm_model(&self, _model: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn unload_model(&self, _model: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
