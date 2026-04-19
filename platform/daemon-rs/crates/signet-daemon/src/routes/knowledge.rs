//! Knowledge graph route handlers.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use signet_services::graph;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListParams {
    pub agent_id: Option<String>,
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn list_entities(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let entity_type = params.entity_type;
    let q = params.q;

    let result = state
        .pool
        .read(move |conn| {
            let items = graph::list_knowledge_entities(
                conn,
                &agent_id,
                entity_type.as_deref(),
                q.as_deref(),
                limit,
                offset,
            )?;
            Ok(serde_json::json!({
                "items": items,
                "limit": limit,
                "offset": offset,
            }))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities/:id
// ---------------------------------------------------------------------------

pub async fn get_entity_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .read(move |conn| {
            let entity = signet_core::queries::entity::get(conn, &id)?;
            let Some(entity) = entity else {
                return Ok(serde_json::json!({"_code": 404}));
            };
            let density = graph::get_structural_density(conn, &id, &agent_id)?;
            let incoming: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM entity_dependencies WHERE target_entity_id = ?1 AND agent_id = ?2",
                    rusqlite::params![id, agent_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let outgoing: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM entity_dependencies WHERE source_entity_id = ?1 AND agent_id = ?2",
                    rusqlite::params![id, agent_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            Ok(serde_json::json!({
                "entity": entity,
                "aspectCount": density.aspect_count,
                "attributeCount": density.attribute_count,
                "constraintCount": density.constraint_count,
                "dependencyCount": density.dependency_count,
                "structuralDensity": density,
                "incomingDependencyCount": incoming,
                "outgoingDependencyCount": outgoing,
            }))
        })
        .await;

    match result {
        Ok(val) => {
            let code = val.get("_code").and_then(|c| c.as_u64());
            if code == Some(404) {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "entity not found"})),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(val)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities/:id/aspects
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AgentIdParam {
    pub agent_id: Option<String>,
}

pub async fn get_aspects(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .read(move |conn| {
            let items = graph::get_aspects_with_counts(conn, &id, &agent_id)?;
            Ok(serde_json::json!({"items": items}))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities/:id/aspects/:aspectId/attributes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AttributeFilterParams {
    pub agent_id: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn get_attributes(
    State(state): State<Arc<AppState>>,
    Path((entity_id, aspect_id)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<AttributeFilterParams>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let kind = params.kind;
    let status = params.status;

    let result = state
        .pool
        .read(move |conn| {
            let items = graph::get_attributes_filtered(
                conn,
                &graph::AttributeFilter {
                    entity_id: &entity_id,
                    aspect_id: &aspect_id,
                    agent_id: &agent_id,
                    kind: kind.as_deref(),
                    status: status.as_deref(),
                    limit,
                    offset,
                },
            )?;
            Ok(serde_json::json!({
                "items": items,
                "limit": limit,
                "offset": offset,
            }))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities/:id/dependencies
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct DependencyParams {
    pub agent_id: Option<String>,
    pub direction: Option<String>,
}

pub async fn get_dependencies(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<DependencyParams>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());
    let direction = params.direction.unwrap_or_else(|| "both".into());

    let result = state
        .pool
        .read(move |conn| {
            let items = graph::get_dependencies_detailed(conn, &id, &agent_id, &direction)?;
            Ok(serde_json::json!({"items": items}))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// POST /api/knowledge/entities/:id/pin
// DELETE /api/knowledge/entities/:id/pin
// ---------------------------------------------------------------------------

pub async fn pin_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .write(signet_core::db::Priority::High, move |conn| {
            let entity = signet_core::queries::entity::get(conn, &id)?;
            if entity.is_none() {
                return Ok(serde_json::json!({"_code": 404}));
            }
            let ts = chrono::Utc::now().to_rfc3339();
            signet_core::queries::entity::pin(conn, &id, &agent_id, &ts)?;
            Ok(serde_json::json!({"pinned": true, "pinnedAt": ts}))
        })
        .await;

    match result {
        Ok(val) => {
            if val.get("_code").and_then(|c| c.as_u64()) == Some(404) {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "entity not found"})),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(val)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn unpin_entity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .write(signet_core::db::Priority::High, move |conn| {
            let ts = chrono::Utc::now().to_rfc3339();
            signet_core::queries::entity::unpin(conn, &id, &agent_id, &ts)?;
            Ok(serde_json::json!({"pinned": false}))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/entities/pinned
// ---------------------------------------------------------------------------

pub async fn list_pinned(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .read(move |conn| {
            let entities = signet_core::queries::entity::list_pinned(conn, &agent_id)?;
            let items: Vec<serde_json::Value> = entities
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "name": e.name,
                        "pinnedAt": e.pinned_at,
                    })
                })
                .collect();
            Ok(serde_json::json!(items))
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/stats
// ---------------------------------------------------------------------------

pub async fn stats(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .read(move |conn| {
            let stats = graph::get_knowledge_stats(conn, &agent_id)?;
            Ok(serde_json::to_value(stats).unwrap_or_default())
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/knowledge/constellation
// ---------------------------------------------------------------------------

pub async fn constellation(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<AgentIdParam>,
) -> axum::response::Response {
    let agent_id = params.agent_id.unwrap_or_else(|| "default".into());

    let result = state
        .pool
        .read(move |conn| {
            let graph = graph::get_constellation(conn, &agent_id)?;
            Ok(serde_json::to_value(graph).unwrap_or_default())
        })
        .await;

    match result {
        Ok(val) => (StatusCode::OK, Json(val)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
