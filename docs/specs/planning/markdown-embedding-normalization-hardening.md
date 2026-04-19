---
id: markdown-embedding-normalization-hardening
title: Markdown Embedding Normalization Hardening
status: planning
informed_by:
  - docs/research/technical/RESEARCH-MARKDOWN-EMBEDDING-NORMALIZATION.md
---

# Markdown Embedding Normalization Hardening

## Why this exists

Issue #418 exposed a contract bug in memory normalization. Signet currently
collapses all whitespace before storing memory content, which flattens
multiline markdown tables into a single line before embedding. Some
embedding providers reject that malformed shape, and the embedding tracker
can keep retrying the same row, creating warning spam.

This is an incident-driven planning stub. The implementation must harden
both content handling and retry behavior.

## Scope

- Preserve multiline markdown structure in stored memory content.
- Keep dedupe/hash behavior whitespace-insensitive.
- Mirror the normalization change in Rust parity paths.
- Add bounded failure suppression in the embedding tracker.
- Add regression tests for multiline markdown tables and retry suppression.

## Non-goals

- No new user-facing configuration surface.
- No schema migration unless implementation proves one is required.
- No provider-specific markdown table parser beyond what is needed to stop
  flattening and uncontrolled retries.

## Contracts

### Content normalization

- `storageContent` must preserve real line breaks.
- `normalizedContent` and `contentHash` may collapse semantic whitespace for
  dedupe.
- Storage normalization and semantic normalization are distinct contracts and
  must not be merged again.

### Retry behavior

- Persistent embedding failures must not trigger unbounded repeated attempts
  on every tracker cycle.
- Failure logs must include row context sufficient to identify the bad
  payload.

### Parity

- `platform/daemon/src/content-normalization.ts`
- `platform/native/src/normalization.rs`
- `platform/daemon-rs/crates/signet-services/src/normalize.rs`

All three paths must preserve equivalent normalization semantics.

## Success criteria

1. Multiline markdown tables remain multiline in stored memory content and
   during embedding requests.
2. Whitespace-only formatting differences still dedupe to the same semantic
   hash.
3. Repeated embedding failures are retried with bounded suppression and do
   not generate warning spam every tracker cycle.
