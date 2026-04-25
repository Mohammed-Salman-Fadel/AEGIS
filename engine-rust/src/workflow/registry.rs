use super::phases::Phase;
use super::{WorkflowDef, WorkflowId};

pub struct WorkflowRegistry;

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, id: WorkflowId) -> WorkflowDef {
        match id {
            WorkflowId::Default => WorkflowDef {
                id: WorkflowId::Default,
                phases: vec![Phase::Plan, Phase::Execute, Phase::Synthesize],
            },
            WorkflowId::DocumentQA => WorkflowDef {
                id: WorkflowId::DocumentQA,
                phases: vec![
                    Phase::RagRetrieve { top_k: 5 },
                    Phase::Compact,
                    Phase::Synthesize,
                ],
            },
            // TODO: remaining workflows
            _ => WorkflowDef {
                id: WorkflowId::Default,
                phases: vec![Phase::Plan, Phase::Execute, Phase::Synthesize],
            },
        }
    }
}
