# 05 — Feature Scope

**Status**: Draft (descriptive back-fill, 2026-08-25 — pending Plan Review)

Inventory of the product requirements Baeus was built to, with implementation
status at alignment time. Authoritative requirement text: the historical
speckit spec (`baeus-spec:specs/001-k8s-cluster-ui/spec.md`, FR-001..FR-076,
SC-001..SC-015). This file is a map, not a re-statement.

## Requirement areas

| Area | FRs | Status at alignment |
|------|-----|---------------------|
| Cluster connectivity (auto-detect contexts, multi-cluster, status indicators, auth methods) | FR-001..004 | Implemented (kube-rs, real API) |
| Resource management (categorized types, sortable/filterable tables, details panel, CRUD, per-kind actions, confirmations) | FR-005..010, 069, 072..074 | Implemented |
| YAML editing (highlight, validation, diff) | FR-011..013 | Implemented (`baeus-editor`) |
| Logging (stream, multi-container, previous container, search modes, download, move-up, toolbar) | FR-014..017, 064, 065 | Implemented (Dock log viewer) |
| Terminal / port-forward (exec, confirmation, port-forward manager view) | FR-018, 019, 039, 040, 068 | Implemented |
| Helm (list, install flow, upgrade, rollback) | FR-020..022 | Listing/inspect implemented; operations via helm CLI hybrid (ADR 0004) |
| Metrics & events (CPU/mem charts, event feed, graceful degradation, issues section) | FR-023..025, 066 | Implemented (metrics-server dependent) |
| Custom resources (CRD discovery, instance browsing/editing) | FR-026, 027 | Implemented (DynamicObject) |
| Visualization (namespace resource relationship map) | FR-028 | Implemented (petgraph model + GPUI rendering) |
| Extensibility (plugin architecture, management UI) | FR-029, 030 | Scaffolded (`baeus-plugins`: trait, loader, registry, sandbox) |
| RBAC awareness | FR-031, 032 | Implemented (SelfSubjectAccessReview, graceful degradation) |
| Navigation & search (global search, multi-namespace, shortcuts) | FR-033..035 | Implemented (command palette, shortcuts) |
| Tab management (closable, pinned, preview/fixed modes) | FR-041, 042, 067 | Implemented |
| Kubeconfig discovery (default path, scan dirs, heuristic, aggregation, dir watching) | FR-043..045, 046, 076 | Implemented (recursive `~/.kube/` scan, `notify` watch) |
| Navigator sidebar (tree, categories, connect action, drill-into, icons, resize, context menu, tracking) | FR-047..054, 070, 071 | Implemented |
| Application layout (title bar, tab bar, workspace, dock, status bar) | FR-055..058, 075 | Implemented |
| Operational completeness (real client, real data, view routing, distributable bundle, error handling) | FR-059..063 | Implemented (release workflow + `build-app.sh`) |
| Design & experience (sleek/minimal, light+dark themes, virtual scrolling) | FR-036..038 | Implemented |

## Post-speckit additions (not in the original FRs)

- AWS SSO login and EKS cluster connection wizard (discover EKS clusters,
  role ARN handling, in-memory credential injection for first connect,
  auto-connect of restored EKS clusters, re-authentication preferences).
- Auto-updater preferences (release channel / reauth-updater feature branch,
  merged via PR #1).
- Enhanced pod detail view (typed view-model, collapsible sections, custom
  SVG section icons) and UX overhaul (per-kind JSON extraction, navigator
  redesign, details panel wiring).

These additions are recorded here descriptively; any *further* direction is a
planning-cycle decision, not an alignment-pass one.

## Success criteria snapshot

The speckit SC-001..SC-015 targets (3s dashboard load, 5s global search,
5 000-row lists, 1s log latency, ≤3-click operations, 10+ clusters, 2s cold
start, distributable bundle, etc.) remain the accepted performance bar; no
alignment-pass re-measurement was performed.
