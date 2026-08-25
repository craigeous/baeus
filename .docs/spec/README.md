# spec/

Approved descriptions of what Baeus is and how it works. Specs are the
highest-authority design documents in the project: **`spec/` wins over
`slice-plans/`**, and approved specs are frozen — changed only through a new
planning cycle.

- **Authored by:** the planner role (via `/loom:run` or `/loom:plan`).
- **Reviewed by:** the plan evaluator (blind) at Plan Review.
- **Lifecycle:** Draft → Plan Review → **Approved** (frozen).

## Reading order

1. [01 — Overview](01-overview.md) — what Baeus is, current capability snapshot
2. [02 — Architecture](02-architecture.md) — workspace crates, threads, key patterns
3. [03 — Toolchain & Gate](03-toolchain-and-gate.md) — build, test, lint, CI, release
4. [04 — Data Model](04-data-model.md) — core entities and relationships
5. [05 — Feature Scope](05-feature-scope.md) — requirement inventory and implementation status
6. [06 — Remediation of High-Severity Findings](06-remediation-highs.md) — prescriptive fix plan for the 17 highs surfaced by research 0002–0006

Specs 01–05 are **Status: Draft** — descriptive back-fill authored during
loom alignment (2026-08-25), pending blind Plan Review. They describe the
project as it exists; they prescribe nothing. Spec 06 is **Status: Plan
Review** — a prescriptive remediation plan derived from the five Approved
research notes.

## Non-negotiable decisions

Imported from the Baeus constitution (`baeus-spec:.specify/memory/constitution.md`,
v2.0.0, ratified 2026-02-24) — pre-existing settled constraints, recorded here
during alignment (no new decisions were made by the alignment pass):

1. **Modular architecture** — independent single-responsibility crates, no
   circular dependencies, strong types at boundaries, optional capabilities
   (Helm, plugins) isolated behind feature boundaries.
2. **Kubernetes-native** — watches/informers over polling; multi-cluster and
   multi-context from day one; dynamic CRD discovery; RBAC-aware with graceful
   degradation.
3. **Test-first (NON-NEGOTIABLE)** — TDD red-green-refactor; mock K8s clients
   for unit tests; integration tests for API interactions; UI interaction
   tests for views; no merges that decrease coverage.
4. **Performance & responsiveness** — UI thread never blocks; informer-based
   caching; <2s startup to first meaningful render; memory scales with
   displayed resources; virtual scrolling for large lists.
5. **User experience** — match or exceed Lens/OpenLens UX; keyboard-driven
   navigation; real-time status indicators; <100ms context/namespace switch;
   built-in log streaming and exec; dark and light themes.

Security requirements (same source):

- Kubeconfig files are read-only; credentials never written to disk, logs, or
  telemetry.
- TLS per kubeconfig; no option to disable certificate verification.
- Exec and port-forward sessions require explicit user confirmation.
- No telemetry or network calls outside configured cluster endpoints without
  opt-in consent.
- Dependencies audited for known vulnerabilities on every CI run.

Framework-level decisions live in [`.docs/ADR/`](../ADR/README.md)
(GPUI, kube-rs, alacritty_terminal, hybrid Helm, dylib plugins, Tokio).
