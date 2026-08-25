# Research: Supporting crates review

**Status**: Research Review
**Date**: 2026-08-25
**Subsystem**: baeus-helm, baeus-terminal, baeus-editor, baeus-plugins, baeus-test-utils

## Summary

All five supporting crates compile and have substantial unit-test coverage, but
several exist primarily as scaffolding: the plugin loader is never called from
app code, the Helm CLI subprocess is not implemented, and alacritty_terminal is
not yet integrated. Two findings carry memory-safety risk — an unchecked
`Box::from_raw` in the dylib loader and an O(m×n) LCS allocator in the diff
engine. The ADRs for terminal (0003), helm (0004), and plugins (0005) each
describe behaviours that the current code does not yet deliver.

## Strengths

- `PtyProcess::Drop` kills and reaps the child process, preventing zombie
  processes on normal teardown (`pty_process.rs:111-118`).
- `normalize_path` in both `loader.rs` and `sandbox.rs` prevents `..` path
  traversal without requiring the path to exist — correct for pre-existence
  checks.
- `decode_helm_release` applies a 50 MB decompressed-size cap via `.take()`
  before reading, protecting against zip-bomb payloads (`releases.rs:10,24`).
- Error types across all crates use `thiserror` and return `Result` from all
  public APIs — no `panic` paths found in production code paths.
- `validate_shell_path` in `pty_process.rs` consults `/etc/shells` as a
  secondary allowlist, so non-standard but user-installed shells work without
  code changes.

## Findings

**1. Plugin loader not connected to app** — HIGH

`PluginLoader`, `PluginRegistry`, and `SandboxedLoader` are never called from
`crates/baeus-ui/src/` or `crates/baeus-app/src/`. Only the `Plugin` struct is
consumed in `plugin_manager.rs:1` for rendering. No scan, load, or install path
runs at runtime. ADR 0005 requires plugin discovery and loading.

Evidence: `baeus-ui/src/views/plugin_manager.rs:1` imports only
`baeus_plugins::{Plugin, PluginError, PluginPermission, PluginState}`; zero
references to `PluginLoader`, `PluginRegistry`, or `SandboxedLoader` were found
in `baeus-app/src/` or `baeus-ui/src/`.

**2. `Box::from_raw(create_fn())` without null check** — HIGH

`loader.rs:178`: if the plugin's `_baeus_plugin_create` function returns a null
pointer, `Box::from_raw` produces undefined behaviour. No null check exists
before this call. The `unsafe` block comment documents that the library "must be
compiled against a compatible ABI" but does not address a null return.

Evidence: `crates/baeus-plugins/src/loader.rs:168-178`.

**3. Helm CLI subprocess not implemented** — HIGH

`HelmOperation::to_args()` (`operations.rs:33-115`) generates argument vectors
but no code in `baeus-helm` or elsewhere calls `std::process::Command` to
execute them. `HelmCommandResult` (`operations.rs:128-134`) is defined but never
populated. ADR 0004 states mutating operations should "shell out to helm CLI."

Evidence: `crates/baeus-helm/src/operations.rs:33-134`; no `std::process`
import anywhere in the crate.

**4. LCS diff allocates O(m×n) memory without guard** — HIGH

`diff.rs:103`: `vec![vec![0usize; n + 1]; m + 1]` allocates a full DP table.
For a 10 000-line K8s manifest against a modified version, this is ~800 MB.
There is no line-count check before allocation. K8s CRDs and large Helm values
files regularly exceed this size.

Evidence: `crates/baeus-editor/src/diff.rs:100-103`.

**5. alacritty_terminal not integrated; custom partial ANSI parser used** — HIGH

`emulator.rs:1-7` header comment states "Full ANSI parsing will be added with
alacritty_terminal integration." The implemented parser ignores multi-byte UTF-8
(`emulator.rs:292-298`), simplifies IL/DL (`emulator.rs:501-507`), and has no
sixel or iTerm2 image support. ADR 0003 selected alacritty_terminal specifically
to avoid reimplementing escape-sequence handling.

Evidence: `crates/baeus-terminal/src/emulator.rs:1-7,292-298,501-513`.

**6. Sandbox is structural, not enforced** — MEDIUM

