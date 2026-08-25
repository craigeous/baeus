# ADR 0004: Helm Integration — Hybrid CLI + kube-rs Approach

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-004-helm-cli-hybrid.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus needs to list, inspect, install, upgrade, and rollback Helm releases. Native Rust Helm libraries are immature.

## Decision

Use a **hybrid approach**: read releases via kube-rs (K8s Secrets), execute operations via `helm` CLI subprocess.

## Rationale

- Helm stores releases as K8s Secrets (`sh.helm.release.v1.<name>.<version>`) — readable directly via kube-rs without Helm dependency
- For install/upgrade/rollback: shell out to `helm` CLI because:
  - Native Rust Helm libraries are immature (helm-api 0.1.0, helm-wrapper-rs 0.1.0)
  - Helm's Go implementation is the source of truth for chart rendering and dependency resolution
  - The helm CLI is already present on most DevOps workstations
- Chart repository browsing uses Helm's HTTP index.yaml format directly

## Alternatives Considered

| Option | Why Rejected |
|--------|-------------|
| helm-api crate | Version 0.1.0, early/experimental, not production-ready. |
| helm-wrapper-rs | Version 0.1.0, basic wrapper with limited command support. |
| Pure kube-rs | Can read releases but cannot install/upgrade/rollback. |

## Consequences

- External `helm` CLI must be installed for mutating operations
- Read-only operations (list, inspect) work without Helm CLI
- Must handle CLI absence gracefully with user guidance
