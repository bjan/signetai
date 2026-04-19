//! Document worker: ingest external documents via chunking + embedding.
//!
//! Polls `memory_jobs` for `document_ingest` jobs, fetches content,
//! splits into overlapping chunks, embeds each chunk, and indexes.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::{info, warn};

use signet_core::db::DbPool;

use crate::provider::{LlmProvider, LlmSemaphore};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the document worker.
#[derive(Debug, Clone)]
pub struct DocumentConfig {
    pub poll_ms: u64,
    pub max_retries: u32,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub max_chunks: usize,
    pub extraction_timeout_ms: u64,
    pub extraction_max_tokens: u32,
}

impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            poll_ms: 2_000,
            max_retries: 3,
            chunk_size: 1024,
            chunk_overlap: 128,
            max_chunks: 100,
            extraction_timeout_ms: 60_000,
            extraction_max_tokens: 2048,
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A document chunk ready for embedding.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub index: usize,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Result of document ingestion.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResult {
    pub chunks_created: usize,
    pub chunks_skipped: usize,
    pub total_chars: usize,
}

// ---------------------------------------------------------------------------
// Worker handle
// ---------------------------------------------------------------------------

pub struct DocumentHandle {
    shutdown: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl DocumentHandle {
    pub async fn stop(self) {
        let _ = self.shutdown.send(true);
        let _ = self.handle.await;
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

pub fn start(
    pool: DbPool,
    provider: Arc<dyn LlmProvider>,
    semaphore: Arc<LlmSemaphore>,
    config: DocumentConfig,
) -> DocumentHandle {
    let (tx, rx) = watch::channel(false);
    let handle = tokio::spawn(worker_loop(pool, provider, semaphore, config, rx));
    DocumentHandle {
        shutdown: tx,
        handle,
    }
}

async fn worker_loop(
    pool: DbPool,
    _provider: Arc<dyn LlmProvider>,
    _semaphore: Arc<LlmSemaphore>,
    config: DocumentConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut failures: u32 = 0;
    let base = Duration::from_millis(config.poll_ms);
    let max = Duration::from_secs(120);

    info!(poll_ms = config.poll_ms, "document worker started");

    loop {
        if *shutdown.borrow() {
            info!("document worker shutting down");
            break;
        }

        let delay = if failures > 0 {
            (base * 2u32.pow(failures.min(6))).min(max)
        } else {
            base
        };

        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = shutdown.changed() => {
                info!("document worker shutting down");
                break;
            }
        }

        // Lease a document_ingest job
        let job = match lease_document_job(&pool, config.max_retries).await {
            Ok(Some(j)) => j,
            Ok(None) => continue,
            Err(e) => {
                warn!(err = %e, "failed to lease document job");
                failures += 1;
                continue;
            }
        };

        info!(job_id = %job.id, "processing document ingest job");

        match process_document(&pool, &job, &config).await {
            Ok(result) => {
                failures = 0;
                let json = serde_json::to_string(&result).unwrap_or_default();
                if let Err(e) = complete_document_job(&pool, &job.id, &json).await {
                    warn!(err = %e, "failed to complete document job");
                }
                info!(
                    job_id = %job.id,
                    chunks = result.chunks_created,
                    chars = result.total_chars,
                    "document ingestion completed"
                );
            }
            Err(e) => {
                failures += 1;
                warn!(err = %e, job_id = %job.id, "document job failed");
                if let Err(fe) = fail_document_job(&pool, &job.id, &e).await {
                    warn!(err = %fe, "failed to record document failure");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Job processing
// ---------------------------------------------------------------------------

/// Split content into overlapping chunks.
pub fn chunk_content(content: &str, size: usize, overlap: usize, max: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let bytes = content.as_bytes();
    let mut start = 0;

    while start < bytes.len() && chunks.len() < max {
        let end = (start + size).min(bytes.len());

        // Find a safe UTF-8 boundary
        let safe_end = if end < bytes.len() {
            // Walk back to find a char boundary
            let mut e = end;
            while e > start && !content.is_char_boundary(e) {
                e -= 1;
            }
            e
        } else {
            end
        };

        if safe_end <= start {
            break;
        }

        chunks.push(Chunk {
            index: chunks.len(),
            text: content[start..safe_end].to_string(),
            start,
            end: safe_end,
        });

        // Advance by (size - overlap), ensuring progress
        let step = size.saturating_sub(overlap).max(1);
        start += step;

        // Find next char boundary
        while start < bytes.len() && !content.is_char_boundary(start) {
            start += 1;
        }
    }

    chunks
}

async fn process_document(
    _pool: &DbPool,
    job: &DocumentJob,
    config: &DocumentConfig,
) -> Result<DocumentResult, String> {
    let content = job
        .payload
        .as_deref()
        .ok_or("document job missing payload")?;

    // Parse payload to get content
    let payload: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("invalid payload: {e}"))?;

    let text = payload["content"]
        .as_str()
        .ok_or("payload missing 'content' field")?;

    if text.trim().is_empty() {
        return Ok(DocumentResult {
            chunks_created: 0,
            chunks_skipped: 0,
            total_chars: 0,
        });
    }

    let chunks = chunk_content(
        text,
        config.chunk_size,
        config.chunk_overlap,
        config.max_chunks,
    );
    let total_chars = text.len();
    let chunks_created = chunks.len();

    // TODO: Phase 5.3 — embed each chunk, create memory rows, link to document,
    // sync vector index, update document status

    Ok(DocumentResult {
        chunks_created,
        chunks_skipped: 0,
        total_chars,
    })
}

// ---------------------------------------------------------------------------
// Job types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentJob {
    id: String,
    payload: Option<String>,
    attempts: i64,
}

// ---------------------------------------------------------------------------
// Job queue
// ---------------------------------------------------------------------------

async fn lease_document_job(
    pool: &DbPool,
    max_attempts: u32,
) -> Result<Option<DocumentJob>, String> {
    let val = pool
        .write(signet_core::db::Priority::Low, move |conn| {
            let ts = chrono::Utc::now().to_rfc3339();
            let mut stmt = conn.prepare_cached(
                "UPDATE memory_jobs SET status = 'leased', leased_at = ?1, updated_at = ?1, attempts = attempts + 1
                 WHERE id = (
                    SELECT id FROM memory_jobs
                    WHERE status = 'pending' AND job_type = 'document_ingest' AND attempts < ?2
                    ORDER BY created_at ASC LIMIT 1
                 ) RETURNING id, payload, attempts",
            )?;

            let job = stmt
                .query_row(rusqlite::params![ts, max_attempts], |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "payload": row.get::<_, Option<String>>(1)?,
                        "attempts": row.get::<_, i64>(2)?,
                    }))
                })
                .ok();

            Ok(job.unwrap_or(serde_json::Value::Null))
        })
        .await
        .map_err(|e| e.to_string())?;

    if val.is_null() {
        Ok(None)
    } else {
        serde_json::from_value(val).map_err(|e| e.to_string())
    }
}

async fn complete_document_job(pool: &DbPool, job_id: &str, result: &str) -> Result<(), String> {
    let id = job_id.to_string();
    let result = result.to_string();
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let ts = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE memory_jobs SET status = 'completed', result = ?1, completed_at = ?2, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![result, ts, id],
        )?;
        Ok(serde_json::Value::Null)
    })
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

