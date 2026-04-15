use crate::context::RequestContext;
use crate::workflow::WorkflowId;

pub struct Classifier;

impl Classifier {
    pub fn new() -> Self { Self }

    pub fn classify(&self, _ctx: &RequestContext) -> WorkflowId {
        // TODO: heuristics + small model fallback
        WorkflowId::Default
    }
}
