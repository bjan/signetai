//! Predictor status, training, and comparison routes.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;

use crate::state::AppState;

/// GET /api/predictor/status
pub async fn status(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Predictor is currently fail-open / not yet integrated
    Json(serde_json::json!({
        "enabled": false,
        "status": null,
    }))
}

#[derive(Deserialize)]
pub struct ComparisonQuery {
    #[serde(default = "default_agent_id")]
    pub agent_id: String,
    pub project: Option<String>,
    #[allow(dead_code)] // Used by comparisons/by-entity endpoint
    pub entity_id: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_agent_id() -> String {
    "default".into()
}
fn default_limit() -> i64 {
    50
}

/// GET /api/predictor/comparisons
pub async fn comparisons(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ComparisonQuery>,
) -> Json<serde_json::Value> {
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);
    let agent = q.agent_id.clone();
    let project = q.project.clone();
    let since = q.since.clone();
    let until = q.until.clone();

    let result = state
        .pool
        .read(move |conn| {
            let mut sql = String::from(
                "SELECT session_key, predictor_ndcg, baseline_ndcg, predictor_won, margin, alpha, candidate_count, created_at
                 FROM predictor_comparisons WHERE agent_id = ?1",
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                vec![Box::new(agent.clone())];
            let mut idx = 2;

            if let Some(ref p) = project {
                sql.push_str(&format!(" AND project = ?{idx}"));
                params.push(Box::new(p.clone()));
                idx += 1;
            }

            if let Some(ref s) = since {
                sql.push_str(&format!(" AND created_at >= ?{idx}"));
                params.push(Box::new(s.clone()));
                idx += 1;
            }

            if let Some(ref u) = until {
                sql.push_str(&format!(" AND created_at <= ?{idx}"));
                params.push(Box::new(u.clone()));
                idx += 1;
            }

            // Count
            let count_sql = format!(
                "SELECT COUNT(*) FROM ({}) t",
                sql.replace("SELECT session_key, predictor_ndcg, baseline_ndcg, predictor_won, margin, alpha, candidate_count, created_at", "SELECT 1")
            );
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let total: i64 = conn
                .query_row(&count_sql, param_refs.as_slice(), |r| r.get(0))
                .unwrap_or(0);

            // Paginated results
            sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{idx}"));
            params.push(Box::new(limit));
            idx += 1;
            sql.push_str(&format!(" OFFSET ?{idx}"));
            params.push(Box::new(offset));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let items: Vec<serde_json::Value> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok(serde_json::json!({
                        "session_key": row.get::<_, String>(0)?,
                        "predictor_ndcg": row.get::<_, f64>(1)?,
                        "baseline_ndcg": row.get::<_, f64>(2)?,
                        "predictor_won": row.get::<_, i64>(3)?,
                        "margin": row.get::<_, f64>(4)?,
                        "alpha": row.get::<_, f64>(5)?,
                        "candidate_count": row.get::<_, i64>(6)?,
                        "created_at": row.get::<_, String>(7)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(serde_json::json!({
                "total": total,
                "limit": limit,
                "offset": offset,
                "items": items,
            }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"total": 0, "items": []}));

    Json(result)
}

/// GET /api/predictor/comparisons/by-project
pub async fn comparisons_by_project(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ComparisonQuery>,
) -> Json<serde_json::Value> {
    let agent = q.agent_id.clone();
    let since = q.since.clone();

    let result = state
        .pool
        .read(move |conn| {
            let mut sql = String::from(
                "SELECT project, SUM(predictor_won) as pw, COUNT(*) - SUM(predictor_won) as bw,
                 CAST(SUM(predictor_won) AS REAL) / COUNT(*) as wr, AVG(margin) as am
                 FROM predictor_comparisons WHERE agent_id = ?1",
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(agent.clone())];

            if let Some(ref s) = since {
                sql.push_str(" AND created_at >= ?2");
                params.push(Box::new(s.clone()));
            }

            sql.push_str(" GROUP BY project ORDER BY wr DESC");

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let items: Vec<serde_json::Value> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok(serde_json::json!({
                        "project": row.get::<_, Option<String>>(0)?,
                        "predictorWins": row.get::<_, i64>(1)?,
                        "baselineWins": row.get::<_, i64>(2)?,
                        "winRate": row.get::<_, f64>(3)?,
                        "avgMargin": row.get::<_, f64>(4)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(serde_json::json!({ "items": items }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"items": []}));

    Json(result)
}

/// GET /api/predictor/comparisons/by-entity
pub async fn comparisons_by_entity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ComparisonQuery>,
) -> Json<serde_json::Value> {
    let agent = q.agent_id.clone();
    let since = q.since.clone();

    let result = state
        .pool
        .read(move |conn| {
            let mut sql = String::from(
                "SELECT entity_id, SUM(predictor_won) as pw, COUNT(*) - SUM(predictor_won) as bw,
                 CAST(SUM(predictor_won) AS REAL) / COUNT(*) as wr, AVG(margin) as am
                 FROM predictor_comparisons WHERE agent_id = ?1 AND entity_id IS NOT NULL",
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(agent.clone())];

            if let Some(ref s) = since {
                sql.push_str(" AND created_at >= ?2");
                params.push(Box::new(s.clone()));
            }

            sql.push_str(" GROUP BY entity_id ORDER BY wr DESC");

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let items: Vec<serde_json::Value> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok(serde_json::json!({
                        "entity_id": row.get::<_, Option<String>>(0)?,
                        "predictorWins": row.get::<_, i64>(1)?,
                        "baselineWins": row.get::<_, i64>(2)?,
                        "winRate": row.get::<_, f64>(3)?,
                        "avgMargin": row.get::<_, f64>(4)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(serde_json::json!({ "items": items }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"items": []}));

    Json(result)
}

#[derive(Deserialize)]
pub struct TrainingQuery {
    #[serde(default = "default_agent_id")]
    pub agent_id: String,
    #[serde(default = "default_training_limit")]
    pub limit: i64,
}

fn default_training_limit() -> i64 {
    20
}

/// GET /api/predictor/training
pub async fn training(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrainingQuery>,
) -> Json<serde_json::Value> {
    let agent = q.agent_id.clone();
    let limit = q.limit.clamp(1, 100);

    let result = state
        .pool
        .read(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT timestamp, model_version, loss, sample_count, duration_ms
                 FROM predictor_training_runs WHERE agent_id = ?1
                 ORDER BY timestamp DESC LIMIT ?2",
            )?;
            let items: Vec<serde_json::Value> = stmt
                .query_map(rusqlite::params![agent, limit], |row| {
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "modelVersion": row.get::<_, i64>(1)?,
                        "loss": row.get::<_, f64>(2)?,
                        "sampleCount": row.get::<_, i64>(3)?,
                        "durationMs": row.get::<_, i64>(4)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(serde_json::json!({ "items": items }))
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({"items": []}));

    Json(result)
}

/// GET /api/predictor/training-pairs-count
pub async fn training_pairs_count(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let count = state
        .pool
        .read(|conn| {
            Ok(conn
                .query_row("SELECT COUNT(*) FROM predictor_training_pairs", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or(0))
        })
        .await
        .unwrap_or(0);

    Json(serde_json::json!({ "count": count }))
}

/// POST /api/predictor/train — trigger training.
pub async fn train(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    // Predictor not yet integrated
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "predictor not enabled",
        })),
    )
}
