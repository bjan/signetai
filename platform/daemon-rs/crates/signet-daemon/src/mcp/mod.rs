//! MCP (Model Context Protocol) server implementation.
//!
//! Implements JSON-RPC 2.0 over HTTP at `/mcp` with 23 built-in tools
//! that delegate to daemon services.

pub mod protocol;
pub mod tools;
pub mod transport;
