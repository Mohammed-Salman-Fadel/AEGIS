use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct RagClient {
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct IndexRequest {
    path: String,
    session_id: String,
}

#[derive(Serialize)]
struct QueryRequest {
    query: String,
    top_k: usize,
    session_id: String,
}

#[derive(Serialize)]
struct DeleteDocumentRequest {
    session_id: String,
    source: String,
}

#[derive(Deserialize, Clone)]
struct SearchResult {
    text: String,
    source: String,
    page: Option<i32>,
    score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagMetrics {
    pub retrieval_time_ms: f64,
    pub avg_similarity: f64,
    pub chunk_count: usize,
    pub backend: String,
}

impl Default for RagMetrics {
    fn default() -> Self {
        Self {
            retrieval_time_ms: 0.0,
            avg_similarity: 0.0,
            chunk_count: 0,
            backend: "unknown".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct QueryResponse {
    results: Vec<SearchResult>,
    metrics: RagMetrics,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalChunk {
    pub text: String,
    pub source: String,
    pub page: Option<i32>,
    pub score: f64,
}

pub struct RetrievalOutcome {
    pub chunks: Vec<RetrievalChunk>,
    pub metrics: RagMetrics,
}

impl Default for RetrievalOutcome {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            metrics: RagMetrics::default(),
        }
    }
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

    pub async fn ingest(
        &self,
        file_path: String,
        session_id: &str,
    ) -> anyhow::Result<IndexOutcome> {
        self.init().await?; // Ensure it is initialized before ingesting

        let resp = self
            .client
            .post(&format!("{}/index", self.base_url))
            .json(&IndexRequest {
                path: file_path,
                session_id: session_id.to_string(),
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("RAG index failed: {}", txt);
        }

        Ok(resp.json::<IndexOutcome>().await?)
    }

    pub async fn retrieve(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
        session_id: &str,
    ) -> anyhow::Result<RetrievalOutcome> {
        self.init().await?; // Ensure it is initialized before querying

        let resp = self
            .client
            .post(&format!("{}/query", self.base_url))
            .json(&QueryRequest {
                query: query.to_string(),
                top_k: limit,
                session_id: session_id.to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("RAG query failed: {}", txt);
        }

        let result: QueryResponse = resp.json().await?;
        let filtered_chunks = result
            .results
            .into_iter()
            .filter(|r| r.score >= threshold)
            .map(|r| RetrievalChunk {
                text: r.text,
                source: r.source,
                page: r.page,
                score: r.score,
            })
            .collect();

        Ok(RetrievalOutcome {
            chunks: filtered_chunks,
            metrics: result.metrics,
        })
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/delete/{}", self.base_url, session_id);

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .context("Failed to send delete request to RAG service")?;

        if !response.status().is_success() {
            let error: serde_json::Value = response
                .json::<serde_json::Value>()
                .await
                .unwrap_or_default();
            anyhow::bail!("RAG deletion failed: {}", error);
        }

        Ok(())
    }

    pub async fn delete_document(&self, session_id: &str, source: &str) -> anyhow::Result<usize> {
        self.init().await?;

        let response = self
            .client
            .post(&format!("{}/delete-document", self.base_url))
            .json(&DeleteDocumentRequest {
                session_id: session_id.to_string(),
                source: source.to_string(),
            })
            .send()
            .await
            .context("Failed to send document delete request to RAG service")?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("RAG document deletion failed: {}", text);
        }

        let payload: serde_json::Value = response.json().await.unwrap_or_default();
        Ok(payload
            .get("deleted_count")
            .and_then(|count| count.as_i64())
            .filter(|count| *count > 0)
            .unwrap_or(0) as usize)
    }

    pub async fn transcribe(
        &self,
        audio_bytes: Vec<u8>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(audio_bytes).file_name("voice.wav"),
        );

        let response = self
            .client
            .post(format!("{}/transcribe", self.base_url))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(format!("RAG transcription failed: {}", err).into());
        }

        let data: Value = response.json().await?;
        Ok(data["text"].as_str().unwrap_or("").to_string())
    }

    pub async fn synthesize(
        &self,
        text: String,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .get(format!("{}/synthesize", self.base_url))
            .query(&[("text", &text)])
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(format!("RAG synthesis failed: {}", err).into());
        }

        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn configure_voice(
        &self,
        keep_cached: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .post(format!("{}/voice/config", self.base_url))
            .query(&[("keep_cached", &keep_cached.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(format!("RAG voice config failed: {}", err).into());
        }

        Ok(())
    }
}
