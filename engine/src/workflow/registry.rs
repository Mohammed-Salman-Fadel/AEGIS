use super::phases::Phase;
use super::{WorkflowDef, WorkflowId};

/// Registry of workflow phase pipelines.
///
/// Each workflow defines an ordered list of phases that the orchestrator
/// should execute.  The phases determine which context sources are queried
/// and how the final response is synthesised.
pub struct WorkflowRegistry;

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self
    }

    /// Look up a workflow definition by its ID.
    ///
    /// Returns a reference to a static definition — no heap allocation.
    pub fn get(&self, id: WorkflowId) -> &'static WorkflowDef {
        match id {
            WorkflowId::Default => &DEFAULT_WORKFLOW,
            WorkflowId::DocumentQA => &DOCUMENT_QA_WORKFLOW,
            WorkflowId::Summarize => &SUMMARIZE_WORKFLOW,
            WorkflowId::CodeExplain => &CODE_EXPLAIN_WORKFLOW,
            WorkflowId::CodeGenerate => &CODE_GENERATE_WORKFLOW,
            WorkflowId::CodeDebug => &CODE_DEBUG_WORKFLOW,
            WorkflowId::Writing => &WRITING_WORKFLOW,
        }
    }
}

// ── static workflow definitions ───────────────────────────────────────

const DEFAULT_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::Default,
    phases: &[Phase::Plan, Phase::Execute, Phase::Synthesize],
};

const DOCUMENT_QA_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::DocumentQA,
    phases: &[
        Phase::RagRetrieve { top_k: 5 },
        Phase::Compact,
        Phase::Synthesize,
    ],
};

const SUMMARIZE_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::Summarize,
    phases: &[
        Phase::RagRetrieve { top_k: 8 },
        Phase::Compact,
        Phase::Synthesize,
    ],
};

const CODE_EXPLAIN_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::CodeExplain,
    phases: &[
        Phase::RagRetrieve { top_k: 3 },
        Phase::Plan,
        Phase::Execute,
        Phase::Synthesize,
    ],
};

const CODE_GENERATE_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::CodeGenerate,
    phases: &[Phase::Plan, Phase::Execute, Phase::Synthesize],
};

const CODE_DEBUG_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::CodeDebug,
    phases: &[
        Phase::RagRetrieve { top_k: 5 },
        Phase::Plan,
        Phase::Execute,
        Phase::Synthesize,
    ],
};

const WRITING_WORKFLOW: WorkflowDef = WorkflowDef {
    id: WorkflowId::Writing,
    phases: &[Phase::Plan, Phase::Execute, Phase::Synthesize],
};
