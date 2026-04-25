use std::sync::RwLock;

pub struct RagClient {
    storage: RwLock<Vec<String>>,
}

impl RagClient {
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(Vec::new()),
        }
    }

    pub async fn ingest(&self, content: String) -> anyhow::Result<()> {
        let mut docs = self
            .storage
            .write()
            .map_err(|_| anyhow::anyhow!("Lock error"))?;
        docs.push(content);
        Ok(())
    }

    pub async fn retrieve(&self, _query: &str, _limit: usize) -> anyhow::Result<Vec<String>> {
        let docs = self
            .storage
            .read()
            .map_err(|_| anyhow::anyhow!("Lock error"))?;
        Ok(docs.clone())
    }
}
