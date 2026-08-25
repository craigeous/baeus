# Evaluation: 0005-supporting-crates-review

Verdict: PASS
Round: 0
Reviewed against: cited source files in `crates/baeus-plugins`, `crates/baeus-helm`, `crates/baeus-terminal`, `crates/baeus-editor`, `crates/baeus-test-utils`, `crates/baeus-ui/src/views/plugin_manager.rs`, and ADRs 0003/0004/0005.

## Findings

- [MINOR] "no `std::process` import anywhere in the crate" (F3) — verified by
  `grep -n "std::process\|process::Command\|Command::new" crates/baeus-helm/src/*.rs`
  returning no matches. Author could optionally cite this as a mechanical check
  rather than a bare assertion, but the invariant is factually true.
- [MINOR] F13 "silently becomes `Utc::now()`" is precise. F11 phrasing "no
  tree-sitter `Parser` or `Language` is ever initialised" — verified: the file
  contains only enum + `from_node_kind` mapping. Accurate but the reader has to
  take the "ever initialised" scope on faith; a passing `grep -rn tree_sitter
  crates/baeus-editor/src` would tighten it. Not blocking.
- [MINOR] "50 MB decompressed-size cap via `.take()` before reading" cites
  `releases.rs:10,24` — line 10 is the const, line 24 is the `.take()`, both
  match. Slightly non-standard citation format (comma list) but unambiguous.

## Required changes (for FAIL)

(none — verdict is PASS)

## Notes

Mechanical spot-checks performed against the working tree at
`/Users/craig.pfeiffer/git/baeus`:

- F1: `grep -rn 'PluginLoader\|PluginRegistry\|SandboxedLoader'
  crates/baeus-ui/src/ crates/baeus-app/src/` returned zero matches — claim
  supported. `plugin_manager.rs:1` import list matches the note verbatim.
- F2: `crates/baeus-plugins/src/loader.rs:178` is exactly
  `Box::from_raw(create_fn())` inside the `unsafe` block starting at 168; no
  null check between the symbol lookup and the deref. Confirmed.
- F3: `HelmOperation::to_args` spans 33–115 as claimed; `HelmCommandResult`
  is defined at 128–134; no `std::process` import (grep). Confirmed.
- F4: `crates/baeus-editor/src/diff.rs:103` is
  `let mut dp = vec![vec![0usize; n + 1]; m + 1];` — full O(m*n) allocation,
  no size guard in `compute_diff` before the call at line 41. Confirmed.
- F5: emulator.rs header at 1–2 matches the "Full ANSI parsing will be added
  with alacritty_terminal integration" quote. 292–298 UTF-8 comment matches
  ("Just skip non-ASCII for now"). 501–507 IL/DL simplified to `clear_line()`.
  Confirmed.
- F6: sandbox.rs 11–20 defines the metadata fields as described; nothing in
  `SandboxedLoader` enforces OS syscalls. Confirmed.
- F7: `PtySource::KubeExec` at pty.rs:15–20; `PtyProcess::spawn_shell` at
  pty_process.rs:21–65 spawns via `portable_pty` only — no kube-rs bridge.
  Confirmed.
- F8: api.rs:253–293 shows `watch_resources` returning a `WatchHandle` with
  no channel/task and `write_resource` returning the "not yet implemented"
  `InternalError`. Confirmed verbatim.
- F9: buffer.rs:37–41 `undo_stack.push(...)` inside `insert`, no cap
  anywhere in the file. Confirmed.
- F10: pty.rs:38–39 buffers are plain `Vec::new()`; `PtyManager` at 100–153
  has no eviction. Confirmed.
- F11: highlight.rs 1–7 header matches; `from_node_kind` at 26–39 is pure
  string→enum, no `Parser`/`Language`. Confirmed.
- F12: loader.rs:261–262 rejects `parts.len() != 3`; "1.0.0-alpha.1" would
  split into 4 parts. Confirmed by inspection.
- F13: releases.rs:41–43 `unwrap_or_else(|_| Utc::now())` — no log/warn.
  Confirmed.
- F14: charts.rs:40–42 `versions.first()` with no sort. Confirmed.
- F15: pty_process.rs:53–57 constructs `child`, `reader`, `writer` before
  `Self { ... }` at 59; a `?` on 56 or 57 leaves `child` un-`Drop`ped since
  `Self` never exists. Confirmed.
- F16: ui_harness.rs is 48 lines, exactly matches "stub TestContext with two
  f32 fields" and quoted header. Confirmed.
- Strengths: PtyProcess::Drop at 111–118 kills+waits; validate_shell_path at
  155–190 (Unix branch) reads `/etc/shells`; decode_helm_release `.take()` at
  line 24 with MAX const at line 10. All confirmed.
- ADR quotes: 0003 line 22 exact "for pod exec: kube-rs WebSocket exec
  pipes through the terminal emulator instead of a local process"; 0004 line
  20 "shell out to `helm` CLI"; 0005 line 21 "Plugins are sandboxed". All
  verified.

Format: `Status: Research Review` present; Summary/Strengths/Findings/
Candidate opportunities/Citations sections all present; severity tags used
consistently (HIGH/MEDIUM/LOW). Scope discipline is clean — "Candidate
opportunities" is worded as options addressing each finding, not smuggled
decisions or task commitments.

Every load-bearing claim carries a file:line citation, and every high-severity
citation resolves to code that supports the claim as written. No BLOCKER or
MAJOR issues found.
