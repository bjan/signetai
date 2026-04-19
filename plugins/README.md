# Signet Plugins

Plugins in this directory are loaded by Signet itself.

Use this rule when placing plugin code:

- `plugins/` contains Signet-native plugins loaded by the Signet plugin host.
- `integrations/<tool>/plugin/` contains plugins loaded by external tools to talk to Signet.
- `platform/daemon/src/plugins/` contains the plugin host, registry, audit, and manifest infrastructure.

Core plugins are privileged first-party capabilities that ship with Signet.
