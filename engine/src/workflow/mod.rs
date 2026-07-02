pub mod phases;
pub mod registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub phases: &'static [phases::Phase],
}
