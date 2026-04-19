//! Timeline debugging routes: activity timeline and incident investigation.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::state::AppState;

/// GET /api/memory/timeline — activity timeline by bucket.
pub async fn activity(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let result = state
        .pool
        .read(|conn| {
            let total: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE deleted = 0",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let total_history: i64 = conn
                .query_row("SELECT COUNT(*) FROM memory_history", [], |r| r.get(0))
                .unwrap_or(0);

            // Bucket: today
            let today: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE deleted = 0 AND created_at >= datetime('now', 'start of day')",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            // Bucket: this week
            let week: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE deleted = 0 AND created_at >= datetime('now', '-7 days')",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            // Bucket: this month
            let month: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE deleted = 0 AND created_at >= datetime('now', '-30 days')",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            Ok(serde_json::json!({
                "generatedAt": chrono::Utc::now().to_rfc3339(),
                "totalMemories": total,
                "totalHistoryEvents": total_history,
                "buckets": [
                    { "label": "Today", "count": today },
                    { "label": "Last 7 days", "count": week },
                    { "label": "Last 30 days", "count": month },
                ]
            }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"error": "db unavailable"}));

    Json(result)
}

/// GET /api/timeline/{id} — incident timeline for a memory/request/session.
pub async fn incident(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let entity_id = id.clone();

    let result = state
        .pool
        .read(move |conn| {
            // Determine entity type
            let entity_type = if conn
                .query_row(
                    "SELECT 1 FROM memories WHERE id = ?1",
                    rusqlite::params![entity_id],
                    |_| Ok(()),
                )
                .is_ok()
            {
                "memory"
            } else if conn
                .query_row(
                    "SELECT 1 FROM session_checkpoints WHERE session_key = ?1 LIMIT 1",
                    rusqlite::params![entity_id],
                    |_| Ok(()),
                )
                .is_ok()
            {
                "session"
            } else {
                "unknown"
            };

            let mut events = Vec::new();

            // History events
            let mut stmt = conn.prepare_cached(
                "SELECT created_at, action, changed_by, field_changed, old_value, new_value
                 FROM memory_history WHERE memory_id = ?1 ORDER BY created_at ASC",
            )?;
            let history: Vec<serde_json::Value> = stmt
                .query_map(rusqlite::params![entity_id], |row| {
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "source": "history",
                        "event": format!("memory:{}", row.get::<_, String>(1)?),
                        "details": {
                            "changedBy": row.get::<_, Option<String>>(2)?,
                            "field": row.get::<_, Option<String>>(3)?,
                            "hasOldContent": row.get::<_, Option<String>>(4)?.is_some(),
                            "hasNewContent": row.get::<_, Option<String>>(5)?.is_some(),
                        }
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            events.extend(history);

            // Job events
            let mut stmt = conn.prepare_cached(
                "SELECT created_at, job_type, status, completed_at
                 FROM memory_jobs WHERE memory_id = ?1 ORDER BY created_at ASC",
            )?;
            let jobs: Vec<serde_json::Value> = stmt
                .query_map(rusqlite::params![entity_id], |row| {
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "source": "job",
                        "event": format!("job:{}:{}", row.get::<_, String>(1)?, row.get::<_, String>(2)?),
                        "details": {
                            "completedAt": row.get::<_, Option<String>>(3)?,
                        }
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            events.extend(jobs);

            // Predictor comparisons (by session_key)
            let mut stmt = conn.prepare_cached(
                "SELECT created_at, predictor_ndcg, baseline_ndcg, margin, alpha, candidate_count
                 FROM predictor_comparisons WHERE session_key = ?1 ORDER BY created_at ASC",
            )?;
            let preds: Vec<serde_json::Value> = stmt
                .query_map(rusqlite::params![entity_id], |row| {
                    let p_ndcg: f64 = row.get(1)?;
                    let b_ndcg: f64 = row.get(2)?;
                    let won = if p_ndcg > b_ndcg { "won" } else { "lost" };
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "source": "predictor",
                        "event": format!("predictor:comparison:{won}"),
                        "details": {
                            "predictorNdcg": p_ndcg,
                            "baselineNdcg": b_ndcg,
                            "margin": row.get::<_, f64>(3)?,
                            "alpha": row.get::<_, f64>(4)?,
                            "candidateCount": row.get::<_, i64>(5)?,
                        }
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            events.extend(preds);

            // Sort by timestamp
            events.sort_by(|a, b| {
                let ta = a["timestamp"].as_str().unwrap_or("");
                let tb = b["timestamp"].as_str().unwrap_or("");
                ta.cmp(tb)
            });

            Ok(serde_json::json!({
                "entityType": entity_type,
                "entityId": entity_id,
                "events": events,
                "generatedAt": chrono::Utc::now().to_rfc3339(),
            }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"error": "db unavailable"}));

    Json(result)
}

/// GET /api/timeline/{id}/export — export incident timeline.
pub async fn export(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let timeline = incident(State(state), Path(id.clone())).await;

    Json(serde_json::json!({
        "meta": {
            "version": env!("CARGO_PKG_VERSION"),
            "exportedAt": chrono::Utc::now().to_rfc3339(),
            "entityId": id,
        },
        "timeline": timeline.0,
    }))
}
