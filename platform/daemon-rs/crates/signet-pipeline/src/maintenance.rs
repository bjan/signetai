//! Maintenance worker: health monitoring and autonomous repair.
//!
//! Runs periodic diagnostics, detects degraded health conditions,
//! and orchestrates repair actions when thresholds are exceeded.

use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;
use tracing::{info, warn};

use signet_core::db::DbPool;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the maintenance worker.
#[derive(Debug, Clone)]
pub struct MaintenanceConfig {
    pub interval_secs: u64,
    pub auto_repair: bool,
    pub max_repair_failures: u32,
    pub dead_job_threshold_pct: f64,
    pub tombstone_threshold_pct: f64,
    pub duplicate_threshold_pct: f64,
    pub stale_lease_timeout_secs: u64,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            interval_secs: 1800, // 30 minutes
            auto_repair: false,
            max_repair_failures: 3,
            dead_job_threshold_pct: 1.0,
            tombstone_threshold_pct: 30.0,
            duplicate_threshold_pct: 5.0,
            stale_lease_timeout_secs: 300,
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Health diagnostics snapshot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub total_memories: usize,
    pub deleted_memories: usize,
    pub tombstone_ratio: f64,
    pub total_jobs: usize,
    pub pending_jobs: usize,
    pub leased_jobs: usize,
    pub dead_jobs: usize,
    pub dead_ratio: f64,
    pub stale_leases: usize,
    pub total_entities: usize,
    pub orphan_entities: usize,
    pub fts_mismatch: bool,
    pub embedding_gaps: usize,
    pub recommendations: Vec<RepairRecommendation>,
}

/// A recommended repair action.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRecommendation {
    pub action: String,
    pub reason: String,
    pub severity: String,
}

/// Result of a maintenance cycle.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceResult {
    pub diagnostics: Diagnostics,
    pub repairs_attempted: usize,
    pub repairs_succeeded: usize,
}

// ---------------------------------------------------------------------------
// Worker handle
// ---------------------------------------------------------------------------

