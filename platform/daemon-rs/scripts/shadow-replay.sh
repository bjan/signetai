#!/usr/bin/env bash
#
# Shadow replay setup: runs TS daemon (primary :3850) and Rust daemon
# (shadow :3851) with the shadow proxy (:3849) comparing responses.
#
# Usage:
#   ./scripts/shadow-replay.sh start    # start all three processes
#   ./scripts/shadow-replay.sh stop     # stop all processes
#   ./scripts/shadow-replay.sh analyze  # analyze divergence log

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DAEMON_RS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$DAEMON_RS_DIR/../.." && pwd)"

AGENTS_DIR="${SIGNET_PATH:-$HOME/.agents}"
SHADOW_DB_DIR="$AGENTS_DIR/.shadow-replay"
SHADOW_DB="$SHADOW_DB_DIR/memory/memories.db"
LIVE_DB="$AGENTS_DIR/memory/memories.db"

RUST_DAEMON="$DAEMON_RS_DIR/target/release/signet-daemon"
RUST_SHADOW="$DAEMON_RS_DIR/target/release/signet-shadow"

PID_DIR="$AGENTS_DIR/.daemon"

case "${1:-help}" in
  start)
    echo "=== Shadow Replay Setup ==="
    echo ""

    # Build Rust binaries
    echo "Building Rust daemon (release)..."
    (cd "$DAEMON_RS_DIR" && cargo build --release -p signet-daemon -p signet-shadow --quiet)

    # Create isolated shadow DB directory
    echo "Setting up isolated shadow DB..."
    mkdir -p "$SHADOW_DB_DIR/memory"
    mkdir -p "$SHADOW_DB_DIR/.daemon/logs"

    # Copy live DB to shadow (isolated copy)
    if [ -f "$LIVE_DB" ]; then
      cp "$LIVE_DB" "$SHADOW_DB"
      # Also copy WAL if exists
      [ -f "$LIVE_DB-wal" ] && cp "$LIVE_DB-wal" "$SHADOW_DB-wal" || true
      [ -f "$LIVE_DB-shm" ] && cp "$LIVE_DB-shm" "$SHADOW_DB-shm" || true
      echo "Copied live DB to shadow: $(du -h "$SHADOW_DB" | cut -f1)"
    else
      echo "WARNING: No live DB found at $LIVE_DB. Shadow will use fresh DB."
    fi

    # Copy agent.yaml to shadow
    [ -f "$AGENTS_DIR/agent.yaml" ] && cp "$AGENTS_DIR/agent.yaml" "$SHADOW_DB_DIR/" || true

    # Start TS daemon on :3850 (if not already running)
    if curl -s http://localhost:3850/health | grep -q '"ok"' 2>/dev/null; then
      echo "TS daemon already running on :3850"
    else
      echo "Starting TS daemon on :3850..."
      (cd "$REPO_ROOT/platform/daemon" && bun src/daemon.ts &) 2>/dev/null
      echo $! > "$PID_DIR/ts-daemon.pid"
      sleep 3
    fi

    # Start Rust daemon on :3851 (shadow, isolated DB)
    echo "Starting Rust daemon on :3851 (shadow)..."
    SIGNET_PATH="$SHADOW_DB_DIR" SIGNET_PORT=3851 SIGNET_BIND=127.0.0.1 \
      "$RUST_DAEMON" &
    echo $! > "$PID_DIR/rust-shadow.pid"
    sleep 2

    # Verify both daemons
    echo ""
    echo "Verifying daemons..."
    echo -n "  TS  (:3850): "
    curl -s http://localhost:3850/health | head -c 100
    echo ""
    echo -n "  Rust (:3851): "
    curl -s http://localhost:3851/health | head -c 100
    echo ""

    # Start shadow proxy on :3849
    echo ""
    echo "Starting shadow proxy on :3849..."
    SIGNET_PARITY_RULES="$DAEMON_RS_DIR/contracts/parity-rules.json" \
      "$RUST_SHADOW" \
        --proxy-port 3849 \
        --primary-port 3850 \
        --shadow-port 3851 &
    echo $! > "$PID_DIR/shadow-proxy.pid"
    sleep 1

    echo ""
    echo "=== Shadow Replay Running ==="
    echo ""
    echo "  Proxy (use this):  http://localhost:3849"
    echo "  Primary (TS):      http://localhost:3850"
    echo "  Shadow (Rust):     http://localhost:3851"
    echo ""
    echo "  Divergence log:    $AGENTS_DIR/.daemon/logs/shadow-divergences.jsonl"
    echo ""
    echo "  To stop:    $0 stop"
    echo "  To analyze: $0 analyze"
    echo ""
    echo "Point your connectors/tools at port 3849 instead of 3850 to"
    echo "exercise shadow comparison during normal usage."
    ;;

  stop)
    echo "Stopping shadow replay processes..."
    for pidfile in "$PID_DIR/shadow-proxy.pid" "$PID_DIR/rust-shadow.pid" "$PID_DIR/ts-daemon.pid"; do
      if [ -f "$pidfile" ]; then
        pid=$(cat "$pidfile")
        if kill -0 "$pid" 2>/dev/null; then
          kill "$pid" 2>/dev/null || true
          echo "  Stopped PID $pid ($(basename "$pidfile" .pid))"
        fi
        rm -f "$pidfile"
      fi
    done
    echo "Done."
    ;;

  analyze)
    LOG="$AGENTS_DIR/.daemon/logs/shadow-divergences.jsonl"
    if [ ! -f "$LOG" ]; then
      echo "No divergence log found at $LOG"
      echo "Start shadow replay first: $0 start"
      exit 1
    fi
    "$RUST_SHADOW" --analyze --log "$LOG"
    ;;

  *)
    echo "Usage: $0 {start|stop|analyze}"
    echo ""
    echo "  start    Start TS daemon, Rust daemon, and shadow proxy"
    echo "  stop     Stop all shadow replay processes"
    echo "  analyze  Analyze divergence log"
    exit 1
    ;;
esac
