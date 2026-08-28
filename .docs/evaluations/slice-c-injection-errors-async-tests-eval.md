# Evaluation: slice-c-injection-errors-async-tests

Verdict: FAIL
Round: 1
Reviewed against: `.docs/spec/06-remediation-highs.md` (§ 0002-H3, § 0002-H4,
Slice Breakdown row C, Invariants, Out of scope), `.docs/research/0002-core-
client-aws-review.md` (§4), and the post-slice-B tree: `crates/baeus-core/src/
{aws_sso.rs, aws_eks.rs, client.rs}`, `crates/baeus-core/Cargo.toml`,
`crates/baeus-ui/src/layout/app_shell.rs`, `.github/workflows/ci.yml`.

## Findings

- [BLOCKER] **Spec 06 0002-H4 acceptance 4a is unmet, and the plan's "Nine of
  nine" proof is mechanically false.** 4a requires "at least one
  `#[tokio::test]` exists for every function listed," and the list includes
  `create_eks_client`; the fix-approach minimum names a "`create_eks_client`
  round-trip (which now includes the refresher from 0002-H2)." The plan claims
  slice B's `eks_refresh_layer_refreshes_token_and_updates_authorization_header`
  (aws_eks.rs:1378) covers it. Verified against the tree: that test never calls
  `create_eks_client` — it constructs `EksTokenRefresher::new(...)` and layers
  `AuthRefreshLayer` manually (its own comment: "Construct an EksTokenRefresher
  directly"), precisely because `AuthRefreshLayer` is private. No test anywhere
  in the tree invokes `create_eks_client`. The plan explicitly declines to add
  one ("Slice C does not re-cover it") and step 3 adds no seam for it. The
  "minimum, not a duplication requirement" reading fails because no prior
  coverage of the function itself exists to avoid duplicating. Landing this
  plan leaves a numbered spec acceptance criterion unsatisfied while declaring
  it satisfied.
- [MAJOR] **Spec 06 0002-H3 test expectation (a) is unmet for
  `inject_aws_credentials_into_kubeconfig`.** The spec requires new unit tests
  covering "both inject functions with (a) a valid exec-block kubeconfig, (b) a
  kubeconfig where the target context has no exec block." The plan adds (b) for
  both functions but relies on retained green-path tests for (a) — and the only
  existing green-path tests (aws_sso.rs:224, :273) exercise
  `inject_aws_profile_into_kubeconfig` exclusively. Verified: the aws_sso.rs
  test module contains exactly six tests, none calling
  `inject_aws_credentials_into_kubeconfig`. Step 1 rewrites that function's
  control flow (double `if let Some` → `match`, including the session-token
  insertion path at aws_sso.rs:155-169) with zero mutation-asserting tests; a
  regression in env-var insertion would compile and pass the planned gate.
  Acceptance 3b's stated proof ("retained green-path tests") does not cover
  this function.
- [MAJOR] **Blanket wiremock fallback is not sanctioned by the spec's actual
  wording.** Spec 06 0002-H4: "prefer AWS smithy mock; fall back to `wiremock`
  only if the smithy mock cannot cover a given call" — a per-call,
  coverage-based condition. The plan takes wiremock for the entire smoke file
  and justifies it with dep-tree churn / deny-gate surface / slice-B precedent —
  none of which is the spec's condition, and the churn rationale is undercut by
  the spec's own budgeting ("`[dev-dependencies]` addition for the chosen
  mock"). The plan's assertion that "Spec 06 explicitly authorises wiremock as
  fallback for the observation-surface it names" overstates a conditional
  per-call fallback as blanket authorization. Every H4 acceptance criterion is
  mock-agnostic, so this is a means-level spec-fidelity defect, not an
  ends-level one — hence MAJOR, not BLOCKER — but the frozen spec's directive
  cannot be re-read by the slice-plan; it must be satisfied, scoped, or
  amended.
- [MINOR] **4c restatement drops the spec's "cargo nextest" wording.** CI
  actually runs `cargo nextest run --workspace` (ci.yml:59-60, :86); the plan's
  gate table and 4c say `cargo test --workspace`. Substance unaffected
  (wiremock `MockServer` tests run identically under nextest's per-process
  model), but the acceptance restatement should track the spec's wording and
  note nextest compatibility explicitly.
- [MINOR] **4b would be more faithfully proven by a pending→success sequence
  in one test.** `sso_poll_for_token_returns_pending_on_authorization_pending`
  serves a single 400 and asserts `SsoTokenResult::Pending`; the retry loop
  itself lives in the wizard caller. A two-response wiremock sequence (400
  `authorization_pending`, then 200 success, `expect(2)`) within one test would
  directly prove "retry-on-pending behaviour" as spec 06 0002-H4's acceptance
  words it. Defensible as-is (the function's retry contract is to return
  `Pending` — verified at aws_eks.rs:316-337), hence MINOR.

## Required changes (for FAIL)

1. Add a `create_eks_client` round-trip `#[tokio::test]` (smoke file or
   inline) that actually invokes `create_eks_client` — feasible without a new
   seam: the function takes `&EksCluster` + `&Credentials` (aws_eks.rs:965-968)
   and its only AWS interaction is the client-side presign via
   `generate_eks_token`, so an `EksCluster` whose endpoint is a wiremock
   `MockServer` URI plus stub credentials suffices; assert the Bearer header
   and (to satisfy "which now includes the refresher") a refresh past the
   10-second leeway, mirroring slice B's wiremock pattern. Correct the "Nine of
   nine" claim in 4a and adjust the step-4 test count (+1) and the +14 delta.
2. Add the valid-exec-block green-path test for
   `inject_aws_credentials_into_kubeconfig` (assert `AWS_ACCESS_KEY_ID` /
   `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN` env insertion) per spec 06
   0002-H3 test expectation (a) for "both inject functions"; correct the 3b
   proof text; adjust the delta (+1).
3. Reconcile the mock choice with spec 06 0002-H4's conditional: either
   demonstrate per-call that the smithy mock cannot cover specific operations
   and scope the wiremock fallback to those calls, or route the blanket
   wiremock choice through the spec-amendment path. Do not assert the spec
   "explicitly authorises" a blanket fallback — it does not.
4. (MINOR) Restate 4c against the real `cargo nextest run --workspace` CI gate
   (ci.yml:86).
5. (MINOR) Consider folding pending→success into a single two-response polling
   test to prove 4b's "retry-on-pending" wording directly.

## Notes

Mechanical verification performed against the post-slice-B tree; everything
not flagged above checked out exactly:

- All nine 0002-H4 function line numbers in the plan match: aws_eks.rs :234,
  :257, :293, :343, :416, :471, :508, :691, :965; `sso_list_account_roles` at
  :378; `discover_clusters_in_region` at :617.
- All nine inline `aws_config::defaults(...)` sites match exactly: :235, :263,
  :299, :344, :383, :422, :480, :512, :621.
- `create_eks_client` returns `Result<(kube::Client, EksTokenRefresher)>`
  (:968); `EksTokenRefresher` at :814, `AuthRefreshLayer` at :888; exactly
  three `#[tokio::test]` in the file (:1321, :1349, :1377), all slice-B — the
  plan's "no wizard-entry-point async tests" claim is accurate.
- `inject_aws_profile_into_kubeconfig` (aws_sso.rs:53) and
  `inject_aws_credentials_into_kubeconfig` (:106) both fall through a double
  `if let Some` (:73-97, :127-171) to terminal `Ok(())` (:100, :173) — the
  silent-path description is exact, and the `KubeconfigInjectionError`
  variants cover every path the functions can fail on (context lookup,
  auth-info lookup, missing `auth_info`/`exec`).
- The three existing aws_sso.rs tests are named exactly as the plan states;
  `test_inject_missing_context_returns_error` asserts only `.is_err()`, so the
  retitle + typed-variant assertion is compatible.
- client.rs call sites verified: inject calls at :168 and :202 inside
  `create_client_from_path` (:159) and `create_client_from_path_with_aws_creds`
  (:192); both functions already return `anyhow::Result`, so the thiserror →
  anyhow `?`-coercion claim is sound. No `error.rs` exists in baeus-core, so
  defining the enum in aws_sso.rs follows the spec's fallback.
- UI "no edit needed" claim verified: the wizard connect
  (app_shell.rs:~1704-1712) maps errors via `format!("{e:#}")` and stores into
  `connection_errors` (:1750, :1764); the `connection_errors` grep reproduces
  the plan's 18 line numbers exactly; `enrich_eks_error` (:1783) passes
  messages without auth keywords through unchanged (the ExecBlockMissing
  message contains none of "Unauthorized"/"401"/"forbidden"/"403"/
  "ExpiredToken"/"InvalidClientTokenId"); the details-panel render (:7355)
  emits `SharedString::from(error_msg.clone())` — full string, no truncation.
- `SsoDeviceAuth` fields (:89-96) match the planned assertions
  (`poll_interval`, `expires_at`, `verification_uri_complete: Option<_>`).
- `SsoTokenResult::Pending` mapping via `is_authorization_pending_exception()`
  verified (:325-331), so the wiremock 400-body test approach is sound.
- `wiremock = "0.6"` dev-dep confirmed at Cargo.toml:49; step 0's "no new
  dependencies" is accurate for the plan as written.
- Non-goal discipline matches spec 06's Out-of-scope list (mediums deferred,
  no SecretString migration, no CLI shell-out replacement, no UI redesign);
  scope otherwise exact against Slice C's row.
- The `pub` + `#[doc(hidden)]` seam visibility reasoning is correct
  (integration tests under `tests/` cannot see `pub(crate)`), and the plan's
  justification against the spec's named file target is sound — the rubric
  question about test-only API pollution is adequately answered by the spec's
  own prescription of `crates/baeus-core/tests/aws_wizard_smoke.rs`.
- Test-count arithmetic (+14) is internally consistent for the plan as
  written; required changes 1-2 move it to +16.