pub struct MaintenanceHandle {
    shutdown: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl MaintenanceHandle {
    pub async fn stop(self) {
        let _ = self.shutdown.send(true);
        let _ = self.handle.await;
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

pub fn start(pool: DbPool, config: MaintenanceConfig) -> MaintenanceHandle {
    let (tx, rx) = watch::channel(false);
    let handle = tokio::spawn(worker_loop(pool, config, rx));
    MaintenanceHandle {
        shutdown: tx,
        handle,
    }
}

async fn worker_loop(pool: DbPool, config: MaintenanceConfig, mut shutdown: watch::Receiver<bool>) {
    let interval = Duration::from_secs(config.interval_secs);

    info!(
        interval_secs = config.interval_secs,
        auto_repair = config.auto_repair,
        "maintenance worker started"
    );

    loop {
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = shutdown.changed() => {
                info!("maintenance worker shutting down");
                break;
            }
        }

        if *shutdown.borrow() {
            break;
        }

        match run_diagnostics(&pool, &config).await {
            Ok(diag) => {
                let recs = diag.recommendations.len();
                if recs > 0 {
                    info!(
                        recommendations = recs,
                        tombstone_ratio = format!("{:.1}%", diag.tombstone_ratio),
                        dead_ratio = format!("{:.1}%", diag.dead_ratio),
                        stale_leases = diag.stale_leases,
                        "maintenance diagnostics collected"
                    );

                    if config.auto_repair {
                        // TODO: Execute repair actions, track failures,
                        // halt after max_repair_failures
                    }
                }
            }
            Err(e) => {
                warn!(err = %e, "maintenance diagnostics failed");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Collect health diagnostics from the database.
pub async fn run_diagnostics(
    pool: &DbPool,
    config: &MaintenanceConfig,
) -> Result<Diagnostics, String> {
    let stale_timeout = config.stale_lease_timeout_secs;
    let dead_thresh = config.dead_job_threshold_pct;
    let tombstone_thresh = config.tombstone_threshold_pct;

    pool.read(move |conn| {
        // Memory stats
        let total_memories: usize = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap_or(0);
        let deleted_memories: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE deleted = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tombstone_ratio = if total_memories > 0 {
            (deleted_memories as f64 / total_memories as f64) * 100.0
        } else {
            0.0
        };

        // Job stats
        let total_jobs: usize = conn
            .query_row("SELECT COUNT(*) FROM memory_jobs", [], |r| r.get(0))
            .unwrap_or(0);
        let pending_jobs: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_jobs WHERE status = 'pending'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let leased_jobs: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_jobs WHERE status = 'leased'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let dead_jobs: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_jobs WHERE status = 'dead'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let dead_ratio = if total_jobs > 0 {
            (dead_jobs as f64 / total_jobs as f64) * 100.0
        } else {
            0.0
        };

        // Stale leases
        let stale_leases: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_jobs WHERE status = 'leased' AND leased_at < datetime('now', ?1)",
                rusqlite::params![format!("-{stale_timeout} seconds")],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Entity stats
        let total_entities: usize = conn
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .unwrap_or(0);
        let orphan_entities: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM entities e LEFT JOIN memory_entity_mentions m ON m.entity_id = e.id WHERE m.entity_id IS NULL AND e.pinned = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // FTS consistency check (sample)
        let fts_count: usize = conn
            .query_row("SELECT COUNT(*) FROM memories_fts", [], |r| r.get(0))
            .unwrap_or(0);
        let active_count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE deleted = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let fts_mismatch = (fts_count as i64 - active_count as i64).unsigned_abs() > 10;

        // Embedding gaps
        let embedding_gaps: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memories m LEFT JOIN memory_embeddings e ON e.memory_id = m.id WHERE m.deleted = 0 AND e.memory_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // Build recommendations
        let mut recommendations = Vec::new();

        if dead_ratio > dead_thresh {
            recommendations.push(RepairRecommendation {
                action: "requeue_dead".into(),
                reason: format!("dead job ratio {dead_ratio:.1}% exceeds {dead_thresh:.1}%"),
                severity: "warning".into(),
            });
        }

        if tombstone_ratio > tombstone_thresh {
            recommendations.push(RepairRecommendation {
                action: "retention_sweep".into(),
                reason: format!(
                    "tombstone ratio {tombstone_ratio:.1}% exceeds {tombstone_thresh:.1}%"
                ),
                severity: "warning".into(),
            });
        }

        if stale_leases > 0 {
            recommendations.push(RepairRecommendation {
                action: "release_leases".into(),
                reason: format!("{stale_leases} stale leases detected"),
                severity: "info".into(),
            });
        }

        if fts_mismatch {
            recommendations.push(RepairRecommendation {
                action: "check_fts".into(),
                reason: format!("FTS count ({fts_count}) != active memory count ({active_count})"),
                severity: "warning".into(),
            });
        }

        if embedding_gaps > 0 {
            recommendations.push(RepairRecommendation {
                action: "re_embed".into(),
                reason: format!("{embedding_gaps} memories missing embeddings"),
                severity: "info".into(),
            });
        }

        if orphan_entities > 0 {
            recommendations.push(RepairRecommendation {
                action: "clean_orphans".into(),
                reason: format!("{orphan_entities} orphan entities"),
                severity: "info".into(),
            });
        }

        Ok(Diagnostics {
            total_memories,
            deleted_memories,
            tombstone_ratio,
            total_jobs,
            pending_jobs,
            leased_jobs,
            dead_jobs,
            dead_ratio,
            stale_leases,
            total_entities,
            orphan_entities,
            fts_mismatch,
            embedding_gaps,
            recommendations,
        })
    })
    .await
    .map_err(|e| e.to_string())
}
