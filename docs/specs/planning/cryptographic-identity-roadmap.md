---
title: "Cryptographic Identity Roadmap"
id: cryptographic-identity-roadmap
status: planning
informed_by: []
section: "Identity"
depends_on:
  - "multi-agent-support"
success_criteria:
  - "Identity model progresses toward cryptographic verification for agents and signed artifacts"
scope_boundary: "Key generation, artifact signing, and cross-machine verification for agent identity. Does not cover network-level auth protocols, blockchain integration, or federated identity standards."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Cryptographic Identity Roadmap

*SSH keys but for AI agents: sign outputs, verify identity, build
portable trust.*

---

## Problem Statement

Agent identity in Signet is currently defined by markdown files
(`SOUL.md`, `IDENTITY.md`) stored at `$SIGNET_WORKSPACE/agents/{name}/`.
The multi-agent system (complete) provides an agent roster in
`agent.yaml` with per-agent identity inheritance. But there is no
cryptographic backing. Any process that can write to the workspace
directory can impersonate any agent. There is no way to verify that
a memory, skill output, or session artifact was produced by a specific
agent. There is no mechanism for an agent's identity to be portable
across machines while remaining verifiable.

As agents become more autonomous and their outputs more consequential,
the trust gap between "this file says it's agent X" and "this artifact
is cryptographically signed by agent X" needs to close.

---

## Goals

1. Generate an Ed25519 keypair per agent, stored in `$SIGNET_WORKSPACE/.keys/{agent_id}/`.
2. Sign agent outputs (memories, skill results, session summaries) with the agent's private key.
3. Verify signatures on any machine that has the agent's public key.
4. Support agent passport export/import for cross-machine identity portability.
5. Integrate signature verification into the daemon's auth middleware for agent-scoped API calls.

---

## Proposed Capability Set

### A. Key Generation and Storage

On agent creation (or first daemon start after migration), generate
an Ed25519 keypair:

```
$SIGNET_WORKSPACE/
└── .keys/
    └── {agent_id}/
        ├── agent.key      # private key (0600 permissions)
        ├── agent.pub      # public key
        └── fingerprint    # SHA-256 fingerprint for display
```

- Key generation uses the `crypto` module (Node.js) or
  `Bun.CryptoHasher` under Bun. Ed25519 is chosen for small key/sig
  size (32B/64B) and speed.
- Private keys are never transmitted over HTTP. They remain on disk.
- The `agents` table (migration 043) gains a `public_key` column
  storing the base64-encoded public key for API-level verification.
- `GET /api/agents/:id/pubkey` returns the public key.

### B. Artifact Signing

Signed artifacts append a detached signature block. Three artifact
types are signed in v1:

1. **Memories**: when the pipeline writes a memory, the daemon signs
   `content + agent_id + created_at` and stores the signature in a
   new `signature` column on `memories`.
2. **Session summaries**: the summary worker signs the `.md` file
   content. Signature stored as a sidecar `.sig` file alongside the
   summary in `$SIGNET_WORKSPACE/.daemon/summaries/`.
3. **Skill outputs**: when a skill invocation produces a result, the
   result payload is signed before returning to the caller. Signature
   included in the response JSON as `_signature` field.

Signing is synchronous and fast (Ed25519 sign is <1ms). No latency
impact on the pipeline.

### C. Signature Verification

- `GET /api/verify` — accepts `{content, signature, agent_id}` and
  returns `{valid: boolean, agent_id, fingerprint}`.
- CLI: `signet verify --file path/to/artifact` reads the file and
  its sidecar `.sig`, resolves the agent's public key from the local
  roster or a provided pubkey file, and reports validity.
- Verification does not require the private key. Any machine with
  the public key can verify.

### D. Agent Passport

An agent passport is a portable identity bundle:

```json
{
  "version": 1,
  "agent_id": "researcher",
  "public_key": "base64...",
  "fingerprint": "SHA256:...",
  "identity": { "soul": "...", "identity_md": "..." },
  "created_at": "2026-03-24T00:00:00Z",
  "signed_by": "base64-signature-of-above-fields"
}
```

