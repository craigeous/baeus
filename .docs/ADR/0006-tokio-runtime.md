# ADR 0006: Async Runtime — Tokio

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-006-tokio-runtime.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus requires an async runtime for all Kubernetes API calls, log streaming, exec sessions, Helm CLI invocations, and file I/O. The UI thread must never be blocked.

## Decision

Use **Tokio** as the async runtime for all background I/O operations.

## Rationale

- kube-rs requires Tokio as its async runtime — no alternative is viable
- GPUI uses its own async executor for UI tasks but interoperates with Tokio for background I/O
- All K8s API calls, log streaming, exec sessions, and Helm CLI invocations run on Tokio tasks
- The UI thread is never blocked — all async results are dispatched back to GPUI's entity/model update cycle
- Tokio's multi-threaded runtime enables concurrent cluster connections and watch streams

## Alternatives Considered

None viable — Tokio is required by kube-rs.

## Consequences

- Two async runtimes coexist (GPUI executor + Tokio) — requires careful bridging
- All I/O code must use Tokio-compatible async primitives
- Background task cancellation must be handled explicitly (e.g., when switching clusters)
