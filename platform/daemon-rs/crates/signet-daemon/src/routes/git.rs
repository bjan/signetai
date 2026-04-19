//! Git sync routes.
//!
//! Status, push, pull, sync, and remote config endpoints.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Used when git config persistence is implemented
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitConfig {
    pub auto_sync: bool,
    pub sync_interval: u64,
    pub remote: String,
    pub branch: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn agents_dir(state: &AppState) -> std::path::PathBuf {
    state.config.base_path.clone()
}

async fn git_cmd(state: &AppState, args: &[&str]) -> Result<String, String> {
    let cwd = agents_dir(state);
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(&cwd)
        .output()
        .await
        .map_err(|e| format!("git exec: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("git error: {stderr}"))
    }
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /api/git/status
pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let branch = git_cmd(&state, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .unwrap_or_else(|_| "unknown".into());
    let clean = git_cmd(&state, &["status", "--porcelain"])
        .await
        .map(|s| s.is_empty())
        .unwrap_or(false);
    let remote = git_cmd(&state, &["remote", "get-url", "origin"])
        .await
        .unwrap_or_default();
    let last_commit = git_cmd(&state, &["log", "-1", "--format=%H %s"])
        .await
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "branch": branch,
            "clean": clean,
            "remote": remote,
            "lastCommit": last_commit,
            "initialized": !branch.is_empty() && branch != "unknown",
        })),
    )
}

/// POST /api/git/pull
pub async fn pull(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match git_cmd(&state, &["pull", "--rebase"]).await {
        Ok(output) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "output": output})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// POST /api/git/push
pub async fn push(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match git_cmd(&state, &["push"]).await {
        Ok(output) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "output": output})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// POST /api/git/sync — pull then push
pub async fn sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pull_result = git_cmd(&state, &["pull", "--rebase"]).await;
    let push_result = git_cmd(&state, &["push"]).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "pull": match &pull_result {
                Ok(o) => serde_json::json!({"success": true, "output": o}),
                Err(e) => serde_json::json!({"success": false, "error": e}),
            },
            "push": match &push_result {
                Ok(o) => serde_json::json!({"success": true, "output": o}),
                Err(e) => serde_json::json!({"success": false, "error": e}),
            },
        })),
    )
}

/// GET /api/git/config
pub async fn get_config(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Git config stored in agent.yaml — not yet parsed into a separate struct
    Json(serde_json::json!({
        "autoSync": false,
        "syncInterval": 300,
        "remote": "",
        "branch": "main",
    }))
}

/// POST /api/git/config — update git config
pub async fn set_config(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Config updates require writing to agent.yaml and reloading
    // Stub for now — full config hot-reload in Phase 8
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "git config update not yet implemented"})),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_git_config() {
        let config = GitConfig::default();
        assert!(!config.auto_sync);
        assert!(config.remote.is_empty());
    }
}