`SandboxedLoader` and the path checks in `PluginLoader::load` prevent loading a
dylib from outside the plugin directory. Once `Library::new()` succeeds
(`loader.rs:159`), the loaded library has full process privileges. The
`allow_network: bool` and `allowed_paths` fields in `SandboxConfig` are
in-process metadata only — they do not restrict OS syscalls. ADR 0005 states
"plugins are sandboxed."

Evidence: `crates/baeus-plugins/src/loader.rs:159-178`;
`crates/baeus-plugins/src/sandbox.rs:11-63`.

**7. KubeExec PTY not bridged to terminal emulator** — MEDIUM

`PtySource::KubeExec` (`pty.rs:15-20`) models the intent, and
`PtyProcess::spawn_shell` (`pty_process.rs:21-65`) implements local shell
spawning. No code bridges a kube-rs WebSocket exec stream to the emulator. ADR
0003 says "for pod exec: kube-rs WebSocket exec pipes through the terminal
emulator instead of a local process."

Evidence: `crates/baeus-terminal/src/pty_process.rs:21-65`;
`crates/baeus-terminal/src/pty.rs:11-20`.

**8. `watch_resources` and `write_resource` are permanent stubs** — MEDIUM

`api.rs:253-293`: `watch_resources` returns a `WatchHandle` that can never
deliver events (no channel, no task). `write_resource` always returns
`InternalError("write_resource not yet implemented")`. Any plugin relying on
either will silently receive no data or a hard error.

Evidence: `crates/baeus-plugins/src/api.rs:253-293`.

**9. `undo_stack` grows without bound** — MEDIUM

`buffer.rs:37`: every `insert` or `delete` pushes to `undo_stack` with no cap.
A session that applies many small edits to a large YAML file will retain the
entire operation history in memory indefinitely. No test covers deep undo stacks
or memory behaviour.

Evidence: `crates/baeus-editor/src/buffer.rs:37-41`.

**10. `PtySession` I/O buffers unbounded** — MEDIUM

`pty.rs:38-39`: `output_buffer` and `input_buffer` use `Vec::new()` with no
capacity limit. A busy shell or a stuck consumer can cause unbounded memory
growth. `PtyManager` has no eviction or backpressure mechanism.

Evidence: `crates/baeus-terminal/src/pty.rs:38-39,66-74`.

**11. tree-sitter not integrated; token mapping is decorative** — MEDIUM

`highlight.rs:1-7` header: "Full tree-sitter integration requires initializing
the tree-sitter parser." `from_node_kind` maps string literals to
`HighlightToken` variants, but no tree-sitter `Parser` or `Language` is ever
initialised. Highlight spans are never produced from real source ranges.

Evidence: `crates/baeus-editor/src/highlight.rs:1-47`.

**12. `parse_semver` rejects pre-release version strings** — LOW

`loader.rs:261-262`: splits on `.` and requires exactly 3 parts.
`"1.0.0-alpha.1"` splits into 4 parts and returns an error, preventing any
plugin published with a pre-release version tag from loading.

Evidence: `crates/baeus-plugins/src/loader.rs:259-263`.

**13. Missing `last_deployed` silently becomes `Utc::now()`** — LOW

`releases.rs:43`: `unwrap_or_else(|_| Utc::now())` — a malformed or absent
`last_deployed` field in the Helm secret displays as the current time with no
log or indication of the fallback.

Evidence: `crates/baeus-helm/src/releases.rs:40-43`.

**14. `latest_version()` trusts index sort order without validation** — LOW

`charts.rs:41`: `versions.first()` assumes the Helm `index.yaml` entry list is
already sorted newest-first. The Helm spec requires this but it is not enforced;
a malformed or third-party index could return a stale version as "latest."

Evidence: `crates/baeus-helm/src/charts.rs:40-42`.

**15. Mid-spawn resource leak in `PtyProcess`** — LOW

`pty_process.rs:53-57`: if `spawn_command` succeeds but `try_clone_reader()` or
`take_writer()` then fails, the child process is abandoned — `PtyProcess::Drop`
never runs because `Self` was never constructed. The child continues running as
an orphan.

Evidence: `crates/baeus-terminal/src/pty_process.rs:53-57`.

**16. `ui_harness` is a stub with no GPUI runtime** — LOW

