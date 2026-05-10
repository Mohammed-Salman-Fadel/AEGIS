pub mod registry;
pub mod phases;

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
    pub id:     WorkflowId,
    pub phases: Vec<phases::Phase>,
}
