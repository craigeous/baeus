# Research: UI layout & app shell review

**Status**: Research Review
**Date**: 2026-08-25
**Subsystem**: crates/baeus-ui layout + crate root

## Summary

`app_shell.rs` has grown to 11,692 lines — nearly 3x the figure cited in project memory — and acts as a god-object absorbing render logic, async coordination, terminal/PTY management, cluster appearance persistence, AWS SSO orchestration, keyboard routing, drag state, and tab management simultaneously. The resource table body is rendered as a plain div child-loop (capped at 200 rows) with no `uniform_list` virtualization despite the navigator sidebar demonstrating the correct pattern. Several synchronous file-system writes occur on the GPUI main thread from event callbacks. Duplicated type definitions and dispatch logic have accumulated that the `pod_detail_render.rs` extraction pattern has not yet reached.

## Strengths

- Navigator sidebar correctly uses `uniform_list` with per-cluster scroll handles, demonstrating the virtualization pattern works (app_shell.rs:5029-5049).
- Async cluster connection and resource-watch tasks correctly use `cx.spawn` + `tokio_handle.spawn` to keep work off the main thread (app_shell.rs:1623, 2431).
- `sanitize_error_message` guards credential leakage in error banners (app_shell.rs:419-433).
- `LazyLock<Regex>` ensures the three redaction patterns are compiled once, not per call (app_shell.rs:424-428).
- Pod detail extraction was successfully extracted to `pod_detail_render.rs`, proving the split-module pattern scales.

## Findings

### 1. God-object: app_shell.rs has outgrown a single file
**Severity**: high

`app_shell.rs` is 11,692 lines (verified: `wc -l`). The `AppShell` struct carries 60+ fields covering unrelated concerns: drag resize coordinates for dock/column/sidebar/topology (`is_dragging_dock`, `column_drag_index`, `is_dragging_cluster_topo_resize`, etc. — app_shell.rs:511-611), YAML editor state (`yaml_editors`, `yaml_editor_focus_handles` — app_shell.rs:557-559), AWS SSO state (`eks_wizard`, `eks_cluster_data`, `pending_sso_login` — app_shell.rs:617-635), and PTY process handles (`pty_processes`, `pty_output_buffers` — app_shell.rs:508-510). There are 8 separate `impl AppShell` blocks (lines 638, 967, 1536, 2997, 3140, 3350, 3694, 10451, 10633, 10859, 11045) and 100+ `render_*` methods. The CLAUDE.md warning "Avoid parallel agent edits to app_shell.rs — file modification races" is a direct symptom. All future feature work carries an overhead proportional to navigating ~12k lines.

### 2. Resource table body has no virtualization
**Severity**: high

`render_resource_table_body_filtered` (app_shell.rs:7992-8042) renders each filtered row as a `.child()` call in a for loop over `rows.iter().take(200)`. This produces up to 200 GPUI element nodes per frame for every render pass of the active resource list. The navigator sidebar uses `uniform_list` (app_shell.rs:5029) so only visible items are rendered. A namespace with 200+ pods, deployments, or events will re-render all 200 row subtrees on every `cx.notify()` (e.g., every informer update, every mouse-move during drag). The hard cap of 200 (app_shell.rs:8021) is a workaround for this cost rather than a fix.

### 3. KeyboardNavigationState allocated on every keydown
**Severity**: medium

`handle_keyboard_shortcut` (app_shell.rs:3352) calls `KeyboardNavigationState::new()` at line 3385 on every `KeyDownEvent`. `::new()` calls `KeybindingConfig::default_bindings()` which allocates a `Vec<KeyBindingEntry>` of 20 entries, each with two heap-allocated `String` fields. The result is immediately discarded after one lookup. `AppShell` already holds state for focus tracking; the keybinding config should be stored once (or be a `const` / `static`).

### 4. Duplicate focus-mode dispatch between AppShellState and AppShell
**Severity**: medium

`AppShellState::handle_key_action` (app_shell.rs:3256-3296) and `AppShell::handle_keyboard_shortcut` (app_shell.rs:3390-3465) implement the same `ToggleCommandPalette` and `FocusSearch` toggle logic in parallel. `AppShellState` (app_shell.rs:3251) holds only a single `focus_mode: FocusMode` field while `AppShell` also carries `pub focus_mode: FocusMode` at line 451. The detached struct is used only in tests; the real keyboard path in `AppShell` does not delegate to it. The two implementations can diverge silently.

### 5. Synchronous file I/O on the GPUI main thread
**Severity**: medium

Three call sites write or read files synchronously on the main thread:
- `persist_cluster_appearances` (app_shell.rs:4533): `std::fs::write` called from click handlers (lines 4763, 4809).
- `load_cluster_appearances` (app_shell.rs:4547): `std::fs::read_to_string` called from `AppShell::new` at startup.
- `generate_eks_kubeconfig_file_with_role` (app_shell.rs:11484): `std::fs::write` called synchronously before the async connect span.
Additionally, `std::process::Command::new("open")` / `explorer.exe` / `xdg-open` are spawned synchronously from a click handler (app_shell.rs:4846-4863). While `spawn()` is non-blocking for the child process, the fork itself happens on the main thread. GPUI owns the main thread; blocking it stalls rendering.

