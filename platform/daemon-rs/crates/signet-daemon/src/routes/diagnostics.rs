//! Diagnostics and log routes.
//!
//! Health diagnostics, log listing, and version/update endpoints.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /api/diagnostics — composite health report
pub async fn report(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let result = state
        .pool
        .read(|conn| {
            let memories: i64 = conn
                .query_row("SELECT COUNT(*) FROM memories WHERE is_deleted = 0", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            let tombstones: i64 = conn
                .query_row("SELECT COUNT(*) FROM memories WHERE is_deleted = 1", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            let entities: i64 = conn
                .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
                .unwrap_or(0);
            let embeddings: i64 = conn
                .query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
                .unwrap_or(0);
            let pending_jobs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pipeline_jobs WHERE status = 'pending'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let dead_jobs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pipeline_jobs WHERE status = 'dead'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let total = memories + tombstones;
            let tombstone_ratio = if total > 0 {
                tombstones as f64 / total as f64
            } else {
                0.0
            };

            let embedding_coverage = if memories > 0 {
                embeddings as f64 / memories as f64
            } else {
                1.0
            };

            // Simple health score: 1.0 = perfect, 0.0 = critical
            let mut score = 1.0_f64;
            if tombstone_ratio > 0.3 {
                score -= 0.2;
            }
            if embedding_coverage < 0.8 {
                score -= 0.2;
            }
            if dead_jobs > 10 {
                score -= 0.1;
            }
            if pending_jobs > 100 {
                score -= 0.1;
            }

            Ok(serde_json::json!({
                "score": score.max(0.0),
                "status": if score > 0.7 { "healthy" } else if score > 0.4 { "degraded" } else { "critical" },
                "domains": {
                    "storage": {
                        "memories": memories,
                        "tombstones": tombstones,
                        "tombstoneRatio": tombstone_ratio,
                        "entities": entities,
                    },
                    "index": {
                        "embeddings": embeddings,
                        "coverage": embedding_coverage,
                    },
                    "queue": {
                        "pending": pending_jobs,
                        "dead": dead_jobs,
                    },
                }
            }))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        ),
    }
}

/// GET /api/diagnostics/:domain — single domain health
pub async fn domain(
    State(state): State<Arc<AppState>>,
    Path(domain): Path<String>,
) -> impl IntoResponse {
    // Reuse the full report and extract the domain
    let result = state
        .pool
        .read(move |conn| {
            let val = match domain.as_str() {
                "storage" => {
                    let memories: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM memories WHERE is_deleted = 0",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    let tombstones: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM memories WHERE is_deleted = 1",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    serde_json::json!({"memories": memories, "tombstones": tombstones, "score": 1.0})
                }
                "queue" => {
                    let pending: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM pipeline_jobs WHERE status = 'pending'",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    let dead: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM pipeline_jobs WHERE status = 'dead'",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    serde_json::json!({"pending": pending, "dead": dead, "score": 1.0})
                }
                "index" => {
                    let embeddings: i64 = conn
                        .query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
                        .unwrap_or(0);
                    serde_json::json!({"embeddings": embeddings, "score": 1.0})
                }
                _ => serde_json::json!({"error": "unknown domain"}),
            };
            Ok(val)
        })
        .await;

    match result {
        Ok(val) if val.get("error").is_some() => (StatusCode::NOT_FOUND, Json(val)),
        Ok(val) => (StatusCode::OK, Json(val)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        ),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // since filter planned
pub struct LogsQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub since: Option<String>,
}

/// GET /api/logs — list recent log entries
pub async fn logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let log_dir = state.config.logs_dir();

    // Read from latest log file
    let entries = tokio::task::spawn_blocking(move || {
        let mut files: Vec<_> = std::fs::read_dir(&log_dir)
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "log" || ext == "jsonl")
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();

        files.sort_by(|a, b| {
            b.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .cmp(
                    &a.metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                )
        });

        let mut logs = Vec::new();
        for file in files.into_iter().take(3) {
            if let Ok(content) = std::fs::read_to_string(file.path()) {
                for line in content.lines().rev().take(limit) {
                    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(ref level) = params.level
                            && entry
                                .get("level")
                                .and_then(|v| v.as_str())
                                .map(|l| !l.eq_ignore_ascii_case(level))
                                .unwrap_or(true)
                        {
                            continue;
                        }
                        logs.push(entry);
                    }
                    if logs.len() >= limit {
                        break;
                    }
                }
            }
            if logs.len() >= limit {
                break;
            }
        }
        logs
    })
    .await
    .unwrap_or_default();

    let count = entries.len();
    (
        StatusCode::OK,
        Json(serde_json::json!({"logs": entries, "count": count})),
    )
}

/// GET /api/version — version info
pub async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "runtime": "rust",
        "target": std::env::consts::ARCH,
    }))
}

/// GET /api/update — update status
pub async fn update_status() -> Json<serde_json::Value> {
    // Update checking requires npm registry calls — stub
    Json(serde_json::json!({
        "available": false,
        "current": env!("CARGO_PKG_VERSION"),
    }))
}
