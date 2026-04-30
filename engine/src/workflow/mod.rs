pub mod phases;
pub mod registry;

pub enum WorkflowId {
    Default,
    DocumentQA,
    Summarize,
    CodeExplain,
    CodeGenerate,
    CodeDebug,
    Writing,
}

pub struct WorkflowDef {
    pub id: WorkflowId,
    pub phases: Vec<phases::Phase>,
}
