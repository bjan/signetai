# Benchmark Specification

Reproducible benchmarks for validating Rust daemon performance against SLO targets.

## Dataset

Fixed dataset used across all benchmarks:
- 1,000 memories (mixed types: observation, fact, preference, decision)
- 50 entities (with relations, aspects, attributes)
- 10 sessions (with checkpoints and summaries)
- 500 embeddings (384-dimensional, matching nomic-embed-text)
- 20 pipeline jobs (mix of pending, processing, completed, dead)

Dataset fixture: `contracts/fixtures/bench-dataset.sql`

## Hardware

Document actual hardware in benchmark output:
- CPU model and core count
- RAM size
- Disk type (SSD/NVMe)
- OS and kernel version

## Methodology

1. **Cold run**: Start daemon, run benchmark once (discard results)
2. **Warm runs**: Run benchmark 3 times (warm SQLite page cache)
3. **Report**: Median of 5 total executions (1 cold + 3 warm + 1 final)

## SLO Targets

| Metric | Current (TS) | Target (Rust) | How Measured |
|--------|-------------|---------------|--------------|
| p50 recall latency | ~45ms | <15ms | POST /api/memory/recall with 10-word query, measure response time |
| p95 recall latency | ~120ms | <40ms | Same as above, 95th percentile over 100 requests |
| Search QPS (hybrid) | ~50/s | >200/s | Concurrent GET /memory/search, measure throughput |
| Pipeline throughput | ~3 jobs/s | >10 jobs/s | Enqueue 100 jobs, measure time to complete all |
| Cold startup time | ~2.5s | <500ms | `time signet-daemon --check-migrations` |
| Memory footprint (idle) | ~180MB | <50MB | RSS after startup + 1 minute idle |
| Binary size | N/A | <30MB | `ls -la target/release/signet-daemon` |

## Recall Latency Benchmark

```bash
# Setup: populate benchmark dataset
sqlite3 /tmp/bench.db < contracts/fixtures/bench-dataset.sql

# Run benchmark
SIGNET_PATH=/tmp/bench-signet cargo bench -p signet-core -- recall_latency

# Or manually:
SIGNET_PATH=/tmp/bench-signet signet-daemon &
PID=$!
sleep 1

for i in $(seq 1 100); do
  curl -s -w '%{time_total}\n' -o /dev/null \
    -X POST http://localhost:3850/api/memory/recall \
    -H 'Content-Type: application/json' \
    -d '{"query": "what did the user say about testing"}' >> /tmp/latencies.txt
done

kill $PID
sort -n /tmp/latencies.txt | awk 'NR==50{print "p50:", $0} NR==95{print "p95:", $0}'
```

## Search QPS Benchmark

```bash
# Concurrent search with wrk or similar
wrk -t4 -c16 -d10s 'http://localhost:3850/memory/search?q=testing+patterns&limit=10'
```

## Startup Benchmark

```bash
hyperfine --warmup 1 --runs 5 \
  'SIGNET_PATH=/tmp/bench-signet target/release/signet-daemon --check-migrations'
```

## Memory Footprint

```bash
SIGNET_PATH=/tmp/bench-signet target/release/signet-daemon &
PID=$!
sleep 60
ps -o rss= -p $PID | awk '{print $1/1024 "MB"}'
kill $PID
```
