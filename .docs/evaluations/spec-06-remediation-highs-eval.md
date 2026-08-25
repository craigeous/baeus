# Evaluation: 06-remediation-highs.md

Verdict: FAIL
Round: 1
Reviewed against: research notes 0002–0006 (all Approved 2026-08-25); ADRs 0001–0006
(Accepted); constitution/non-negotiables in `.docs/spec/README.md`; specs 01–05
(consistency); loom playbook plan-eval rubric + spec template. All line-level claims
verified mechanically against the code tree (wc/grep/sed).

## Coverage (criterion a) — complete

17 highs enumerated in the research notes: 0002 ×4 (findings 1–4), 0003 ×2 (findings
1–2), 0004 ×2 (findings 1–2), 0005 ×5 (findings 1–5), 0006 ×4 (findings 1–4). The spec
maps all 17 with correct IDs (0002-H1..H4, 0003-H1..H2, 0004-H1..H2, 0005-H1..H5,
0006-H1..H4); H-numbering follows document order of High-severity findings in each
note. Slice table accounts for all 17 exactly once (A:4, B:2, C:2, D:4, E:1, F:3, G:1).
Mediums/lows correctly excluded, with explicit deferrals (0002 §7 AbortHandle, 0005 §7
exec bridge, 0006 §5 signing, §7 SHA pinning, §12 .sdlc).

## Accuracy spot-checks (criterion b) — verified

- `client.rs:1181/1246` watch fns ✓; `aws_eks.rs:774` X-Amz-Expires=60 ✓;
  `aws_sso.rs:80-81,139-140` double-nested `if let Some` ✓; `cluster.rs:39`
  `token_expiry` ✓.
- `app_shell.rs` = 11,692 lines ✓; `render_resource_table_body_filtered` at 7992 with
  `take(200)` at 8021 ✓; navigator `uniform_list` at 5029 ✓; AppShell struct has 63
  fields (437–636) ✓ "60+"; drag+pty region 508–611 holds 33 fields, EKS region 6 —
  the ≤25-field post-extraction target is arithmetically feasible.
- `log_viewer.rs:918` `render_log_body` ✓; `json_extract.rs:187-193`
  `debug_assert_eq!` ✓ (imports `columns_for_kind` from `resource_table.rs:674`).
- `loader.rs` `Box::from_raw(create_fn())` with no null check ✓; `diff.rs:103`
  `vec![vec![0usize; n+1]; m+1]` ✓; `operations.rs:33` `to_args`, no `std::process` ✓.
- `ci.yml:16` macos-14 ✓; `ci.yml:24` components: clippy only ✓; `ci.yml:6-9` paths
  filter = crates/**, Cargo.toml, Cargo.lock ✓; `release.yml` three
  `v0.1.0-dev.${DATE}.${SHORT_SHA}` generators at 73/190/272 (spec cites 74-79 etc.,
  off by one — non-material) ✓; `release.yml` already runs ubuntu/windows jobs, so the
  0006-H3 matrix is feasible ✓.

## ADR judgment (criterion c) — sound

"No new ADR" is correct per group: 0002-H1/H2 are mechanics inside ADR 0002 + ADR 0006
(whose Consequences literally require explicit background-task cancellation);
0003-H1/H2 and 0004-H1 mirror the existing `uniform_list`/sub-struct patterns plus the
constitution's virtual-scrolling non-negotiable; 0005-H1/H2 conform to ADR 0005;
0005-H3 conforms to ADR 0004 including its "handle CLI absence gracefully" consequence;
0005-H5 conforms to ADR 0003's mandated architecture (exec bridge correctly excluded
as medium 0005 §7); 0006-H1 is verbatim constitution compliance. The escape hatch
(raise an ADR if slice-planning surfaces a genuine open decision) is appropriate.

## Findings

- [MAJOR] 0006-H1 acceptance criterion contradicts the spec's own scope boundary —
  the AC requires "Every PR that touches crates/**, Cargo.toml, Cargo.lock, deny.toml,
  or the workflow itself triggers a `cargo deny check` run." Verified: `ci.yml:6-9`
  today filters to `crates/**`, `Cargo.toml`, `Cargo.lock` only, so satisfying this AC
  forces extending the `paths:` filter — the exact substance of **medium** finding
  0006 §9 ("CI `paths:` filter excludes `.github/workflows/` changes"). The Out of
  scope section defers "Medium- and low-severity findings from all five research
  notes" without qualification and names 0006 §5, §7, §12 as deferred — §9 is absent
  while its substance is required. A slice-planner cannot satisfy both the AC and the
  scope statement: either they creep into a deferred medium or the AC fails. (The
  spec's demonstrated care elsewhere — deferring §7 SHA pinning while using
  `taiki-e/install-action` — makes the §9 omission read as oversight, not judgment.)
- [MINOR] "disjoint file sets" claim in Slice Breakdown fails mechanically — B and C
  share `aws_eks.rs`, `client.rs` (0002-H3's Affected files), and
  `baeus-core/Cargo.toml`; D (0005-H1's registry-handle injection), F, and G all touch
  `app_shell.rs`. Sequential ordering with stated rationale (B→C, F→G) makes this
  safe, but the invariant as worded is false; restate as "sequentially ordered to
  avoid file conflicts" or soften to "near-disjoint primary files."
- [MINOR] Slice C's table row omits `crates/baeus-core/src/client.rs` even though
  0002-H3's Affected files list it (`create_client_from_path_with_aws_creds` error
  propagation). "Primary crates / files" hedging covers it, but aligning the row
  avoids slice-plan confusion.
- [MINOR] 0002-H1's "verify from `Cargo.lock`" for tokio-util is muddled — tokio-util
  IS in `Cargo.lock` (transitive; verified) but is NOT a direct dependency of
  `baeus-core` (only `tokio.workspace`); lock-file presence does not make it
  importable. The conditional should key on `baeus-core/Cargo.toml`.
- [MINOR] 0004-H2 — `columns_for_kind` has two definitions: `resource_table.rs:674`
  (the one `json_to_table_row` imports, verified at json_extract.rs:9) and a duplicate
  at `resource_list.rs:230`. The enumeration test should name
  `resource_table::columns_for_kind` explicitly; the unmentioned duplicate is itself a
  drift hazard of exactly the class the finding guards.
- [MINOR] 0005-H4 citation imprecision — the O(m×n) allocation lives in
  `longest_common_subsequence` (`diff.rs:103`), not directly in `compute_diff` as the
  Finding line states. Guard placement is unaffected.

## Required changes (for FAIL)

1. Resolve the 0006-H1 scope contradiction: either (a) state explicitly that the
   minimal `paths:` filter extension (adding `deny.toml` and `.github/workflows/`) is
   in scope for 0006-H1 as an intrinsic part of making the deny gate fire, and note
   0006 §9 as partially remediated by slice A in Out of scope; or (b) narrow the
   acceptance criterion to the existing trigger paths and leave §9 wholly to the
   later medium cycle.

## Notes

Everything else is approval-quality: coverage is complete and ID-mapped; every line
reference spot-checked against the tree is accurate; per-finding acceptance criteria
and test expectations satisfy the constitution's TDD non-negotiable (with a sound,
explicit exemption for workflow-only changes); slice ordering rationale is correct
(A's gate-first landing, B before C for API shape, F before G within app_shell.rs);
the no-new-ADR judgment is well-argued per group with a proper escape hatch; and the
format conforms to the spec template (Status: Plan Review, Authority, Design,
Invariants, Out of scope; the spec README index already lists it). The single MAJOR
is a one-paragraph fix.
