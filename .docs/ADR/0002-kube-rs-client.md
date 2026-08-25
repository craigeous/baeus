# ADR 0002: Kubernetes Client Library — kube-rs

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-002-kube-rs-client.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus needs a Rust-native Kubernetes client supporting watches/informers, CRUD operations, exec, port-forward, log streaming, CRD discovery, and multi-cluster connections with various auth methods.

## Decision

Use **kube-rs 3.x** (CNCF Sandbox project) with **kube-runtime** for informers/reflectors and **k8s-openapi** for built-in type definitions.

## Rationale

- Production-ready CNCF Sandbox project with active development
- Full watch/informer support via `kube_runtime` — reflectors maintain state with auto-recovery
- Dynamic resource support via `DynamicObject` and `ApiResource` for CRD discovery
- Authentication: kubeconfig parsing, OIDC, cloud provider plugins (EKS/GKE/AKS)
- Exec and port-forward via streaming WebSocket protocol
- Log streaming as `AsyncBufRead`
- Multi-cluster via separate `Client` instances
- RBAC checking via `SelfSubjectAccessReview` from k8s-openapi
- Async-native with Tokio

## Alternatives Considered

| Option | Why Rejected |
|--------|-------------|
| Custom HTTP client | Reimplements what kube-rs already provides. No informer/watch abstractions. |
| Shelling out to kubectl | Slow, hard to parse output, no streaming primitives. |

## Consequences

- Requires Tokio as async runtime (kube-rs dependency)
- No client-side field validation (delegated to API server)
- Must bridge kube-rs async operations to GPUI's UI thread
