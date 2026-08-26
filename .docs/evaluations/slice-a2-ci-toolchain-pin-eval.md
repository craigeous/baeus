# Evaluation: slice-a2-ci-toolchain-pin

Verdict: FAIL
Round: 1
Reviewed against: `.docs/spec/06-remediation-highs.md` (Approved 80f8049 +
2026-08-26 amendment, diff-verified additive-only), owner direction recorded in
`.docs/slice-plans/archive/slice-a-ci-hardening.md#landed-receipt`, the real
`.github/workflows/ci.yml` / `release.yml`, the live `dtolnay/rust-toolchain`
action repo (refs + `action.yml` on `master`, `stable`, `1.98.0`), the live
`static.rust-lang.org/dist/channel-rust-stable.toml`, and GitHub Actions
`GITHUB_TOKEN` / `workflow_dispatch` documented behaviour.

## What checked out (verified mechanically, not by eye)

- **Amendment discipline (spec 06).** `git diff 80f8049..HEAD` shows the
  amendment is additive-only: new "Amendments" section, new "Slice A2" section,
  one A2 row in the Slice Breakdown table, one A2 ordering-rationale bullet.
  No pre-existing Approved content rewritten or contradicted. The amendment is
  dated, names owner direction as authority, confines its scope, and its claim
  "No other section of this spec is altered" is true. The owner-direction
  citation resolves (slice A landed receipt, "Follow-up queued by owner").
