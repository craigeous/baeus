# Evaluation: 0003-ui-layout-shell-review

Verdict: PASS
Round: 0
Reviewed against: cited source files under /Users/craig.pfeiffer/git/baeus/crates/baeus-ui/src/layout/ (primarily app_shell.rs and sidebar.rs)

## Findings

- [MINOR] Finding #1 body says "8 separate `impl AppShell` blocks" then enumerates 11 line numbers (638, 967, 1536, 2997, 3140, 3350, 3694, 10451, 10633, 10859, 11045). Verified via `grep '^impl AppShell'` — actual count is 11. The listed line numbers are all correct; only the word "8" is wrong. Purely a counting typo; the substance of the god-object claim is fully supported.
- [MINOR] Finding #3 says `KeybindingConfig::default_bindings()` allocates "a `Vec<KeyBindingEntry>` of 20 entries" — the actual vec (app_shell.rs:231-338) contains 21 `KeyBindingEntry` values. Off-by-one; the underlying claim (per-keydown allocation of a heap Vec with String fields) remains fully supported.
- [MINOR] Finding #5 cites `generate_eks_kubeconfig_file_with_role (app_shell.rs:11484)` — the function definition is at line 11419; line 11484 is where the sync `std::fs::write` call inside that function lives. The pointed-at behavior (sync write on main thread) is correctly located; only the fn-definition line label is off. Similar micro-offset in Strengths list (2431 vs the actual `tokio_handle.spawn` at 2402) — the async-pattern claim is still supported nearby.
- [MINOR] Finding #6 phrases the YAML `serde_yaml_ng::to_string` call (app_shell.rs:9015) as "guarded by a lazy-init check, but the guard is also evaluated per frame". Verified: the call is inside `if !self.yaml_editors.contains_key(&key)`, so `to_string` runs only on first entry, though the containment check itself is per frame. The four `extract_*` calls (9080/9090/9102/9114) do run every frame as claimed.

## Required changes (for FAIL)

N/A — verdict is PASS.

## Notes

Sources-match-claims check performed against the repo at /Users/craig.pfeiffer/git/baeus.

High-severity items verified thoroughly:
- `wc -l app_shell.rs` = 11692 (matches the "11,692 lines" claim in Summary and Finding #1).
- Finding #1 field citations spot-checked: `pty_processes`/`pty_output_buffers` at 508/510, drag fields at 512-526 and 605, `yaml_editors`/`yaml_editor_focus_handles` at 557/559, AWS SSO fields `pending_sso_login`/`eks_wizard`/`eks_cluster_data` at 617/623/627 — all correct.
- Finding #2 verified: `render_resource_table_body_filtered` at 7992, `rows.iter().take(200)` at 8021, hard cap message at 8030-8036, no `uniform_list` used; navigator `uniform_list` pattern verified at 5029-5049.
- Finding #4 verified: `AppShellState` at 3251 with single `focus_mode` field; `AppShell.focus_mode: FocusMode` at 451; `AppShellState::handle_key_action` at 3257 and `AppShell::handle_keyboard_shortcut` at 3352 both implement the same `ToggleCommandPalette` / `FocusSearch` toggle pattern in parallel.
- Finding #5 verified: `persist_cluster_appearances` at 4533 (sync `std::fs::write`), `load_cluster_appearances` at 4547 (sync `std::fs::read_to_string`), sync `Command::new("open"|"explorer.exe"|"xdg-open").spawn()` at 4846-4863 inside `on_click`.
- Finding #7 verified: identical 12-entry `[u32; 12]` palettes at app_shell.rs:4562-4575 and sidebar.rs:116-129 (`0x3B82F6, 0x10B981, ...` identical order).
- Strengths spot-checks confirmed: `sanitize_error_message` at 419 with three `LazyLock<Regex>` at 423/425/427.

Format conformance: Status line "Research Review" present; Summary/Strengths/Findings/Candidate opportunities/Citations sections match a research-note shape. Scope discipline is fine — "Candidate opportunities" are suggestions framed as options, not smuggled decisions. All cited files exist under crates/baeus-ui/src/layout/ and crates/baeus-ui/src/lib.rs.

The minor line-number and count offsets above are hygiene-only; none change the direction of any finding. Author may want to fix these on the next revision but they do not block landing under the light-check gate for research notes.
