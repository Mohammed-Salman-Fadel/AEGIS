use crate::context::RequestContext;

pub struct Compactor;

impl Compactor {
    pub fn new() -> Self { Self }

    pub fn compact(&self, ctx: &mut RequestContext) {
        // TODO: measure tokens, compress history, summarize large slots
    }
}
