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


---

# Evaluation: 06-remediation-highs.md (re-review, round 2)

Verdict: PASS
Round: 1
Reviewed against: research notes 0002–0006 (all Approved 2026-08-25); ADRs 0001–0006
(Accepted); constitution/non-negotiables in `.docs/spec/README.md`; prior evaluation
`.docs/evaluations/spec-06-remediation-highs-eval.md` (FAIL, round 1); diff
`5f04621..HEAD` of the artifact; loom playbook plan-eval rubric + spec template.
All new/changed line-level claims re-verified mechanically against the code tree.

## Prior-findings resolution (each proven against the diff and the tree)

- [MAJOR — RESOLVED] 0006-H1 scope contradiction. The revision takes required-change
  option (a): a new **"In-scope `paths:` filter extension"** subsection pulls exactly
  the minimal extension (`deny.toml`, `.github/workflows/**`) into slice A as
  intrinsic to 0006-H1; the acceptance criterion is rewritten to match (filter
  includes all five paths; every PR touching them triggers `cargo deny check`); and
  Out of scope now carries the explicit exception, naming medium 0006 §9 as
  **partially remediated by slice A** with broader trigger-path redesign deferred.
  Mechanically verified: `ci.yml:6-9` filter is exactly `crates/**`, `Cargo.toml`,
  `Cargo.lock` (sed); research 0006 finding 9 (line 104) is Severity: medium and its
  evidence is precisely that workflow/`deny.toml` changes do not trigger CI — so the
  "partially remediated" framing is accurate and conservative (the two added paths
  are §9's exact evidence; under-claiming full remediation is scope-safe). AC,
  in-scope subsection, and Out-of-scope text are now mutually consistent; a
  slice-planner can satisfy all three simultaneously. Genuine fix, not rewording.
- [MINOR — RESOLVED] "disjoint file sets" claim. Slice Breakdown now reads
  "sequentially ordered so file overlaps do not cause conflicts… near-disjoint on
  primary files but not mechanically disjoint" and enumerates the two overlap groups
  with required ordering. Verified against each finding's Affected files:
  B ∩ C = {`aws_eks.rs` (B:H2 / C:H4 tweaks), `client.rs` (B:H1,H2 / C:H3
  propagation), `baeus-core/Cargo.toml` (B dep add / C dev-deps)} ✓; D/F/G all touch
  `app_shell.rs` (D: 0005-H1 registry-handle injection — listed in 0005-H1 Affected
  files; F: 0003-H2 table body; G: 0003-H1 decomposition) ✓. All named files exist.
- [MINOR — RESOLVED] Slice C row now lists `crates/baeus-core/src/{aws_sso.rs,
  client.rs, aws_eks.rs}`, the new test file, and `Cargo.toml` (dev-deps) — matches
  0002-H3 + 0002-H4 Affected files exactly. Slice B row likewise gained
  `baeus-core/Cargo.toml` (0002-H1 dependency add).
- [MINOR — RESOLVED] tokio-util dependency claim re-keyed correctly: "not currently a
  direct dependency of `baeus-core` (verified against `crates/baeus-core/Cargo.toml`:
  only `tokio.workspace = true`…; appears in `Cargo.lock` transitively but that does
  not make it importable)". Verified: `grep tokio crates/baeus-core/Cargo.toml` → only
  `tokio.workspace = true` (+ dev-dep test-util); `tokio-util 0.7.18` present in
  `Cargo.lock` transitively. Directive now says add under `[dependencies]` (or promote
  to workspace) — correct.
- [MINOR — RESOLVED] 0004-H2 now binds the enumeration test to
  `resource_table::columns_for_kind` by explicit path, citing
  `components/resource_table.rs:674` (verified: `pub fn columns_for_kind` at line 674;
  imported at `json_extract.rs:9`) vs the duplicate at `views/resource_list.rs:230`
  (verified: second `pub fn columns_for_kind` at line 230). The new Drift-hazard note
  defers dedup as medium and explains the explicit-path binding — exactly the fix the
  finding asked for.
- [MINOR — RESOLVED] 0005-H4 Finding line now attributes the O(m×n) allocation to
  `longest_common_subsequence` (called by `compute_diff`). Verified: fn starts at
  `diff.rs:100`, allocation `vec![vec![0usize; n + 1]; m + 1]` at line 103; guard is
  placed at the `compute_diff` entry and the helper is explicitly not modified.

## Fresh attack on the revision

- No new contradictions: the revised 0006-H1 AC, the in-scope subsection, and the
  Out-of-scope exception name the same two added paths and the same five-path filter.
- Round-0 invariants re-confirmed: slice table covers all 17 highs exactly once
  (A:4, B:2, C:2, D:4, E:1, F:3, G:1 = 17); no scope creep beyond the explicitly
  justified §9 sliver (the mandated resolution); no-new-ADR judgment unchanged and
  sound; every slice remains gate-able (fmt/clippy/test/deny per Invariants).
- Overlap claims and all new line citations verified mechanically (above).

## Findings

- [MINOR] 0004-H2 "all 34 resource kinds" / "34 individual assertions (one per kind)"
  is stale against the tree — `resource_table::columns_for_kind` has **37** named
  kind arms (plus a `_ =>` catch-all), and `json_to_table_row` dispatches on the same
  37 (verified by arm enumeration, `resource_table.rs:674-973`). The test
  specification itself is count-agnostic ("every resource kind covered by
  `resource_table::columns_for_kind`"), so planning is not blocked; correct the
  number or drop it.
- [MINOR] Ordering-rationale prose "D touches five unrelated files across four
  crates" is stale against the revised slice-D table row, which now names six files
  across five crates (`app_shell.rs` added this round). Cosmetic; the operative
  table is correct.

## Notes

All six round-1 findings are genuinely resolved with tree-accurate text; the two new
MINORs are pre-existing/stale counts surfaced by this round's mechanical verification
and do not block approval. Everything round 1 marked approval-quality (coverage,
ID-mapping, ADR judgment, TDD conformance with the sound workflow-only exemption,
ordering rationale, template conformance) remains intact.
