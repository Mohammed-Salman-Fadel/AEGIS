use crate::context::RequestContext;

pub struct Compactor;

impl Compactor {
    pub fn new() -> Self {
        Self
    }

    pub fn compact(&self, _ctx: &mut RequestContext) {
        // TODO: measure tokens, compress history, summarize large slots
    }
}
