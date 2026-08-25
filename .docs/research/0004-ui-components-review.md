# Research: UI components & views review

**Status**: Research Review
**Date**: 2026-08-25
**Subsystem**: crates/baeus-ui components + views

## Summary

The components and views layer is functionally complete and well-structured for its current scale, with thorough test coverage (61 integration files, 3,500+ tests), no panic-inducing `unwrap`/`expect` chains in hot paths, and consistent use of a per-component color-struct pattern that keeps render methods clean. The main weaknesses are performance (log viewer renders all visible lines eagerly with no virtual scroll), structural duplication across detail views and layout algorithms, an EKS wizard state-machine whose back/forward transition tables must be kept in sync manually, and a `debug_assert_eq!` guard on column/cell count that silently drops the assertion in release builds. Accessibility coverage is shallow throughout.

## Strengths

- **No panic-inducing extraction**: `json_extract.rs` uses `unwrap_or`/`unwrap_or_default` throughout; no bare `unwrap()` or `expect()` in extraction paths.
- **Bounded log buffer**: `LogBuffer::max_lines` is enforced at push time (`logs.rs:97-98`); caller sets 10,000 lines at `app_shell.rs:11119`.
- **Credential zeroization**: `EksWizardState::drop` zeroizes `secret_access_key`, `session_token`, `sso_client_secret`, `sso_access_token` (`eks_wizard.rs:188-200`).
- **Clean module split**: large components split across `_actions`, `_render`, `_state` files; `impl AppShell` in auxiliary modules keeps `app_shell.rs` from being a monolith.
- **Section collapse infra**: collapsible section pattern in `pod_detail_render.rs:174-268` is generic and reused by both pod and node detail views.
- **ANSI stripping**: `strip_ansi_escapes` with a `LazyLock<Regex>` prevents terminal injection via K8s log output (`log_viewer.rs:20-31`).

## Findings

### 1. Log viewer renders all lines eagerly — no virtual scroll
**Severity**: High

`render_log_body` (`log_viewer.rs:918-958`) calls `self.state.visible_lines()` which returns up to `max_lines` (10,000) `LogLine` references, then immediately materializes a GPUI child element for every line in a `for` loop (`log_viewer.rs:952-953`). GPUI's `uniform_list` is used elsewhere in the navigator (`sidebar.rs`) for virtualized rendering, but is absent here. At 10,000 lines and active streaming, every incoming line triggers a full re-render that builds 10,000 element subtrees. This is likely to cause frame-rate degradation during busy log streams.

### 2. Column/cell count mismatch silently passes in release builds
**Severity**: High

`json_to_table_row` (`json_extract.rs:187-193`) uses `debug_assert_eq!` to verify that the cell vec returned by each extractor matches the column count from `columns_for_kind`. In a release build this assertion is compiled out. A mismatch (e.g., adding a column definition without updating the extractor) renders an empty or truncated row with no error, not a panic. The guard covers 34 resource kinds; every future kind addition needs both files updated.

### 3. Duplicate layout algorithms in resource_map.rs
**Severity**: Medium

`resource_map.rs` contains two nearly identical Sugiyama implementations: `compute_layout` (top-down, `lines 60-131`) and `compute_layout_lr` (left-right, `lines 138-232`). The docstring at line 135 says "Same algorithm… but with swapped axes." The ~130-line bodies are structurally identical except for the final x/y assignment (lines differ only in which axis gets `layer_idx * spacing` vs `order_idx * spacing`). Topology views use `compute_layout_lr` (`topology_render.rs:10-13`); namespace map uses `compute_layout` (`namespace_map.rs:1`). The duplication means any bug fix or improvement must be applied twice.

### 4. EKS wizard: manual forward/back transition tables must stay in sync
**Severity**: Medium

The wizard's back-navigation (`eks_wizard_actions.rs:31-46`) and forward-navigation (`eks_wizard_actions.rs:72-130`) are separate match expressions encoding the same step graph. Adding a step (e.g., MFA prompt) requires updating `EksWizardStep`, `can_advance`, the forward match, and the back match independently. The `Discovering` step is handled as `return` (line 45) rather than a typed guard, which means it silently no-ops if navigation is invoked from the wrong context. There is no type-state pattern or graph structure enforcing valid transitions; invalid paths (e.g., navigating back from `Discovering` when discovery is in-flight) rely on runtime guards.

### 5. Per-component color structs — 13 separate definitions across the codebase
**Severity**: Medium

Every component defines its own private color struct extracted from `Theme`: `LogViewerColors`, `PanelColors`, `DialogColors`, `EditorColors`, `NotificationColors`, `PortForwardColors`, `MapColors`, `HeaderColors`, `BodyColors`, `NamespaceMapColors`, `ChartColors`, `LoadingColors`, `DetailColors`. Each extracts the same 5-8 fields from the same `Theme` type in slightly different ways. A theme change or new color field requires updating each struct. There is no shared `ThemeColors` extraction utility or trait.

### 6. Detail view render helpers are not fully shared
**Severity**: Medium

`pod_detail_render.rs` defines `render_kv_badges` (line 716) and `render_pod_section` (line 174). `node_detail_render.rs` defines a separate `render_kv_table` (line 132) with a different name and layout. Both files implement collapsible section wrappers that call `render_pod_section` — meaning node detail rendering is implemented as `impl AppShell` calling `render_pod_section`, a function conceptually named after "pod". `resource_detail.rs` implements its own `render_conditions` (line 764) that duplicates the conditions table from `pod_detail_render.rs`. There is no shared `render_conditions_table` utility that all three detail views call.

