use std::sync::RwLock;

use serde::{Deserialize, Serialize};

pub struct RagClient {
    storage: RwLock<Vec<String>>,
    base_url: Option<String>,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct IndexRequest<'a> {
    path: &'a str,
}

#[derive(Debug, Deserialize)]
struct IndexResponse {
    #[allow(dead_code)]
    status: String,
    chunks_added: usize,
}

#[derive(Debug, Serialize)]
struct QueryRequest<'a> {
    query: &'a str,
    top_k: usize,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    text: String,
}

impl RagClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            storage: RwLock::new(Vec::new()),
            base_url: base_url.map(|url| url.trim_end_matches('/').to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub async fn health(&self) -> bool {
        let Some(base_url) = &self.base_url else {
            return true;
        };

        self.client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    }

    pub async fn initialize(&self) -> anyhow::Result<()> {
        let Some(base_url) = &self.base_url else {
            return Ok(());
        };

        self.client
            .post(format!("{base_url}/init"))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn ingest(&self, content: String) -> anyhow::Result<()> {
        let mut docs = self.storage.write().map_err(|_| anyhow::anyhow!("Lock error"))?;
        docs.push(content);
        Ok(())
    }

    pub async fn ingest_path(&self, path: &str) -> anyhow::Result<usize> {
        let Some(base_url) = &self.base_url else {
            let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
            self.ingest(content).await?;
            return Ok(0);
        };

        self.initialize().await?;
        let response = self
            .client
            .post(format!("{base_url}/index"))
            .json(&IndexRequest { path })
            .send()
            .await?
            .error_for_status()?
            .json::<IndexResponse>()
            .await?;

        Ok(response.chunks_added)
    }

    pub async fn retrieve(&self, query: &str, limit: usize) -> anyhow::Result<Vec<String>> {
        let Some(base_url) = &self.base_url else {
            let docs = self.storage.read().map_err(|_| anyhow::anyhow!("Lock error"))?;
            return Ok(docs.clone());
        };

        self.initialize().await?;
        let response = self
            .client
            .post(format!("{base_url}/query"))
            .json(&QueryRequest {
                query,
                top_k: limit,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<QueryResponse>()
            .await?;

        Ok(response.results.into_iter().map(|result| result.text).collect())
    }
}
