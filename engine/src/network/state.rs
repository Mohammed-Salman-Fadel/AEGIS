use std::sync::Arc;

use crate::orchestrator::Orchestrator;

/// Shared state injected into every handler by axum.
///
/// axum clones this for every incoming request — that clone is cheap
/// because Arc just increments a reference count, it does not copy
/// the underlying data.
#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<Orchestrator>,
}

impl AppState {
    pub fn new(orchestrator: Orchestrator) -> Self {
        Self {
            orchestrator: Arc::new(orchestrator),
        }
    }
}