### 6. JSON extraction called on every render frame for detail views
**Severity**: medium

`render_resource_detail_content` (app_shell.rs:8974) calls `json_extract::extract_detail_properties`, `extract_labels`, `extract_annotations`, and `extract_conditions` (lines 9080-9114) unconditionally on every render pass when the Overview tab is active. Each call traverses and allocates from a `serde_json::Value`. The raw JSON is also serialised to YAML via `serde_yaml_ng::to_string` inside the render body at line 9015 (guarded by a lazy-init check, but the guard is also evaluated per frame). The pattern pod_detail already uses — extracting typed `PodDetailData` once on data arrival — is not yet applied to generic resource details.

### 7. Duplicate color palette constant
**Severity**: low

`AppShell::CLUSTER_COLOR_PALETTE` (app_shell.rs:4562-4575) and `generate_cluster_color::PALETTE` (sidebar.rs:116-129) are identical 12-element `[u32; 12]` arrays. A change to one must be manually mirrored to the other.

### 8. AppShellState is a detached shadow of AppShell
**Severity**: low

`AppShellState` (app_shell.rs:3251-3253) holds only `focus_mode: FocusMode`, yet `AppShell` also has `pub focus_mode: FocusMode` at line 451 — the same field. `AppShellState` exists solely to enable unit tests for `handle_key_action`; the real action dispatch in `AppShell::handle_keyboard_shortcut` never calls through it. The struct creates the illusion of a separable state object without the substance.

### 9. Render method mutability inconsistency
**Severity**: low

Many `render_*` methods take `&mut self` (e.g., `render_cluster_settings` app_shell.rs:4580, `render_resource_detail_content` app_shell.rs:8975, `render_prefs_kubernetes` app_shell.rs:6297) while others take `&self`. Mutable borrows in the render path prevent composing multiple render helpers in the same expression without intermediate `let` bindings, and signal that side effects (lazy init of `Input` entities, YAML editor creation) are mixed into rendering rather than separated into a prepare phase.

### 10. Render helper return type inconsistency
**Severity**: low

`render_prefs_kubernetes` (app_shell.rs:6296) returns `Vec<Div>` while every other `render_*` helper returns a single `Div` or `Stateful<Div>`. The caller must iterate the vec to attach children (app_shell.rs:6080+), breaking the uniform call pattern and making the function harder to inline or replace.

## Candidate opportunities

- Extract `AppShell` into focused sub-structs: `DragState`, `DockManager`, `EksConnectionState`, `ClusterAppearanceStore` — each owned by `AppShell` but defined and tested separately. Mirrors the existing `SidebarState`, `DockState`, `WorkspaceState` pattern.
- Replace the resource table body for-loop with `uniform_list` using a cached `Vec<TableRow>` as the item source, following the navigator pattern at app_shell.rs:5029.
- Store `KeybindingConfig` as a field on `AppShell` (or a `static`) rather than allocating it per keydown event.
- Merge `AppShellState` into `AppShell`, delegate `handle_keyboard_shortcut` through a single focus-mode handler, and keep unit-test coverage by constructing `AppShell` in a headless context.
- Move `persist_cluster_appearances` and kubeconfig writes to `cx.spawn` async tasks so file I/O never touches the main thread.
- Cache `extract_detail_properties` / `extract_labels` / `extract_conditions` results alongside `resource_detail_data`, invalidating them only when the JSON value changes — following the `PodDetailData` extraction pattern.
- Deduplicate `CLUSTER_COLOR_PALETTE` / `PALETTE` into a single `const` in `sidebar.rs` (or a shared constants module) referenced by both callers.
- Standardise render helper signatures: `&self` for pure view helpers, `&mut self` only when lazy init is genuinely needed; move lazy-init logic out of render and into a `prepare_view` step called from `cx.notify` handlers.
- Normalise `render_prefs_kubernetes` to return `Div` (wrapping its children) so all render helpers share the same return type.

## Citations

- `crates/baeus-ui/src/layout/app_shell.rs` — primary subject; all line references above are from this file
- `crates/baeus-ui/src/layout/sidebar.rs` — `PALETTE` constant (line 116), `uniform_list` pattern reference
- `crates/baeus-ui/src/layout/mod.rs` — `NavigationTarget` enum, `AppLayout`
- `crates/baeus-ui/src/layout/dock.rs` — `DockState`, `DockTab`, constants
- `crates/baeus-ui/src/layout/workspace.rs` — `WorkspaceState`, `Tab`
- `crates/baeus-ui/src/layout/header.rs` — `ClusterSelector`, `NamespaceSelector`, `EnhancedNamespaceSelector`
- `crates/baeus-ui/src/layout/command_palette.rs` — `CommandPaletteState`, `CommandEntry`
- `crates/baeus-ui/src/layout/indent_guides.rs` — `NavigatorIndentGuideDecoration`
- `crates/baeus-ui/src/lib.rs` — crate root
