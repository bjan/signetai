//! Retention worker: data lifecycle management.
//!
//! Runs on a configurable interval to purge expired data in a safe,
//! ordered sequence. Archives to cold storage before hard deletion.

use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;
use tracing::{info, warn};

use signet_core::db::DbPool;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the retention worker.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    pub interval_secs: u64,
    pub batch_size: u32,
    pub tombstone_days: u32,
    pub history_days: u32,
    pub completed_job_days: u32,
    pub dead_job_days: u32,
    pub training_pair_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            interval_secs: 6 * 3600, // 6 hours
            batch_size: 500,
            tombstone_days: 30,
            history_days: 180,
            completed_job_days: 14,
            dead_job_days: 30,
            training_pair_days: 90,
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a retention sweep.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionResult {
    pub graph_links_purged: usize,
    pub orphan_entities_cleaned: usize,
    pub embeddings_purged: usize,
    pub tombstones_purged: usize,
    pub history_purged: usize,
    pub completed_jobs_purged: usize,
    pub dead_jobs_purged: usize,
    pub training_pairs_purged: usize,
}

// ---------------------------------------------------------------------------
// Worker handle
// ---------------------------------------------------------------------------

pub struct RetentionHandle {
    shutdown: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl RetentionHandle {
    pub async fn stop(self) {
        let _ = self.shutdown.send(true);
        let _ = self.handle.await;
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

pub fn start(pool: DbPool, config: RetentionConfig) -> RetentionHandle {
    let (tx, rx) = watch::channel(false);
    let handle = tokio::spawn(worker_loop(pool, config, rx));
    RetentionHandle {
        shutdown: tx,
        handle,
    }
}

async fn worker_loop(pool: DbPool, config: RetentionConfig, mut shutdown: watch::Receiver<bool>) {
    let interval = Duration::from_secs(config.interval_secs);

    info!(
        interval_secs = config.interval_secs,
        "retention worker started"
    );

    loop {
        // Wait for interval or shutdown
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = shutdown.changed() => {
                info!("retention worker shutting down");
                break;
            }
        }

        if *shutdown.borrow() {
            break;
        }

        info!("starting retention sweep");

        match run_sweep(&pool, &config).await {
            Ok(result) => {
                let total = result.graph_links_purged
                    + result.orphan_entities_cleaned
                    + result.embeddings_purged
                    + result.tombstones_purged
                    + result.history_purged
                    + result.completed_jobs_purged
                    + result.dead_jobs_purged
                    + result.training_pairs_purged;

                if total > 0 {
                    info!(
                        tombstones = result.tombstones_purged,
                        history = result.history_purged,
                        jobs = result.completed_jobs_purged + result.dead_jobs_purged,
                        total,
                        "retention sweep completed"
                    );
                }
            }
            Err(e) => {
                warn!(err = %e, "retention sweep failed");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sweep stages (ordered for referential integrity)
// ---------------------------------------------------------------------------

/// Run a full retention sweep. Each step is a separate short transaction.
pub async fn run_sweep(pool: &DbPool, config: &RetentionConfig) -> Result<RetentionResult, String> {
    let mut result = RetentionResult {
        graph_links_purged: 0,
        orphan_entities_cleaned: 0,
        embeddings_purged: 0,
        tombstones_purged: 0,
        history_purged: 0,
        completed_jobs_purged: 0,
        dead_jobs_purged: 0,
        training_pairs_purged: 0,
    };

    // Step 1: Purge graph links for deleted memories
    result.graph_links_purged = purge_graph_links(pool, config.batch_size).await?;

    // Step 2: Clean orphan entities
    result.orphan_entities_cleaned = purge_orphan_entities(pool, config.batch_size).await?;

    // Step 3: Purge embeddings for deleted memories
    result.embeddings_purged = purge_orphan_embeddings(pool, config.batch_size).await?;

    // Step 4: Hard-delete tombstoned memories (archive to cold first)
    result.tombstones_purged =
        purge_tombstones(pool, config.tombstone_days, config.batch_size).await?;

    // Step 5: Purge old history events
    result.history_purged = purge_history(pool, config.history_days, config.batch_size).await?;

    // Step 6: Purge completed jobs
    result.completed_jobs_purged =
        purge_completed_jobs(pool, config.completed_job_days, config.batch_size).await?;

    // Step 7: Purge dead-letter jobs
    result.dead_jobs_purged =
        purge_dead_jobs(pool, config.dead_job_days, config.batch_size).await?;

    // Step 8: Purge old training pairs
    result.training_pairs_purged =
        purge_training_pairs(pool, config.training_pair_days, config.batch_size).await?;

    Ok(result)
}

async fn purge_graph_links(pool: &DbPool, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM memory_entity_mentions WHERE memory_id IN (
                SELECT id FROM memories WHERE deleted = 1 LIMIT ?1
            )",
            rusqlite::params![batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_orphan_entities(pool: &DbPool, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM entities WHERE id IN (
                SELECT e.id FROM entities e
                LEFT JOIN memory_entity_mentions m ON m.entity_id = e.id
                WHERE m.entity_id IS NULL AND e.pinned = 0
                LIMIT ?1
            )",
            rusqlite::params![batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_orphan_embeddings(pool: &DbPool, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM memory_embeddings WHERE memory_id IN (
                SELECT me.memory_id FROM memory_embeddings me
                LEFT JOIN memories m ON m.id = me.memory_id
                WHERE m.id IS NULL OR m.deleted = 1
                LIMIT ?1
            )",
            rusqlite::params![batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_tombstones(pool: &DbPool, days: u32, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        // Archive to cold storage first
        conn.execute(
            "INSERT OR IGNORE INTO memories_cold (id, content, type, source_type, importance, created_at, updated_at, deleted_at, archived_at, archive_reason)
             SELECT id, content, type, source_type, importance, created_at, updated_at, deleted_at, datetime('now'), 'retention_decay'
             FROM memories
             WHERE deleted = 1 AND deleted_at < datetime('now', ?1)
             LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;

        // Hard delete
        let count = conn.execute(
            "DELETE FROM memories WHERE deleted = 1 AND deleted_at < datetime('now', ?1) LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;

        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_history(pool: &DbPool, days: u32, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM memory_history WHERE created_at < datetime('now', ?1) LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_completed_jobs(pool: &DbPool, days: u32, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM memory_jobs WHERE status = 'completed' AND completed_at < datetime('now', ?1) LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_dead_jobs(pool: &DbPool, days: u32, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM memory_jobs WHERE status = 'dead' AND updated_at < datetime('now', ?1) LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}

async fn purge_training_pairs(pool: &DbPool, days: u32, batch: u32) -> Result<usize, String> {
    pool.write(signet_core::db::Priority::Low, move |conn| {
        let count = conn.execute(
            "DELETE FROM predictor_training_pairs WHERE created_at < datetime('now', ?1) LIMIT ?2",
            rusqlite::params![format!("-{days} days"), batch],
        )?;
        Ok(serde_json::json!(count))
    })
    .await
    .map(|v| v.as_u64().unwrap_or(0) as usize)
    .map_err(|e| e.to_string())
}
