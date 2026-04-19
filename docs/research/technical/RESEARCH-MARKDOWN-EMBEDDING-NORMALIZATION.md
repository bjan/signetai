---
title: RESEARCH-MARKDOWN-EMBEDDING-NORMALIZATION
question: How should Signet preserve structured markdown for embeddings while preventing repeated poison-pill retries from malformed or provider-sensitive payloads?
date: 2026-03-30
---

# RESEARCH-MARKDOWN-EMBEDDING-NORMALIZATION

## Trigger

Issue #418 reports that imported OpenClaw Session Logs markdown tables can
be flattened onto a single line before embedding. The flattened payload
causes Ollama `/api/embeddings` to return HTTP 500 for
`mxbai-embed-large`, and the daemon continues retrying the same row,
creating repeated warning spam.

## Observed code paths

- `platform/daemon/src/content-normalization.ts`
- `platform/native/src/normalization.rs`
- `platform/daemon-rs/crates/signet-services/src/normalize.rs`
- `platform/daemon/src/embedding-tracker.ts`
- `platform/daemon/src/daemon.ts`

The current normalization contract collapses all whitespace for storage,
which includes real newlines. That is safe for plain prose dedupe, but it
destroys markdown table structure before embedding and makes provider
behavior depend on an implementation detail rather than on the original
user content.

## Findings

1. **Storage normalization is too aggressive**
   - `replace(/\s+/g, " ")` converts multiline markdown into one line.
   - The native Rust addon and daemon-rs shadow normalization mirror the
     same behavior, so the bug exists across parity paths.

2. **The dedupe goal is valid, but belongs in semantic normalization**
   - Deduplication wants whitespace-insensitive comparison and hashing.
   - Embedding and display want preserved line structure.
   - These are different contracts and should not share the same
     normalization step.

3. **Retry behavior lacks durable suppression**
   - The embedding tracker continues revisiting rows with no embedding.
   - A persistent provider-sensitive payload can therefore generate
     recurring warnings indefinitely.

## Decision direction

Split the normalization contract into two layers:

- **Storage content**: preserve line structure, normalize line endings, trim
  outer whitespace only.
- **Normalized/hash content**: collapse semantic whitespace and strip
  trailing punctuation for dedupe and equality.

Then add bounded retry suppression in the embedding tracker so a single bad
payload cannot create continuous warning spam.

## Implications

- Regression tests are required for multiline markdown tables.
- Rust parity must be updated in both `platform/native` and
  `platform/daemon-rs`.
- The incident should leave a durable guardrail in the spec system and the
  test suite.
