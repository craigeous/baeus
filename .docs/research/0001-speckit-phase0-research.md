# Research: Kubernetes Cluster Management UI (speckit Phase 0)

**Status**: Approved (imported)
**Date**: 2026-02-24
**Provenance**: Imported verbatim from `baeus-spec:specs/001-k8s-cluster-ui/research.md` during loom alignment. Findings R-001..R-008 are recorded as ADRs 0001-0006 in `.docs/ADR/` (R-005 YAML editor stack and R-007 resource-map rendering have no ADR; they are captured here).

---

## R-001: GPU-Rendered UI Framework Selection

**Decision**: GPUI (Zed's GPU-accelerated UI framework)

**Rationale**:
- Direct lineage from Warp's approach — Nathan Sobo built Warp's custom UI framework, which evolved into GPUI for Zed
- Production-proven: Zed editor ships with GPUI rendering complex UIs at 120+ FPS on Metal
- Native Metal support on macOS with flexbox layout (Taffy engine) and grid support
- GPUI Component library (by Longbridge) provides 40+ production-ready components including data tables with virtual scrolling, charts (line, bar, area, pie), forms, dialogs, and more
- Excellent text rendering quality — Zed is a code editor built on GPUI
- Built-in accessibility and keyboard navigation support
- Hybrid immediate/retained mode with reactive state management

**Alternatives considered**:

| Framework | Why Rejected |
|-----------|-------------|
| Makepad | Font rendering and internationalization acknowledged as lacking. No data table or chart components. Accessibility unclear. Better suited for creative/artistic apps than data-heavy enterprise tools. |
| Xilem/Masonry/Vello | Alpha state, explicitly "not recommended for production." Text input described as "janky." Limited widget set. Would need 1-2 years to mature. |
| Custom framework (Warp-style) | Massive engineering effort (person-years). GPUI provides equivalent capability for free. Better to build the app than rebuild the framework. |

## R-002: Kubernetes Client Library

**Decision**: kube-rs 3.x (CNCF Sandbox project)

**Rationale**:
- Production-ready, CNCF Sandbox project with active development
- Full watch/informer support via `kube_runtime` — reflectors maintain accurate state with auto-recovery
- Dynamic resource support via `DynamicObject` and `ApiResource` for CRD discovery without compile-time types
- Authentication: kubeconfig parsing, OIDC, cloud provider plugins (EKS/GKE/AKS)
- Exec and port-forward via streaming WebSocket protocol
- Log streaming as `AsyncBufRead`
- Multi-cluster via separate `Client` instances
- RBAC checking via standard SelfSubjectAccessReview API types from k8s-openapi
- Async-native with Tokio

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| Custom HTTP client | Reimplements what kube-rs already provides. No informer/watch abstractions. |
| Shelling out to kubectl | Not suitable for a desktop app — slow, hard to parse output, no streaming primitives. |

**Known limitations vs Go client-go**:
- No client-side field validation (delegated to API server — acceptable)
- Less comprehensive documentation/examples — mitigated by kube.rs docs and community
- Uses k8s-openapi for builtin types rather than maintaining its own

## R-003: Helm Integration Strategy

**Decision**: Hybrid approach — read releases via kube-rs (Secrets), execute operations via `helm` CLI subprocess

**Rationale**:
- Helm stores releases as Kubernetes Secrets (`sh.helm.release.v1.<name>.<version>`) — these can be read directly via kube-rs for listing and inspecting releases without any Helm dependency
- For install, upgrade, and rollback: shell out to the `helm` CLI. This is the most reliable approach because:
  - Native Rust Helm libraries are immature (helm-api 0.1.0, helm-wrapper-rs 0.1.0)
  - Helm's Go implementation is the source of truth for chart rendering and dependency resolution
  - The helm CLI is already present on most DevOps workstations
- Chart repository browsing can use Helm's HTTP index.yaml format directly

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| helm-api crate | Version 0.1.0, early/experimental, generated from proto files, not production-ready. |
| helm-wrapper-rs | Version 0.1.0, basic wrapper with limited command support. |
| Pure kube-rs (no Helm at all) | Can read releases but cannot install/upgrade/rollback charts. |

## R-004: Terminal Emulation

**Decision**: alacritty_terminal + portable-pty (same stack as Zed editor)

**Rationale**:
- `alacritty_terminal` (v0.25.1) — battle-tested terminal emulation library used by Alacritty and Zed
- `portable-pty` (v0.9.0) — cross-platform PTY handling with native macOS support, 900k+ downloads/month
- Zed editor provides a reference implementation of embedding alacritty_terminal within GPUI — we can follow the same architecture:
  1. Terminal core wraps `alacritty_terminal::Term` with a custom event listener
  2. PTY subprocess managed via portable-pty
  3. Terminal grid rendered to GPUI view with GPU-accelerated text rendering
  4. Keyboard input forwarded to PTY, output processed through term parser
- For pod exec specifically: kube-rs provides WebSocket-based exec — the PTY output is piped through the K8s exec stream rather than a local process

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| Custom VTE implementation | Reimplements complex terminal escape sequence handling. Not worth the effort. |
| xterm.js via WebView | Introduces a web runtime dependency, breaks the pure-Rust GPU rendering approach. |

## R-005: YAML Editor Stack

**Decision**: ropey + tree-sitter + tree-sitter-yaml + serde-yaml-ng

**Rationale**:
- `ropey` (v1.x, stable) — production-ready rope data structure with single-digit microsecond edits even on large files, SIMD acceleration, UTF-8 aware. Used by Helix editor.
- `tree-sitter` + `tree-sitter-yaml` — syntax highlighting via incremental parsing. Used by GitHub, Zed, Helix for precise, consistent highlighting.
- `serde-yaml-ng` — active, human-maintained fork of the deprecated serde_yaml. Chosen over `serde_yml` due to AI-generated documentation quality concerns.
- `yaml-rust2` — fully YAML 1.2 compliant parser for lower-level parsing needs.

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| serde_yaml | Deprecated and archived (March 2024). |
| serde_yml | AI-generated documentation with quality concerns (broken docs.rs, hallucinated flags). |
| xi-rope | Discontinued (xi-editor project no longer maintained). |

## R-006: Async Runtime

**Decision**: Tokio

**Rationale**:
- kube-rs requires Tokio as its async runtime
- GPUI uses its own async executor for UI tasks but interoperates with Tokio for background I/O
- All K8s API calls, log streaming, exec sessions, and Helm CLI invocations run on Tokio tasks
- The UI thread is never blocked — all async results are dispatched back to GPUI's entity/model update cycle

**Alternatives considered**: None viable — Tokio is required by kube-rs.

## R-007: Resource Relationship Visualization

**Decision**: Custom graph rendering component built on GPUI primitives

**Rationale**:
- No off-the-shelf graph visualization library exists for GPUI
- Resource relationships in Kubernetes are well-defined (owner references, label selectors, service selectors) — the graph structure is predictable and bounded per namespace
- GPUI provides low-level drawing primitives (rectangles, lines, text, bezier curves) sufficient for rendering a directed acyclic graph
- Layout algorithm: Use a layered/hierarchical layout (Sugiyama-style) since K8s resource relationships naturally form DAGs (Ingress → Service → Deployment → ReplicaSet → Pods)
- Interactive features (zoom, pan, click-to-select) map directly to GPUI's event model

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| petgraph + custom renderer | petgraph is a graph data structure library, not a visualization library. Would still need custom GPUI rendering. Will use petgraph for the data model but render ourselves. |
| Embedded WebView with D3.js | Breaks the pure-Rust GPU rendering approach. Performance and integration overhead. |

## R-008: Plugin Architecture

**Decision**: Dynamic library loading via Rust `libloading` with a defined Plugin trait API

**Rationale**:
- Plugins implemented as Rust dynamic libraries (.dylib on macOS) that implement a versioned `Plugin` trait
- Plugin API surface exposes: registering custom views, adding resource actions, providing sidebar items, hooking into resource lifecycle events
- Plugins are sandboxed — they interact only through the defined API, cannot access arbitrary application state
- Hot-reloading: plugins can be loaded/unloaded without app restart via `libloading`
- Plugin discovery: local directory scanning + optional remote catalog (HTTPS fetch of plugin index)

**Alternatives considered**:

| Option | Why Rejected |
|--------|-------------|
| WASM plugins | Higher isolation but significant performance overhead and limited access to GPUI rendering primitives. |
| Lua scripting | Limited type safety, another language for plugin authors to learn. |
| gRPC-based out-of-process plugins | Complex IPC, latency overhead, harder to integrate custom UI views. |
