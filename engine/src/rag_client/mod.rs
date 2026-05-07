use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct RagClient {
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct IndexRequest {
    path: String,
}

#[derive(Serialize)]
struct QueryRequest {
    query: String,
    top_k: usize,
}

#[derive(Deserialize)]
struct SearchResult {
    text: String,
    #[allow(dead_code)]
    source: String,
}

#[derive(Deserialize)]
struct QueryResponse {
    results: Vec<SearchResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IndexOutcome {
    pub chunks_added: usize,
}

impl RagClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    pub async fn init(&self) -> anyhow::Result<()> {
        let resp = self
            .client
            .post(&format!("{}/init", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("RAG init failed: {}", txt);
        }
        Ok(())
    }

    pub async fn ingest(&self, file_path: String) -> anyhow::Result<IndexOutcome> {
        self.init().await?; // Ensure it is initialized before ingesting

        let resp = self
            .client
            .post(&format!("{}/index", self.base_url))
            .json(&IndexRequest { path: file_path })
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("RAG index failed: {}", txt);
        }

        Ok(resp.json::<IndexOutcome>().await?)
    }

    pub async fn retrieve(&self, query: &str, limit: usize) -> anyhow::Result<Vec<String>> {
        self.init().await?; // Ensure it is initialized before querying

        let resp = self
            .client
            .post(&format!("{}/query", self.base_url))
            .json(&QueryRequest {
                query: query.to_string(),
                top_k: limit,
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("RAG query failed: {}", txt);
        }

        let result: QueryResponse = resp.json().await?;
        Ok(result.results.into_iter().map(|r| r.text).collect())
    }
}