`ui_harness.rs:6-9`: the module is a `TestContext` struct with two `f32` fields.
The header says "Full GPUI test harness integration requires the gpui crate
dependency." No GPUI render pipeline or event dispatch is testable through it.

Evidence: `crates/baeus-test-utils/src/ui_harness.rs:1-48`.

## Candidate opportunities

- Wire `PluginLoader::scan_directory` + `PluginLoader::load` into the app
  startup path so the plugin manager UI can operate on real loaded plugins
  (addresses F1, F8).
- Add a null-pointer check before `Box::from_raw(create_fn())` and document the
  exact ABI contract plugins must satisfy (addresses F2).
- Implement `execute_helm_operation(op: HelmOperation) -> HelmCommandResult` in
  `operations.rs` using `std::process::Command::new("helm")` (addresses F3).
- Add a line-count guard (e.g., 5 000 lines) in `compute_diff` before
  allocating the LCS table; consider falling back to a hunk-only diff for large
  inputs (addresses F4).
- Complete the alacritty_terminal integration in `emulator.rs` as the ADR
  specifies, replacing the custom parser (addresses F5).
- Bridge `PtySource::KubeExec` to a kube-rs exec WebSocket, feeding its stdout
  bytes into `TerminalEmulator::process_input` (addresses F7).
- Replace `watch_resources` stub with a real broadcast channel backed by the
  core resource store, so plugin watchers receive live events (addresses F8).
- Cap `undo_stack` to a configurable maximum (e.g., 10 000 operations), dropping
  the oldest entries when exceeded (addresses F9).
- Apply a `MAX_BUFFER_BYTES` cap to `PtySession::push_output` and
  `enqueue_input`, returning a backpressure signal when exceeded (addresses F10).
- Initialise a `tree_sitter::Parser` with the `tree-sitter-yaml` grammar in
  `highlight.rs` and produce real `HighlightSpan` values (addresses F11).

## Citations

- `crates/baeus-plugins/src/loader.rs` — plugin loading, unsafe blocks, path
  normalization, version checking
- `crates/baeus-plugins/src/sandbox.rs` — sandbox config, path/permission checks
- `crates/baeus-plugins/src/api.rs` — PluginContext, watch/write stubs
- `crates/baeus-plugins/src/registry.rs` — PluginRegistry lifecycle
- `crates/baeus-plugins/src/lib.rs` — PluginManifest, PluginState, APP_VERSION
- `crates/baeus-helm/src/releases.rs` — base64/gzip/JSON decode pipeline
- `crates/baeus-helm/src/operations.rs` — HelmOperation arg building (no exec)
- `crates/baeus-helm/src/charts.rs` — ChartIndex search, latest_version
- `crates/baeus-helm/src/lib.rs` — HelmRelease, HelmReleaseStatus
- `crates/baeus-terminal/src/emulator.rs` — custom ANSI parser, UTF-8 gap
- `crates/baeus-terminal/src/pty.rs` — PtySession, PtyManager, unbounded buffers
- `crates/baeus-terminal/src/pty_process.rs` — portable-pty spawn, Drop cleanup
- `crates/baeus-editor/src/buffer.rs` — Rope TextBuffer, undo_stack
- `crates/baeus-editor/src/diff.rs` — LCS diff, O(m×n) table
- `crates/baeus-editor/src/highlight.rs` — token enum, no tree-sitter runtime
- `crates/baeus-editor/src/yaml.rs` — validate_yaml, error location parsing
- `crates/baeus-test-utils/src/ui_harness.rs` — stub TestContext
- `crates/baeus-test-utils/src/fixtures.rs` — Resource/Event fixture builders
- `crates/baeus-test-utils/src/mock_cluster.rs` — MockCluster, MockClusterManager
- `.docs/ADR/0003-alacritty-terminal.md` — terminal integration decision
- `.docs/ADR/0004-helm-cli-hybrid.md` — helm hybrid approach decision
- `.docs/ADR/0005-plugin-dylib.md` — plugin sandbox and permission model decision
- `crates/baeus-ui/src/views/plugin_manager.rs` — only Plugin/PluginState imported
- `crates/baeus-ui/src/layout/app_shell.rs` — PtyProcess and helm status usage
