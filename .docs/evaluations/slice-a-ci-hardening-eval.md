# Evaluation: slice-a-ci-hardening.md

Verdict: FAIL
Round: 1
Reviewed against: `.docs/spec/06-remediation-highs.md` (Approved; Slice A = 0006-H1..H4), `.docs/spec/03-toolchain-and-gate.md`, `.docs/research/0006-quality-infra-review.md`, current `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `deny.toml`, workspace `Cargo.toml` / `crates/baeus-app/Cargo.toml`.

## What was verified mechanically (not by eye)

- `cargo metadata --no-deps --format-version 1 | jq -r '.packages | map(select(.name == "baeus-app")) | .[0].version'` → `0.1.0`; `crates/baeus-app/Cargo.toml` uses `version.workspace = true`, workspace `Cargo.toml` has `version = "0.1.0"`. The plan's H4 mechanism satisfies spec 06's "workspace version drives the tag" acceptance. OK.
- `cargo fmt --all -- --check` on the current tree → **fails** (exit 1, diffs in `crates/baeus-app/src/app.rs` and elsewhere). The plan's step-2 pre-landing contingency (in-slice `cargo fmt --all` commit) covers this and matches spec 06 H2 acceptance verbatim. OK — contingency is real and necessary.
- `cargo deny check` (cargo-deny 0.19.0) against the current `Cargo.lock` → **FAILS (exit 5): advisories FAILED, licenses FAILED.** Details below.
- `grep -rn 'v0.1.0-dev' .github/` → only the three `Generate release tag` steps the plan deletes. A4(d) achievable. OK.
- `steps.tag.outputs` references → exactly the four sites (macOS tag_name + name, linux tag_name, windows tag_name) the plan rewrites. No dangling references post-edit. OK.
- Linux `apt-get` list matches `release.yml:112-121` verbatim; macOS/linux/windows tag-step line ranges (73–79, 190–196, 272–279) match the plan's citations. OK.
- `taiki-e/install-action@cargo-deny` follows the same per-tool tag pattern as the existing `@nextest` usage; matrix/include, `needs:` wiring, `$GITHUB_OUTPUT` writes, and `${{ needs.compute-tag.outputs.* }}` expressions are syntactically valid GitHub Actions. OK.

## Findings

- [BLOCKER] **The plan's deny-gate precondition is false against the current tree, and acceptance criterion A1 is unachievable without unplanned, out-of-scope work.** — Verification step 3 asserts "Deny gate is achievable … Must exit zero against the current `Cargo.lock`" and treats failure as hypothetical ("If it surfaces a new advisory or license issue…"). Mechanically verified: it fails **today**, and not marginally:
  - `error[vulnerability]` RUSTSEC-2026-0044 and RUSTSEC-2026-0048 — `aws-lc-sys 0.38.0` (via `rustls 0.23.37`), genuine security vulnerabilities.
  - `error[vulnerability]` — invalid pointer dereference in `fmt::Pointer` (additional crate in the lock).
  - `error[unmaintained]` RUSTSEC-2025-0052 (`async-std`), RUSTSEC-2026-0105 (`core2`, all versions yanked).
  - `error[rejected]` licenses — `0BSD` (×2), `NCSA`, and `Apache-2.0 WITH LLVM-exception` are not in `deny.toml`'s allow list.

  The plan contains **no step** to remediate this. Its only offered remedies are (a) "update `deny.toml` suppressions with a documented reason" — which, applied here, means suppressing *live security vulnerabilities* to make the gate green, directly defeating the purpose of 0006-H1 and the constitution's "dependencies audited on every CI run" (and contradicting the plan's own non-goal, see MAJOR below); or (b) "stop and flag it as a real dependency issue requiring a separate cycle" — which means Slice A **cannot land** as planned. Either way A1 ("a `cargo-deny` check runs on all three OSes and passes") and spec 06's invariant "CI gate stays green … and (after slice A) `cargo deny check`" cannot be satisfied by the steps in this plan. Remediation requires real dependency upgrades (rustls/aws-lc-sys chain, async-std/core2 transitive sources) and/or `deny.toml` allow-list + exception entries — none of which are in spec 06 Slice A's affected files (`.github/workflows/ci.yml` only) and all of which are planning decisions (new slice, reordering, or explicit spec-sanctioned suppressions), not developer improvisation. Per frozen-spec conformance, this must go back through the planner.

- [MAJOR] **Internal contradiction on `deny.toml` edits.** — Non-goals: "This slice adds **no edits to `deny.toml`**; it is only a trigger path." Verification step 3: "either update `deny.toml` suppressions with a documented reason (**still within Slice A** because `deny.toml` is a Slice A concern for making the gate green)…". A cold executor cannot satisfy both. Given the BLOCKER above, this is not hypothetical: the license failures (`0BSD`, `NCSA`, `LLVM-exception`) can *only* be fixed by editing `deny.toml`, so the contradiction is guaranteed to fire on first execution. The plan must state one consistent scope for `deny.toml` (and that scope must be reconciled with spec 06, which lists only `ci.yml` as an H1 affected file).

- [MINOR] **Gate table mislabels the CI lint command.** — The slice gate table lists lint as `cargo clippy --workspace --all-targets -- -D warnings` for "local + CI", but the prescribed CI YAML (step 4) runs `cargo clippy --workspace -- -D warnings` without `--all-targets` (unchanged from current `ci.yml`). The concrete YAML is authoritative and unambiguous, so impact is limited to a misleading summary row.

- [MINOR] **A4 verification narrative is factually wrong about its own trigger.** — "After the PR lands on `main`, the `release.yml` run triggered by that merge shows…". `release.yml`'s `on.push.paths` is `crates/**`, `Cargo.toml`, `Cargo.lock`; this slice's merge touches only `.github/workflows/**`, so **no release run is triggered by that merge**. The first observable run is the next `main` push touching code (or the dry-run/fork route the plan also describes — which spec 06 H4's test expectations actually name as the validation route). The acceptance criteria themselves remain checkable; only the narrative sentence is wrong. (Correctly, the plan does *not* extend `release.yml`'s push paths — spec 06 defers that to medium 0006 §9.)

- [MINOR] **Step 6's "spec 03 needs no change" conclusion is inaccurate.** — Spec 03:26-28 describes CI as "`macos-14`, PRs to main touching crates/ or Cargo manifests" running "lint and tests" — a description this slice makes stale (3-OS matrix, fmt + deny added). The plan's own step-6 escape hatch (raise `Needs Clarification` rather than editing a spec from a slice-plan) is the right mechanism, but the pre-emptive "No change needed" conclusion contradicts the step's stated purpose. Spec 03 is a Draft descriptive back-fill, so severity is low.

## Required changes (for FAIL)

1. Resolve the deny-gate impasse through planning, not improvisation. Either (a) add a slice-ordering/scope decision that remediates the current `cargo deny check` failures first (dependency upgrades for the `rustls`/`aws-lc-sys` advisories and the unmaintained/yanked crates, plus `deny.toml` license entries for `0BSD`, `NCSA`, and the `LLVM-exception` exception), with explicit spec 06 conformance analysis; or (b) obtain an explicit spec-level sanction for a narrowed initial deny scope or a enumerated set of suppressions. The plan must not instruct the developer to suppress live vulnerabilities to make the gate green.
2. Make the `deny.toml` scope statement internally consistent: one authoritative sentence covering whether/when the slice may edit `deny.toml`, reconciled with spec 06's H1 affected-files list.
3. Fix the gate table's lint row so the CI command shown matches the prescribed YAML (or annotate that `--all-targets` is local-only).
4. Correct A4's narrative: the slice's own merge does not trigger `release.yml`; verification occurs on the first subsequent code-touching push to `main` or via the spec-endorsed throwaway-branch dry run.
5. Revisit step 6: either note that spec 03:26-28's CI prose becomes stale and flag it for the next spec revision, or invoke the step's own `Needs Clarification` escape hatch.

## Notes

What the plan gets right (so the revision keeps it): scope selection exactly matches spec 06 Slice A — the paths-filter extension is precisely the two entries spec 06 authorizes and no more, correctly navigating the round-1 spec concern; non-goals otherwise faithfully mirror spec 06's Out-of-scope section including the §9 partial-remediation framing; the `compute-tag` design (single job, four outputs, `baeus-app`-selected version, per-OS cache keys with `runner.os`) is mechanically sound and its deviation from spec 06's illustrative `.packages[0].version` is within the latitude the spec grants ("or `cargo pkgid`") and is verified correct against this workspace; the fmt-failure contingency matches spec 06 H2 acceptance verbatim; the TDD-for-CI approach (PR-as-acceptance-test + dry-run branch) is the only feasible verification shape and matches spec 06's test expectations. The BLOCKER is not the design — it is that the plan's load-bearing precondition ("deny gate is achievable against the current lock") is false today, and the plan's contingency for that case is either harmful (suppress live vulnerabilities) or abortive (stop and escalate), with no planned path to green.


---

# Evaluation: slice-a-ci-hardening.md (re-review, round 2 review of round-1 FAIL)

Verdict: PASS
Round: 1
Reviewed against: `.docs/spec/06-remediation-highs.md` (Approved, frozen; Slice A = 0006-H1..H4), `.docs/spec/03-toolchain-and-gate.md`, `.docs/research/0006-quality-infra-review.md`, prior eval `.docs/evaluations/slice-a-ci-hardening-eval.md` (FAIL, round 1), and the live tree: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `deny.toml`, `Cargo.lock`.

## What was verified mechanically (not by eye) — this round

- `git diff 100f4d8..HEAD -- .docs/slice-plans/slice-a-ci-hardening.md`: the revision adds the Context failure-enumeration, an explicit affected-files superset, the 5-rule Deny-gate triage policy, new step 3a (i–iv), renumbers the CI deny step to 3b, rewrites verification step 3 ("achievable" → "green", slice-blocking), rewrites step 6 (spec 03 staleness + finalize routing), rewrites A4 (two-part dry-run/secondary scheme), fixes the gate-table lint rows, and rewrites the deny.toml non-goal. Steps 1, 2, 4, 5 (ci.yml paths/fmt/matrix, release.yml compute-tag) are unchanged from the version round 1 verified line-by-line.
- `cargo deny check` (cargo-deny 0.19.0) against the checked-in `Cargo.lock` → `advisories FAILED, licenses FAILED`, confirming the revision's Context enumeration **exactly**: 10 distinct `error[vulnerability]` advisories (RUSTSEC-2026-0044, -0048 on aws-lc-sys 0.38.0; -0049, -0098, -0099, -0104 on rustls-webpki 0.101.7/0.103.9; -0258 on h2 0.3.27/0.4.13; -0194, -0195 on quick-xml 0.38.4; -0204 on crossbeam-epoch 0.9.18), 6 `error[unmaintained]` (async-std 2025-0052, core2 2026-0105, paste 2024-0436, proc-macro-error2 2026-0173, rustybuzz 2026-0206, ttf-parser 2026-0192), 4 `error[rejected]` license entries across 3 expressions (0BSD ×2 via enum-iterator/enum-iterator-derive → gpui-component; NCSA via libfuzzer-sys → rav1e; `Apache-2.0 WITH LLVM-exception` via ar_archive_writer only). The plan's counts and provenance comments are accurate.
- Bump feasibility via `cargo update -p … --dry-run` (lockfile untouched):
  - `crossbeam-epoch` 0.9.18 → 0.9.20 — clears -0204 (fix ≥0.9.20). ✓
  - `rustls-webpki@0.103.9` → 0.103.15 — clears -0049/-0098/-0099/-0104 for the 0.103 line (fixes ≥0.103.12/0.103.13); the same resolution cascades aws-lc-rs 1.16.1 → 1.18.0 and aws-lc-sys 0.38.0 → 0.44.0, proving aws-lc-sys ≥0.39 (fix for -0044/-0048) is reachable **within existing constraints**. ✓
  - `h2@0.4.13` → 0.4.19 — clears -0258 for the 0.4 line. ✓
  - `aws-lc-sys` alone → "Locking 0 packages" (aws-lc-rs 1.16.1 pins ^0.38); reachable only via the parent chain — which triage rule 2 explicitly covers ("where the crate is transitive, the direct parent chain"). See MINOR 1.
  - `h2@0.3.27` → 0 packages (no 0.3.x fix exists; fix is ≥0.4.16) → correctly routed to justified-ignore by the plan's own hedge. Pin verified: aws-smithy-http-client 1.1.12 → h2 0.3 (and → rustls 0.21.12 → rustls-webpki 0.101.7); `cargo update -p aws-smithy-http-client --dry-run` caps at 1.1.13 because 1.4.0 requires Rust 1.94.1 (workspace MSRV 1.85) — a genuine transitive pin the slice does not own.
  - `quick-xml` → ambiguous (0.30.0 and 0.38.4 in lock); 0.38.4 cannot reach the ≥0.41.0 fix within ^0.38, and its parent chain (wayland-scanner 0.31.8 proc-macro ← wayland-client ← ashpd 0.11 ← gpui 0.2.2) is pinned outside the slice → justified-ignore per rule 2, with real pin nameable. **No live vulnerability is pre-committed to an ignore without a verified-bump-attempt path, and every survivor has a confirmed unreachable fix.**
- License-remediation syntax: the 3a-ii TOML matches the existing `deny.toml` allow list verbatim plus `"0BSD"`, `"NCSA"`; `[[licenses.exceptions]] name = "ar_archive_writer" allow = ["Apache-2.0 WITH LLVM-exception"]` is valid cargo-deny schema and is correctly scoped — cargo-deny's own output shows `ar_archive_writer` is the **only** crate in the tree carrying the WITH-expression. The four rejections are fully covered; `[bans]`/`[sources]` untouched (already `ok`).
- Combined effect: bumps clear 7 advisory-instances (0044, 0048, 0049, 0098/0099/0104 on 0.103, 0258 on 0.4, 0204); 6 surviving vulnerability ignores + 6 unmaintained ignores all satisfy "no safe upgrade / no in-constraint fix"; license edits cover all 4 rejections. `cargo deny check` exit-zero at slice tip is **achievable as planned**. The round-1 BLOCKER is resolved.

## Prior-findings disposition

- [BLOCKER, round 1] "Deny-gate precondition false; no planned path to green" — **Resolved.** Step 3a is now a mandatory, ordered, per-finding triage preceding 3b; verification step 3 is a slice-blocking green-gate check; the escape hatch (rule 5) replaces "stop and flag" with a defined Needs Clarification route. Required change #1's route (a) is what the revision implements: remediation with an explicit spec 06 conformance analysis (the "CI gate stays green … (after slice A) `cargo deny check`" invariant — genuinely unsatisfiable otherwise — plus the same intrinsic-scope rationale spec 06 itself used for the `paths:` filter). The plan no longer instructs suppression of bump-fixable vulnerabilities: rule 2 makes the bump attempt a precondition for any vulnerability ignore, and my dry-runs confirm every planned-ignore candidate is unfixable within existing constraints.
- [MAJOR, round 1] "deny.toml scope contradiction (non-goal vs verification step)" — **Resolved.** The non-goal now reads "This slice edits `deny.toml`'s `[advisories]` and `[licenses]` sections only … `[bans]` and `[sources]` remain unchanged," and verification step 3 names the triage policy as the recovery path. One authoritative scope statement; internally consistent.
- [MINOR, round 1] gate-table lint row — **Resolved.** Table now splits "lint (CI)" (`--workspace`, no `--all-targets`) from "lint (local)" (`--all-targets`, annotated local-only).
- [MINOR, round 1] A4 narrative — **Resolved.** A4 now correctly states the slice's merge does not itself trigger `release.yml` (push paths untouched), makes the spec-endorsed throwaway-branch/`act` dry run primary, the next code-touching push secondary, and adds the (correct) observation that 3a's `Cargo.lock` bump falls within `release.yml`'s `on.push.paths`, so a lockfile-touching merge self-verifies.
- [MINOR, round 1] step 6 "no change needed" — **Resolved.** Step 6 now enumerates the three counts on which spec 03:26-28 goes stale, correctly refuses to edit an Approved spec from a slice-plan, and routes the follow-up through finalize handoff/roadmap notes plus a Needs Clarification escalation path.

## Findings (this round)

- [MINOR] **Step 3a-i's `cargo update -p aws-lc-sys` bullet is a no-op as written.** — Verified: aws-lc-rs 1.16.1 constrains aws-lc-sys to ^0.38, so the named command updates 0 packages; the advisory clears only via the parent chain (`cargo update -p aws-lc-rs`, verified reachable → aws-lc-sys 0.44.0). Triage rule 2 already instructs parent-chain updates, so a cold executor self-corrects, but the bullet's command is one hop short of its claimed effect.
- [MINOR] **Pin attribution hedges name the wrong parent.** — 3a-i says rustls-webpki 0.101.7 "may be pinned by an older kube-rs transitive"; verified pin is aws-smithy-http-client 1.1.x → rustls 0.21.12 (MSRV-capped at 1.1.13 since 1.4.0 needs Rust 1.94.1). Both bullets are hedged ("may"), and 3a-iii requires the developer to fill in *verified* details from re-check output, so execution is unaffected; the example text misleads.
- [MINOR] **`cargo update -p quick-xml` bullet overclaims.** — "resolves RUSTSEC-2026-0194 and RUSTSEC-2026-0195" is unreachable in-constraint (fix ≥0.41.0, locked at ^0.38 via wayland-scanner ← ashpd ← gpui; also ambiguous without `@0.38.4`). The 3a-i→3a-iii fallback is the actual path for these two advisories; the bullet should have carried the same hedge as the h2 bullet.

## Required changes (for FAIL)

None — no BLOCKERs, no MAJORs.

## Notes

Scope-discipline attack (spec 06 H1 lists only `ci.yml` as an affected file; the slice now edits `deny.toml` and `Cargo.lock`): judged **not** creep-disguised-as-intrinsic. The expansion is openly declared ("Affected files (superset of spec 06's per-finding lists …)"), justified from load-bearing spec text (the "CI gate stays green … (after slice A) `cargo deny check`" invariant, which no ci.yml-only slice can satisfy against this lockfile), mirrors the spec's own precedent for pulling the `paths:` extension into Slice A, and is bound by a 5-rule policy that keeps every suppression per-ID, provenance-named, bump-attempt-first, and expiry-triggered — exactly the planning-level (not developer-improvisation) remedy round 1 demanded. Non-goals correctly continue to exclude `[bans]`/`[sources]` tightening, SHA pinning, broader trigger redesign, and any `Cargo.toml` constraint edits. Round-0 invariants re-confirmed: the four Slice A findings retain their round-1-verified steps; GH Actions semantics unchanged in the verified regions; the TDD/verification shape (PR-as-acceptance-test + dry-run branch + slice-blocking local green gate) is coherent and consistent with spec 06's test-expectation exception for pure workflow/config changes (no Rust code touched, test count invariant holds). Escape hatch + acceptance 1c ("escape hatch invoked ⇒ criterion failed ⇒ not mergeable") is the correct failure semantics for the one residual unknowable (a future advisory surfacing mid-slice).


---

# Code evaluation (round 2)

# Evaluation: slice-a-ci-hardening (code review)

Verdict: FAIL
Round: 2
Reviewed against: `.docs/slice-plans/slice-a-ci-hardening.md` (Implemented),
`.docs/spec/06-remediation-highs.md` (Approved; Slice A = 0006-H1..H4),
`.docs/spec/03-toolchain-and-gate.md`, diff `git diff 9ad8d57..HEAD` on branch
`slice/a-ci-hardening`, `.docs/evaluations/slice-a-ci-hardening-review-findings.md`.
Round counting: prior plan-eval FAIL on this artifact was Round 1
(`.docs/evaluations/slice-a-ci-hardening-eval.md`); per the one-counter-per-artifact
rule, this code-eval FAIL increments to Round 2.

## Gate re-run (performed, not trusted)

| Step | Command | Result |
|------|---------|--------|
| format | `cargo fmt --all -- --check` | PASS (exit 0) |
| lint (spec 03 local) | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | **FAIL (exit 101)** — 46+ errors |
| lint (CI shape) | `cargo clippy --workspace -- -D warnings` | PASS (exit 0) |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` | PASS (exit 0; 76 binaries, 3,680 passed, 0 failed) |
| deny (slice-specific) | `cargo deny check` | PASS (exit 0; advisories/bans/licenses/sources ok) |
| yaml | `actionlint .github/workflows/ci.yml .github/workflows/release.yml` | PASS (exit 0) |

(First-pass runs of clippy/test/deny were piped through `tail`, masking cargo's
exit code; all were re-run with direct exit-code capture. The table reflects the
unmasked results.)

The clippy failure detail (current stable clippy 0.1.94; no rust-toolchain.toml
pin exists, so the gate floats on stable):

- `field_reassign_with_default` ×39 — `crates/baeus-ui/src/views/plugin_manager.rs`,
  `components/resource_map.rs`, `views/crd_browser.rs`, `views/events.rs`,
  `views/helm_install.rs`, `crates/baeus-app/src/settings.rs`, etc. (lib tests,
  bin tests, integration tests)
- "this expression always evaluates to false" ×3 — `resource_map_render_tests.rs`
- `function ctrl_key is never used` ×1 — `terminal_view_render_tests.rs`
- 9 targets fail to compile under `-D warnings`: `baeus-ui` lib test, `baeus-app`
  bin test, and 7 `baeus-ui` integration test files.

Spot-verified the linted patterns are pre-existing at `9ad8d57` (e.g.
`plugin_manager.rs` old lines 744-745, 757-758) — this is toolchain drift on
stable clippy, not slice-introduced code. That does not clear the slice: see
BLOCKER 1.

## Findings

- [BLOCKER] Spec-03 lint gate is red at the slice tip —
  `cargo clippy --workspace --all-targets -- -D warnings` exits 101 with 46+
  errors. The slice-plan's own Verification §Local step 4 makes this exact
  command a slice acceptance floor ("These must pass unchanged on macOS; they
  are the invariant floor spec 06 restates"), and spec 03's gate table defines
  lint with `--all-targets`. The plan's "known scope gap" sentence documents
  only that **CI** omits `--all-targets`; it does not waive the local gate —
  the plan's documented handling therefore does not cover shipping with this
  command red, and the commit message claim "gate green (Implemented)" is not
  reproducible. The developer demonstrably accepted responsibility for this
  gate in-slice (ten files received behavior-neutral `--all-targets` lint
  fixes: `useless_vec`, `len_zero`, `expect_fun_call`, unused-import removal,
  byte-str comparisons) but stopped short, leaving 46+ errors of the same
  drift class unfixed. A red gate is an automatic BLOCKER; landing as-is would
  leave every subsequent slice inheriting a red local gate.

- [MINOR] Scope-text gap: the ten files carrying clippy fixes exceed the
  plan's "no `crates/**` edits" change list, which authorized only a
  `cargo fmt --all` commit. Verified the fixes are gate-necessary (a probe
  crate with the old patterns errors under the installed clippy) and
  behavior-neutral (all 3,680 tests pass; the other 130 changed `crates/`
  files verified byte-identical to `rustfmt(old)`). Recorded as a plan-text
  gap — the plan should have authorized "lint fixes required by the
  `--all-targets` gate" the same way it authorized the fmt commit — not as a
  code defect.

- [MINOR] Plan step 6's documentation obligation (record in
  `handoff.md`/`roadmap.md` that spec 03:26-28's CI prose is now stale and
  queue a spec-03 revision cycle) is not yet present in the diff. This is
  finalize-pass territory (the finalize pass runs after a PASS verdict), so it
  is recorded as an outstanding finalize obligation, not a current defect.
  Note: spec 03 is `Status: Draft` (not Approved as the plan's step 6
  assumed), so the frozen-spec concern is moot.

## Fidelity to plan & specs (verified mechanically)

- 0006-H1 — `on.pull_request.paths:` gains exactly `deny.toml` and
  `.github/workflows/**` (matches plan step 1 verbatim); `Install cargo-deny`
  (`taiki-e/install-action@cargo-deny`, same `@vN`-style as existing `@nextest`)
  sits between Install Rust and Install cargo-nextest; `cargo deny check` step
  runs after Format check and before Clippy. `cargo deny check` exits 0
  (re-run). Triage complies with the plan's binding policy: every
  `[advisories].ignore` addition is per-ID with provenance + pin + removal
  trigger; the three pins were confirmed in `Cargo.lock` (hyper 0.14.32 →
  `h2 0.3.27`; rustls 0.21.12 → `rustls-webpki 0.101.7`; wayland-scanner
  0.31.8 → `quick-xml 0.38.4`); bumps applied where reachable (aws-lc-sys
  0.38.0→0.44.0, aws-lc-rs →1.18.0, `h2 0.4.13→0.4.19`, `rustls-webpki
  0.103.9→0.103.15`, crossbeam-epoch 0.9.18→0.9.20) — no workspace or crate
  `Cargo.toml` edits. Licenses: `0BSD`/`NCSA` added to `[licenses].allow` with
  OSI-status comments; `Apache-2.0 WITH LLVM-exception` scoped via
  `[[licenses.exceptions]]` to `ar_archive_writer` only. `[bans]`/`[sources]`
  untouched, per non-goals.
- 0006-H2 — `components: rustfmt, clippy`; `Format check` is the first gate
  step; in-slice fmt commit `1cddd73` present; tree passes fmt (re-run).
- 0006-H3 — `check` job is a `fail-fast: false` matrix over
  macos-14/ubuntu-latest/windows-latest with per-OS targets; Linux dep list
  matches `release.yml`; cache key is `${{ runner.os }}-ci-${{ hashFiles }}`;
  `RUST_MIN_STACK` retained on Clippy and Test.
- 0006-H4 — single `compute-tag` job emits `tag/version/date/short_sha`;
  version via `cargo metadata … select(.name == "baeus-app")`; all three
  build jobs `needs:` it and consume `needs.compute-tag.outputs.tag`; the
  three per-runner `Generate release tag` steps are deleted; `grep -rn
  'v0.1.0-dev' .github/` → no matches.
- Scope — the change surface is exactly `.github/workflows/ci.yml`,
  `.github/workflows/release.yml`, `deny.toml`, `Cargo.lock`, the 140
  fmt/lint-only `crates/` files, and `.docs/` status/review artifacts
  (slice-plan status line, handoff, review-findings — recorder/orchestrator
  territory). No spec/ADR edits, no workspace `Cargo.toml` edits.
- Test expectations — infra slice; spec 06's "no new unit tests; verification
  is CI itself" applies; test count undecreased (3,680 passing). Local
  verifications the plan mandates (actionlint, fmt, deny) all re-run green;
  remote A1–A4 acceptance is PR-time evidence outside the checkout.

## Review-findings adjudication

- `/security-review` — Status: ran-clean. **Confirmed** as consistent with my
  own read: no blanket suppressions in `deny.toml` (every ignore is per-ID
  with justification), workflow `permissions:` unchanged (`contents: read` on
  ci.yml; pre-existing `contents: write` on release.yml), no new secret
  handling, install-action pinning matches the spec-06-sanctioned `@vN`
  pattern. Nothing to escalate.
- `/code-review` — Status: skipped: command-unavailable. Per the rubric this
  is informational, not a finding and not a clean review; the dimension was
  covered manually above (diff-vs-plan fidelity, YAML expression correctness,
  lockfile pin verification, whitespace/format-only verification of the
  `crates/` surface).

## Required changes (for FAIL)

1. Make `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
   -D warnings` exit 0 on the slice branch — fix the 46+ drift lints
   (`field_reassign_with_default`, always-false comparisons, dead `ctrl_key`)
   with the same behavior-neutral minimal-edit class already used for the ten
   files in this slice.
2. Re-run the full spec 03 gate unmasked (no `| tail` pipelines swallowing
   exit codes) and record honest gate evidence for the slice.

## Notes

The blocking defect is not the workflow YAML, the deny triage, or the lockfile
work — those were verified correct against the plan and spec 06. The slice
fails solely because its own mandated local gate is red at the tip while the
recorded claim says green; the fix is mechanical and confined to test/bin
targets.
