//! MCP HTTP transport handler.
//!
//! Handles JSON-RPC requests at `/mcp` endpoint.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::protocol::{
    JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION, PARSE_ERROR, SERVER_NAME,
};
use super::tools;
use crate::state::AppState;

/// POST /mcp — handle MCP JSON-RPC requests.
pub async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Parse as JSON-RPC request
    let rpc: JsonRpcRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(
                    serde_json::to_value(JsonRpcResponse::error(
                        None,
                        PARSE_ERROR,
                        format!("invalid JSON-RPC: {e}"),
                    ))
                    .unwrap(),
                ),
            );
        }
    };

    let response = dispatch(&state, &rpc).await;
    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap()),
    )
}

async fn dispatch(state: &Arc<AppState>, req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "initialized" => {
            // Notification — no response needed, but we send an ack
            JsonRpcResponse::success(req.id.clone(), serde_json::json!({}))
        }
        "tools/list" => handle_tools_list(req),
        "tools/call" => handle_tools_call(state, req).await,
        "ping" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({})),
        _ => JsonRpcResponse::method_not_found(req.id.clone(), &req.method),
    }
}

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": false,
                },
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            },
        }),
    )
}

fn handle_tools_list(req: &JsonRpcRequest) -> JsonRpcResponse {
    let defs = tools::definitions();
    let tools: Vec<serde_json::Value> = defs
        .into_iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            })
        })
        .collect();

    JsonRpcResponse::success(req.id.clone(), serde_json::json!({ "tools": tools }))
}

async fn handle_tools_call(state: &Arc<AppState>, req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match &req.params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                -32602,
                "missing params for tools/call".into(),
            );
        }
    };

    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                -32602,
                "missing tool name in params".into(),
            );
        }
    };

    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = tools::execute(state, name, &args).await;

    JsonRpcResponse::success(req.id.clone(), serde_json::to_value(result).unwrap())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_response() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "initialize".into(),
            params: None,
        };
        let resp = handle_initialize(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn tools_list_response() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(2)),
            method: "tools/list".into(),
            params: None,
        };
        let resp = handle_tools_list(&req);
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 23);
    }
}