### 7. Log level detection uses substring contains — imprecise
**Severity**: Low

`level_color_for_line` (`log_viewer.rs:476-487`) lowercases the full line content and calls `.contains("error")`, `.contains("warn")`, `.contains("info")`. This produces false positives: "error_count=0", "warn_level", "informational" all color-shift lines that are not errors or warnings. Structured log formats (JSON, logfmt) would be better parsed at the key-value boundary.

### 8. search_bar.rs is a standalone component not reused by log viewer
**Severity**: Low

`search_bar.rs` implements `SearchBarState` with fuzzy match results and a `SearchMatch` struct for global resource search. The log viewer (`log_viewer.rs:406-446`) implements its own inline search using `gpui_component::input::Input` directly. These are intentionally different (resource search vs log search), but the log viewer's per-component search input `ensure_search_input` duplicates the input entity lifecycle pattern (lazy create + subscription). No shared "search input + clear + count" widget exists.

### 9. No keyboard navigation in log viewer search, no tab order in EKS wizard
**Severity**: Low

Log viewer search prev/next buttons (`log_viewer.rs:1046-1069`) are click-only; there are no `on_key_down` bindings for Enter/Shift+Enter to cycle matches. EKS wizard input fields (`eks_wizard_render.rs`) use GPUI `InputState` entities but there is no explicit Tab order between fields, relying entirely on mouse interaction.

### 10. topology_render.rs is 2,247 lines mixing state, actions, and render
**Severity**: Low

`topology_render.rs` mixes `TopologyState` definition (lines 21-43), `kind_color` helper, node/edge canvas rendering, hover interaction, zoom/pan state, and GPUI `Render` impl in a single file. The navigation, interaction (click on node, hover), and layout computation are interleaved. Compared to the EKS wizard's clean three-file split (`_wizard.rs`, `_wizard_actions.rs`, `_wizard_render.rs`), topology has no analogous decomposition despite being nearly twice the size of the wizard render file.

## Candidate opportunities

- **Virtualize the log body**: Replace the eager `for (i, line) in visible.iter().enumerate()` loop in `render_log_body` with `uniform_list` (already used in navigator); this is the highest-impact change for runtime performance.
- **Promote `debug_assert` to checked assertion or a startup test**: Either use a release-time check (`assert_eq!` with a feature flag, or a cargo test that exercises every kind) to catch column/cell count mismatches before they reach users.
- **Factor out layout axis as a parameter**: Replace `compute_layout` and `compute_layout_lr` with a single `compute_layout(rels, axis: LayoutAxis)` to eliminate the 130-line duplication.
- **Type-state wizard transitions**: Encode the wizard step graph as a Rust enum with transitions enforced at type level, or at minimum extract the back/forward maps into a single `fn next_step(step, auth_method) -> Option<Step>` / `fn prev_step(step, auth_method) -> Option<Step>` pair to avoid the dual-match synchronization burden.
- **Shared `ThemeColors` extraction**: A shared `struct ThemeColors` constructed from `&Theme` once per render, passed by reference, would remove the 13 separate color struct definitions and ensure consistency.
- **Unify conditions/kv render helpers**: Extract `render_conditions_table`, `render_kv_table`, and `render_kv_badges` into a shared module (e.g., `components/detail_helpers.rs`) instead of having pod, node, and resource_detail each define their own.
- **Structured log parsing for level detection**: Check for JSON keys (`"level"`, `"severity"`) or logfmt tokens before falling back to substring search in `level_color_for_line`.
- **Keyboard bindings for log search**: Add Enter/Shift+Enter on the search input entity to invoke `next_search_match`/`prev_search_match`.
- **Split topology_render.rs**: Apply the wizard's three-file decomposition pattern (state, actions, render) to `topology_render.rs`.

## Citations

- `crates/baeus-ui/src/components/log_viewer.rs` — lines 20-31 (ANSI strip), 186-210 (LogViewerState::new), 918-958 (render_log_body, eager loop), 1229-1287 (LogViewerColors struct), 476-487 (level_color_for_line)
- `crates/baeus-ui/src/components/json_extract.rs` — lines 140-219 (json_to_table_row with debug_assert), 187-193 (debug_assert_eq!)
- `crates/baeus-ui/src/components/eks_wizard.rs` — lines 14-39 (EksWizardStep), 163-186 (can_advance), 188-200 (Drop/zeroize)
- `crates/baeus-ui/src/components/eks_wizard_actions.rs` — lines 22-50 (go_back match), 53-130 (advance match)
- `crates/baeus-ui/src/components/resource_map.rs` — lines 54-131 (compute_layout), 133-232 (compute_layout_lr, duplicate)
- `crates/baeus-ui/src/components/topology_render.rs` — lines 1-60 (TopologyState, kind_color, file scope)
- `crates/baeus-ui/src/components/pod_detail_render.rs` — lines 174-268 (render_pod_section), 716 (render_kv_badges)
- `crates/baeus-ui/src/components/node_detail_render.rs` — lines 1-5 (uses render_pod_section), 132 (render_kv_table)
- `crates/baeus-ui/src/views/resource_detail.rs` — lines 764 (render_conditions), 847-860 (DetailColors struct)
- `crates/baeus-core/src/logs.rs` — lines 82-98 (LogBuffer max_lines eviction)
- `crates/baeus-ui/src/layout/app_shell.rs` — line 11119 (LogViewerState::new(10_000))
- `crates/baeus-ui/src/components/search_bar.rs` — lines 1-50 (SearchBarState, SearchMatch)
- `crates/baeus-ui/tests/` — 61 integration test files
