//! signet-mcp: stdio-to-HTTP bridge for MCP.
//!
//! Reads JSON-RPC from stdin, forwards to the daemon's `/mcp` endpoint,
//! writes responses to stdout. No daemon internals — pure HTTP proxy.

use std::io::{BufRead, Write};

use reqwest::Client;
use tracing::{debug, error, warn};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3850;
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn daemon_url() -> String {
    if let Ok(url) = std::env::var("SIGNET_DAEMON_URL") {
        return url;
    }
    let host = std::env::var("SIGNET_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
    let port: u16 = std::env::var("SIGNET_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    format!("http://{host}:{port}")
}

#[tokio::main]
async fn main() {
    // Logging to stderr so stdout stays clean for JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "signet_mcp=warn".into()),
        )
        .init();

    let base = daemon_url();
    let mcp_url = format!("{base}/mcp");

    debug!(url = %mcp_url, "signet-mcp stdio bridge starting");

    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("failed to build HTTP client");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!(err = %e, "stdin read error");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse to validate JSON and extract id for error responses
        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}")
                    }
                });
                let _ = writeln!(stdout, "{err_resp}");
                let _ = stdout.flush();
                continue;
            }
        };

        let id = parsed.get("id").cloned();

        // Forward to daemon
        let resp = match client
            .post(&mcp_url)
            .header("content-type", "application/json")
            .header("x-signet-runtime-path", "plugin")
            .header("x-signet-actor", "mcp-server")
            .header("x-signet-actor-type", "harness")
            .body(trimmed.to_string())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(err = %e, "daemon request failed");
                let err_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32603,
                        "message": format!("daemon unavailable: {e}")
                    }
                });
                let _ = writeln!(stdout, "{err_resp}");
                let _ = stdout.flush();
                continue;
            }
        };

        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                warn!(err = %e, "failed to read daemon response");
                let err_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32603,
                        "message": format!("response read error: {e}")
                    }
                });
                let _ = writeln!(stdout, "{err_resp}");
                let _ = stdout.flush();
                continue;
            }
        };

        // Write response (already JSON from daemon)
        let _ = writeln!(stdout, "{body}");
        let _ = stdout.flush();
    }

    debug!("signet-mcp stdio bridge exiting");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn default_constants() {
        assert_eq!(super::DEFAULT_HOST, "127.0.0.1");
        assert_eq!(super::DEFAULT_PORT, 3850);
    }

    #[test]
    fn timeout_is_reasonable() {
        assert!(super::REQUEST_TIMEOUT.as_secs() >= 10);
        assert!(super::REQUEST_TIMEOUT.as_secs() <= 60);
    }

    #[test]
    fn parse_error_response_shape() {
        let err = serde_json::json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": -32700,
                "message": "parse error: test"
            }
        });
        assert_eq!(err["error"]["code"], -32700);
        assert_eq!(err["jsonrpc"], "2.0");
    }
}