- **Slice-plan ↔ amendment match.** Plan takes the spec-deferred options
  exactly as the spec allows: includes `rust-toolchain.toml` (spec: "optional,
  recommended by the slice-plan"), picks monthly cadence (spec: "owner
  direction defaults to monthly"), records cadence at the pin-site inline
  comment (a spec-sanctioned location), queues the spec 03 revision as a
  follow-up instead of editing spec 03 (spec: explicitly admissible).
- **Pin mechanism valid.** `dtolnay/rust-toolchain@1.98.0` is a real branch
  (verified via `git ls-remote`); its `action.yml` hardcodes `toolchain:
  1.98.0` (line 51 of the branch's action.yml) and still accepts
  `targets:`/`components:` inputs, so the planned `@stable → @1.98.0` swap
  installs rustc 1.98.0 on every matrix leg without breaking
  `targets: ${{ matrix.target }}` or the `taiki-e/install-action@nextest`
  install. Current live stable is exactly 1.98.0 (channel manifest:
  `1.98.0 (88d9e12ae 2026-08-18)`), so the plan's default pin selection
  applies with no drift conflict.
- **Local/CI parity claim valid.** rustup honours `rust-toolchain.toml`
  (channel + components + targets); `components = ["rustfmt", "clippy"]`
  matches CI's component set; `RUST_MIN_STACK` is env-scoped and orthogonal.
  Verification A2-b (`rustup default 1.85.0 && cargo --version`) genuinely
  exercises the override.
- **Edit targets verified against the real tree.** `ci.yml` has exactly one
  `dtolnay/rust-toolchain@stable` (line 33); `release.yml` has exactly five
  (lines 50, 127, 173, 223, 258 — the plan's cited lines are exact).
- **Scope discipline.** Non-goals exclude MSRV, `crates/**`, action SHA
  pinning (spec 06 Out of scope, medium 0006 §7), and spec 03 edits; the
  in-scope set (`.github/workflows/**` + `rust-toolchain.toml`) matches the
  amendment's A2 row.

## Findings

- [MAJOR] **Step 5's bump-PR workflow cannot deliver its stated enforcement,
  and its acceptance check (A2-c) is unexecutable pre-merge.** Two mechanical
  defects, both against documented GitHub Actions behaviour:
  (a) `peter-evans/create-pull-request` with the default `GITHUB_TOKEN`
  creates PRs whose `pull_request` event **does not trigger workflow runs**
  (GitHub: events caused by `GITHUB_TOKEN`, except
  `workflow_dispatch`/`repository_dispatch`, never create new runs; the
  action's own README documents this and recommends a PAT). The plan's PR
  body asserts "The full CI matrix must pass on this PR before landing" —
  as designed, no CI will ever run on a bump PR, so the "monthly cadence is
  enforced by CI" claim (the sole stated reason step 5 exists: "if the policy
  is unenforced, drift returns in a slower form") is false. Either the bump
  PR merges unverified, or (if checks are required by branch protection) it
  is unmergeable without manual intervention. The plan addresses neither.
  (b) A2-c requires "a manual `workflow_dispatch` invocation of
  `toolchain-bump.yml` on the slice branch" — but GitHub only registers
  `workflow_dispatch` for workflow files present on the **default branch**;
  a brand-new workflow file cannot be dispatched until it merges. The slice
  therefore cannot verify its own step-5 deliverable during its PR.
  Step 5 is declared severable ("step 5 can be deferred to a follow-up slice
  A3 … recommended-but-severable"), so this does not contaminate the
  mandatory floor (steps 1–4), but as written the plan ships a silently
  non-enforcing automation with an impossible verification. — slice-plan
  §Step 5, §Verification/A2-c.
- [MINOR] **Acceptance criterion 2's verification command fails when step 5
  is included.** `sed -n 's|.*rust-toolchain@\([0-9.]*\).*|\1|p'
  .github/workflows/*.yml | sort -u | wc -l` is asserted to return 1, but
  `toolchain-bump.yml` itself contains two literal `dtolnay/rust-toolchain@`
  lines (the "Compute current pin" grep and the rewrite `sed` with
  `@${OLD}`/`@${NEW}`); `[0-9.]*` matches the empty string there, so the
  pipeline yields a second (empty) distinct value and the count is 2.
  Exclude `toolchain-bump.yml` from the glob or require a non-empty version
  match (`[0-9.]\+` with an anchored pattern). — slice-plan §Acceptance
  criteria, item 2.
- [MINOR] **`rust-toolchain.toml` `targets = ["aarch64-apple-darwin"]` also
  applies on CI's Linux/Windows legs and on any non-Apple dev machine** —
  rustup auto-installs the listed target on first use, so every Windows /
  ubuntu CI leg and Intel/Linux developer downloads an Apple std they never
  use. Harmless (no failure) but contradicts the plan's own rationale for
  keeping the list minimal; the workflow's `targets: ${{ matrix.target }}`
  already covers CI. Consider dropping `targets` from the toml. — slice-plan
  §Step 4.
- [MINOR] **Step 1's evaluator-verification instruction is stale on
  arrival.** The plan asks the evaluator to "verify by re-reading the CI log
  linked from PR #5" and branches on live stable having drifted to `1.99.x+`.
  Live stable is exactly `1.98.0` (channel manifest, 2026-08-18), so the
  default applies — fine — but the plan would be more executable if step 1
  stated the outcome unconditionally now that the evaluator's re-check is
  settled: pin `<VERSION>` = `1.98.0`. — slice-plan §Step 1.

## Required changes (for FAIL)

1. Resolve the step-5 enforcement defect: either (a) sever step 5 to a
   follow-up slice A3 — the plan's own stated fallback — and reduce in-slice
   cadence enforcement to the inline comments in steps 2/4, updating A2-c and
   acceptance criterion 4 accordingly; or (b) keep step 5 and fix the design:
   document that `GITHUB_TOKEN`-created PRs do not trigger CI and specify the
   concrete workaround (a PAT/machine-account secret, or an explicit
   post-creation `gh workflow run` / close-reopen step), and rewrite A2-c so
   the dispatch verification happens post-merge on the default branch (or is
   explicitly deferred to the first scheduled fire with a recorded follow-up).
2. Fix acceptance criterion 2's sed/grep so it ignores (or correctly parses)
   the `dtolnay/rust-toolchain@${...}` literals inside `toolchain-bump.yml`
   when step 5 remains in scope.

## Notes

The mandatory floor of this plan — pin both workflow files to
`dtolnay/rust-toolchain@1.98.0`, add `rust-toolchain.toml`, document the
monthly cadence at the pin site — is correct, spec-faithful, mechanically
verified, and executable as written. The FAIL is confined to the optional
bump-PR automation (step 5) whose central promise (CI-gated monthly bump PRs)
is false under `GITHUB_TOKEN` and whose verification step cannot run
pre-merge; plus one mechanically wrong acceptance-check command. A revision
that severs step 5 per the plan's own escape hatch, or properly redesigns
it, should pass on re-review.