- `signet agent export <name>` — generates passport JSON, self-signed
  by the agent's own key.
- `signet agent import <passport.json>` — adds the agent to the local
  roster with the imported public key. Identity files are written to
  the agent's subdirectory. The import does NOT include the private
  key — the imported agent operates in read-verify mode on the
  destination machine until re-keyed.
- Passport import verifies the self-signature before accepting. A
  tampered passport is rejected.

### E. Auth Middleware Integration

The daemon's auth module (`platform/daemon/src/auth/`) gains an
optional signature-based auth path:

- API requests can include `X-Signet-Agent-Id` and
  `X-Signet-Signature` headers.
- The middleware verifies the signature against the agent's stored
  public key before processing the request.
- This is opt-in (disabled by default). Token-based auth remains the
  primary path. Signature auth is for high-trust automation scenarios
  where agents call the daemon API directly.

---

## Non-Goals

- PKI infrastructure or certificate authority hierarchy.
- Blockchain-based identity or decentralized identifiers (DIDs).
- Encrypting memories at rest (separate concern; this spec is about
  signing and verification, not encryption).
- Key rotation automation (v1 is manual; `signet agent rekey <name>`
  generates new pair and re-signs recent artifacts).
- Federated identity protocols (OAuth, SAML, OpenID Connect).

---

## Integration Contracts

- **Multi-Agent**: key generation hooks into agent creation flow.
  `agents` table stores public keys. Agent roster in `agent.yaml`
  gains optional `fingerprint` field for display. `agent_id` scoping
  (invariant 1) applies to all key operations.
- **Predictive Scorer**: signature verification status (signed vs
  unsigned memories) becomes a scorer signal. Signed memories from
  verified agents may rank higher in cross-agent `shared` retrieval.
- **Entity Taxonomy**: no new entity types. Agent entities
  (`entity_type = 'person'` for agents) gain a `verified` attribute
  when a keypair exists (invariant 3 respected).
- **Constraints**: signing does not affect constraint surfacing.
  Constraints surface regardless of signature status (invariant 5).

---

## Rollout Phases

### Phase 1: Key Generation + Memory Signing

Generate keypairs on agent creation. Sign new memories. Add
`signature` column to `memories`. `signet verify` CLI command for
local verification. No API auth changes.

### Phase 2: Passport Export/Import

Agent passport generation and import workflow. Session summary
signing. Skill output signing. `GET /api/verify` endpoint.

### Phase 3: Auth Middleware + Cross-Machine Trust

Optional signature-based API auth. Dashboard shows agent verification
status and fingerprints. Cross-machine passport trust chain
(multiple imported agents with verified signatures).

---

## Validation and Tests

- Key generation test: create an agent, verify keypair files exist
  at expected paths with correct permissions (0600 for private key).
- Signing test: write a memory via the pipeline, verify the
  `signature` column is populated and verifies against the agent's
  public key.
- Tamper detection test: modify a signed memory's content, verify
  signature validation fails.
- Passport test: export agent passport, import on a fresh workspace,
  verify identity files are written and public key matches.
- Passport tamper test: modify a field in the passport JSON, verify
  import rejects it with a clear error.
- Auth middleware test: send an API request with valid signature
  header, verify acceptance. Send with invalid signature, verify 401.

---

## Success Metrics

- Every memory written by a keyed agent has a valid detached
  signature verifiable without the private key.
- Agent passport export/import round-trips without identity loss
  (all markdown files, public key, and fingerprint preserved).
- Signature verification adds <5ms to memory write latency.

---

## Open Decisions

1. **Key storage format** — raw Ed25519 bytes vs PEM-encoded vs
   SSH-compatible format? SSH-compatible (`ssh-ed25519 AAAA...`)
   would allow reusing existing SSH tooling for key management.
2. **Re-keying strategy** — when an agent is re-keyed, should old
   signatures be re-signed with the new key, or should a key
   lineage chain preserve the old public key for historical
   verification?
3. **Multi-machine key sync** — should private keys sync via git
   (encrypted) or remain machine-local with passport-based public
   key distribution only?
