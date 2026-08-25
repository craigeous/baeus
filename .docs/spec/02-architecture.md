# 02 — Architecture

**Status**: Draft (descriptive back-fill, 2026-08-25 — pending Plan Review)

How the codebase is actually organized today. Decision rationale lives in
[`.docs/ADR/`](../ADR/README.md); this file maps the current reality.

## Workspace

Cargo workspace (resolver 2, edition 2024, MSRV 1.85) with 8 crates:

| Crate | Responsibility | Notable modules |
|-------|----------------|-----------------|
| `baeus-app` | Entry point, GPUI app init, macOS bundling, settings | `main.rs`, `app.rs`, `settings.rs` (UserPreferences), `assets.rs` |
| `baeus-core` | Kubernetes client & cluster management | `client.rs`, `cluster.rs`, `kubeconfig.rs` (discovery/scan), `informer.rs`, `watch.rs`, `resource.rs`, `logs.rs`, `exec.rs`, `metrics.rs`, `rbac.rs`, `crd.rs`, `auth.rs`, `runtime.rs` (TokioHandle), `aws_eks.rs`, `aws_sso.rs` |
| `baeus-ui` | GPUI UI: shell, navigator, tables, detail views | `layout/` (app_shell, sidebar, workspace, dock, header, command_palette, indent_guides), `components/` (resource_table, json_extract, pod_detail(+_render), node/event detail renders, log_viewer, terminal_view(+_component), eks_wizard(+_actions/_render), aws_sso_banner, editor_view, yaml_editor_render, metrics_chart, donut_chart, resource_map, topology_render, search_bar, details_panel, confirm_dialog, notification, loading, port_forward, status_badge), `views/`, `theme.rs`, `icons.rs`, `models.rs` |
| `baeus-helm` | Helm release decoding & operations | `releases.rs` (decode from K8s Secrets), `charts.rs`, `operations.rs` (CLI hybrid) |
| `baeus-terminal` | Terminal emulation | `emulator.rs` (alacritty_terminal), `pty.rs`, `pty_process.rs` (portable-pty) |
| `baeus-editor` | YAML editor | `buffer.rs` (ropey), `highlight.rs` (tree-sitter), `yaml.rs`, `diff.rs` |
| `baeus-plugins` | Plugin loading | `api.rs` (BaeusPlugin trait), `loader.rs` (libloading), `registry.rs`, `sandbox.rs` |
| `baeus-test-utils` | Shared test helpers | `mock_cluster.rs`, `fixtures.rs`, `ui_harness.rs` |

Top-level `tests/ui/` holds workspace-level UI tests; the bulk of view tests
live as integration files under `crates/baeus-ui/tests/` (61 files) — large
test modules were extracted there to keep inline test modules small enough to
avoid GPUI proc-macro stack overflows.

## Threading model

- **GPUI owns the main thread** — all rendering and entity updates.
- **Tokio runs on a background thread** via the `TokioHandle` global
  (`baeus-core::runtime`); every K8s API call, watch, log stream, exec
  session, and helm CLI invocation is a Tokio task; results dispatch back
  into GPUI's entity update cycle (ADR 0006).

## UI patterns in force

- `impl Render for X` returns `impl IntoElement`; deeply chained builders are
  broken into small helper methods returning `Div` to avoid proc-macro stack
  overflow (`RUST_MIN_STACK=268435456` and `#![recursion_limit = "4096"]` in
  `baeus-ui/src/lib.rs` are load-bearing).
- `impl AppShell` rendering methods live in separate component modules
  (e.g. `pod_detail_render.rs`, `node_detail_render.rs`) to keep
  `app_shell.rs` small; accessed fields/methods are `pub(crate)`.
- Navigator: per-cluster `uniform_list` with `NavigatorIndentGuideDecoration`
  painting 1px guide lines; `NavigatorFlatEntry` + `flatten_navigator_tree()`
  in `sidebar.rs`.
- Theme `to_gpui()` returns `Rgba` (not `Hsla`); semi-transparency via
  explicit `Rgba { r, g, b, a }` construction.
- Interactive elements need `.id()` (→ `Stateful<Div>`) for `on_click`,
  `overflow_y_scroll`, keyboard handlers; `gpui::prelude::FluentBuilder`
  provides `.when()`.
- `BaeusAssets` combines `gpui-component-assets` with rust-embed custom
  Lucide SVGs (`SectionIcon` enum, ~20 variants).

## External surface

- kube-rs 0.98 (+ kube-runtime, k8s-openapi 0.24) for all cluster I/O.
- AWS SDK (config, eks, sso, ssooidc, sts, smithy runtime) for the EKS
  connection wizard and AWS SSO re-authentication.
- helm CLI subprocess for install/upgrade/rollback (ADR 0004).
- `notify` for kubeconfig directory watching; `zeroize`/`secrecy`/`ring`/
  `rustls` for credential handling; `petgraph` for the resource map model.
