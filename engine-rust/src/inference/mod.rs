pub mod backends;

use async_trait::async_trait;

#[async_trait]
pub trait InferenceBackend {
    // TODO: call, stream, list_models, health
}