async fn fail_document_job(pool: &DbPool, job_id: &str, error: &str) -> Result<(), String> {
    let id = job_id.to_string();
    let error = error.to_string();
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let ts = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE memory_jobs SET status = 'pending', error = ?1, failed_at = ?2, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![error, ts, id],
        )?;
        Ok(serde_json::Value::Null)
    })
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_basic() {
        let text = "abcdefghij"; // 10 chars
        let chunks = chunk_content(text, 4, 1, 100);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].text, "abcd");
        // With overlap=1, step=3, second chunk starts at 3
        assert_eq!(chunks[1].text, "defg");
    }

    #[test]
    fn chunk_empty() {
        let chunks = chunk_content("", 4, 1, 100);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_max_limit() {
        let text = "a".repeat(1000);
        let chunks = chunk_content(&text, 10, 0, 5);
        assert_eq!(chunks.len(), 5);
    }

    #[test]
    fn chunk_unicode_safety() {
        // Multi-byte UTF-8: each char is 4 bytes
        let text = "🎉🎊🎈🎁🎂";
        let chunks = chunk_content(text, 8, 0, 100);
        // Each emoji is 4 bytes, chunk_size=8 fits 2 emojis
        for chunk in &chunks {
            // Verify no panics on invalid UTF-8
            assert!(!chunk.text.is_empty());
        }
    }
}
