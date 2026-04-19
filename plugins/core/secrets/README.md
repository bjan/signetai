# Signet Secrets Core Plugin

`signet.secrets` is the reference core plugin for the Signet plugin system.
It contributes daemon routes, CLI commands, MCP tools, dashboard metadata, SDK
surfaces, connector capabilities, prompt guidance, health checks, and audit
metadata for local encrypted secrets.

The plugin host infrastructure lives in `platform/daemon/src/plugins/`.
The current bundled implementation remains in
`platform/daemon/src/plugins/bundled/secrets.ts` until core plugins are split
into independently built plugin packages.
