#!/usr/bin/env bash
# Build signet from source (Termux-compatible — skips tsc type declarations)
set -e

cd "$(dirname "$0")"

echo "=== core ==="
cd packages/core
bun build ./src/index.ts --outdir ./dist --target node --external better-sqlite3
bun build ./src/pipeline-providers.ts --outdir ./dist --target browser
cd ../..

echo "=== connector-base ==="
cd packages/connector-base
bun build ./src/index.ts --outdir ./dist --target node --external better-sqlite3
cd ../..

for conn in claude-code codex forge gemini hermes-agent openclaw opencode oh-my-pi pi; do
  echo "=== connector-$conn ==="
  cd "packages/connector-$conn"
  bun build src/index.ts --outdir dist --target node --external better-sqlite3
  cd ../..
done

echo "=== sdk ==="
cd packages/sdk
bun build ./src/index.ts ./src/react.tsx ./src/ai-sdk.ts ./src/openai.ts --outdir ./dist --target node --external react --external zod
cd ../..

echo "=== daemon + cli ==="
cd packages/signetai
bun run build:daemon
bun run build:cli
cd ../..

echo "=== dashboard ==="
cd packages/cli/dashboard
bunx svelte-kit sync 2>/dev/null || true
bunx vite build
cd ../../..

echo ""
echo "Built:"
ls -lh packages/signetai/dist/daemon.js packages/signetai/dist/cli.js packages/signetai/dist/mcp-stdio.js
echo "Dashboard: packages/cli/dashboard/build/"
