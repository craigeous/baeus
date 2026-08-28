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


---

# Evaluation: slice-c-injection-errors-async-tests (re-review, round 2)

Verdict: FAIL
Round: 2
Reviewed against: `.docs/spec/06-remediation-highs.md` (§ 0002-H3, § 0002-H4,
Slice Breakdown row C), `.docs/research/0002-core-client-aws-review.md` (§4),
the round-1 evaluation (`.docs/evaluations/slice-c-injection-errors-async-
tests-eval.md`), the `git diff 278c736..fd8f0f5` of the artifact, and the
unchanged post-slice-B tree: `crates/baeus-core/src/{aws_eks.rs, aws_sso.rs,
client.rs}`, `Cargo.lock`, vendored `aws-smithy-runtime-1.10.3` /
`aws-smithy-http-client-1.1.12` / `kube-client-0.98.0` sources,
`.github/workflows/ci.yml`. No code has changed since the round-0
verification (`git log a302847..HEAD` on the reviewed crates is empty);
all round-0 verified-exact claims were spot-rechecked and still hold
(aws_eks.rs :234/:257/:293/:343/:416/:471/:508/:617/:691/:965, :806, :858,
:981; aws_sso.rs :53/:106/:155-169; client.rs :168/:202; ci.yml:86 =
`cargo nextest run --workspace`).

## Round-1 findings — resolution status

- [BLOCKER, round 1] *No test invokes `create_eks_client`.* **Substantively
  resolved, but the replacement test is partially infeasible — see new
  BLOCKER below.** The revision adds
  `create_eks_client_returns_bearer_token_and_refreshes_past_leeway`
  (step 6) that literally calls
  `create_eks_client(&cluster, &credentials).await?` — signature-compatible
  with the real `pub async fn create_eks_client(cluster: &EksCluster,
  credentials: &Credentials) -> Result<(kube::Client, EksTokenRefresher)>`
  (aws_eks.rs:965-968). The 4a "Nine of nine" accounting is now honest
  (7 smoke + 2 inline), and the Bearer-header half is sound: the
  `AuthRefreshService` inserts `authorization: Bearer <token>`
  (:938-947), `should_refresh()` is false on the first request (60s
  initial TTL), so wiremock observes the initial `k8s-aws-v1.…` token.
  The TLS/HTTP analysis is also sound — kube-client 0.98.0's
  `rustls_https_connector()` builds with `enforce_http(false)` +
  `https_or_http()` (vendored config_ext.rs:222/:238), so an `http://`
  wiremock endpoint passes through, and the fallback
  `create_eks_client_with_http_client` seam covers residual risk.
- [MAJOR, round 1] *Missing green-path test for
  `inject_aws_credentials_into_kubeconfig`.* **Resolved (primary
  assertions).** Step 1's new
  `test_inject_credentials_sets_aws_env_vars_on_valid_context` asserts
  `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`
  insertion — matching the real code paths at aws_sso.rs:129-153 and
  :155-168 — and the 3b proof text was corrected. Spec 06 0002-H3 test
  expectation (a) is now met for both inject functions. A residual
  flaw in the test's second half is flagged as a new MINOR below.
- [MAJOR, round 1] *Blanket wiremock fallback exceeds the spec's per-call
  conditional.* **Resolved.** The revision adopts
  `aws-smithy-runtime`'s `test-util` feature (`StaticReplayClient`) as
  primary for every SDK-touching test and retains wiremock for exactly
  one non-SDK call surface — the K8s API server in step 6's
  `create_eks_client` test (kube-rs speaks `hyper_util` HTTP, which no
  smithy mock can intercept). That is precisely the per-call,
  coverage-based fallback spec 06 0002-H4 permits ("fall back to
  `wiremock` only if the smithy mock cannot cover a given call"). The
  `StaticReplayClient` claim is REAL, with one versioning caveat (new
  MINOR below): the type exists in the locked tree — defined at
  `aws_smithy_http_client::test_util::StaticReplayClient`
  (aws-smithy-http-client 1.1.12, vendored
  `src/test_util/replay.rs:154`) and re-exported at the plan's stated
  path `aws_smithy_runtime::client::http::test_util::StaticReplayClient`
  (aws-smithy-runtime 1.10.3, vendored `src/client/http.rs:26`).
  `Cargo.lock` pins aws-smithy-runtime 1.10.3 as a transitive dep of the
  aws-sdk-* crates. The `SdkConfig.http_client(...)` injection pattern
  is the documented one, and all eight `_with_config` seams (which take
  `&aws_config::SdkConfig` and pass it to `aws_sdk_*::Client::new`)
  plausibly route through a replay client.
- [MINOR, round 1] *4c nextest wording.* **Resolved.** Verification §3,
  the gate table, and 4c all now say
  `RUST_MIN_STACK=268435456 cargo nextest run --workspace` (ci.yml:86)
  with a note on process-model compatibility.
- [MINOR, round 1] *Fold pending→success into one test.* **Resolved.**
  `sso_poll_for_token_returns_pending_then_success` replays a 400
  `authorization_pending` then a 200 success against one client and
  asserts `Pending` then `Success`; StaticReplayClient pops replay
  events in request order, so the two-step sequence is deterministic.
  4b's "retry-on-pending" wording is now proven directly.

## New findings

- [BLOCKER] **Step 6's refresher-regeneration assertion is mechanically
  impossible as sketched, and the plan's 4a proof text relies on it.**
  The test prescribes: "call `refresher.refresh().await?` and assert
  `refresher.current_token()` is a distinct `k8s-aws-v1.…` value from
  the initial (proves the refresh closure captured by `create_eks_client`
  regenerates a fresh presigned token)". Verified against the tree:
  `EksTokenRefresher::refresh()` (aws_eks.rs:858-863) acquires the
  mutex, re-checks `should_refresh()`, and **returns `Ok(())` without
  invoking the refresh closure when the check is false**.
  `should_refresh()` (:834-837) compares `now + 10s`
  (`REFRESH_LEEWAY_SECS = 10`, :806) against `expires_at`, and
  `create_eks_client` hardcodes the initial state to
  `expires_at = now + 60s` (:981) with no parameter, env hook, or seam
  to override it. Therefore `refresh()` called immediately after
  construction — as sketched — is a no-op and `current_token()` equals
  the initial token; the "distinct value" assertion fails. Slice B's
  own test (:1400-1418, :1457-1458) engineered a 12s TTL plus a 3s
  sleep precisely because of this gating; through `create_eks_client`
  no such engineering is available — proving "regenerating past the
  10-second leeway" requires either a ~51-second wall-clock sleep (not
  disclosed anywhere in the plan; step 4 even claims "No wall-clock
  sleeps are needed", and spec 06 0002-H4's test expectations require
  tests to "run within CI's existing runtime budget" across three CI
  legs) or a token-state seam the plan does not prescribe (its only
  fallback seam, `create_eks_client_with_http_client`, bypasses the
  TLS connector, not the token state). The 4a proof text explicitly
  states the test "asserts on both the Bearer header … and the
  refresher regenerating past the 10-second leeway", and the test name
  encodes the same claim — so the plan's accounting of spec 06
  0002-H4 acceptance 4a again rests on an assertion that cannot exist
  as written. This is the same defect class as round 1's BLOCKER (a
  coverage claim falsified by the code), one level down, in the very
  step added to resolve it. The direct-invocation + Bearer-header half
  is sound and would satisfy 4a's letter on its own; the refresher
  half must be re-planned (see Required change 1), not improvised.
- [MINOR] **The new green-path test's second half contradicts the
  unchanged insertion semantics it claims to regression-cover.** Step 1
  prescribes calling `inject_aws_credentials_into_kubeconfig` "a second
  time with `session_token = None` and assert the env vec no longer
  contains `AWS_SESSION_TOKEN`". The code path (aws_sso.rs:155:
  `if let Some(token) = session_token`) only inserts/updates when
  `Some`; it never removes an existing entry, and step 1 explicitly
  leaves "the mutating body (env-var insertion) unchanged". On the
  same kubeconfig the assertion fails (the token from the first call
  remains); on a fresh kubeconfig the assertion is trivially true but
  "no longer contains" is the wrong description. One-line fix at
  implementation, but the instruction as written fails if followed
  literally.
- [MINOR] **Step 0's prescribed dev-dep path is `#[deprecated]` in the
  locked tree and trips the `-D warnings` gate.** The plan names
  `aws_smithy_runtime::client::http::test_util::StaticReplayClient`.
  In the locked aws-smithy-runtime 1.10.3 that module carries
  `#[deprecated = "… Please use the `test-util` feature from
  `aws-smithy-http-client` instead"]` (vendored
  `src/client/http.rs:12-14`) over its re-export; referencing the
  deprecated module under
  `cargo clippy --workspace --all-targets -- -D warnings` (which
  compiles test targets) errors. The correct formulation for the
  locked tree is dev-dep
  `aws-smithy-http-client = { version = "1.1", features = ["test-util"] }`
  with `use aws_smithy_http_client::test_util::StaticReplayClient`
  (aws-smithy-http-client 1.1.12 is already in Cargo.lock). Bounded —
  same crate family, spec 06's "[dev-dependencies] addition for the
  chosen mock" covers either — hence MINOR.
- [MINOR] **Step 0's "does not add a new crate to the tree" claim is
  false.** aws-smithy-runtime 1.10.3's `test-util` feature =
  `["aws-smithy-runtime-api/test-util", "dep:tracing-subscriber",
  "aws-smithy-http-client/test-util", "legacy-test-util"]`, and
  `aws-smithy-http-client/test-util` pulls `dep:aws-smithy-protocol-test`
  — which is absent from Cargo.lock (grep count 0). `tracing-subscriber`,
  `http` 0.2.12, and `hyper` 0.14.32 are already locked, so the delta is
  small and licence-compatible, and the plan does hedge with a
  post-add `cargo tree` / `cargo deny check` verification — but the
  flat assertion (repeated in gate step 4: "does not add a new crate,
  so the licence/advisory surface is unchanged") is inaccurate as
  written.
- [MINOR] **"All nine SDK operations" misstates the smoke file's
  coverage.** The In-scope section claims the step-4 per-call review
  "shows StaticReplayClient covers all nine SDK operations"; the smoke
  file touches eight operations (the ninth and tenth 4a functions —
  `create_eks_client`, `generate_eks_token` — live in step 6, one
  wiremock-backed, one mock-free). Test-count arithmetic itself is
  consistent throughout (+15 = 5 + 8 + 2, verified against step 1's
  five named tests, step 4's eight, step 6's two, the gate table, and
  4a's 7+2 accounting).

## Required changes (for FAIL)

1. Re-plan step 6's refresher half. Either (a) weaken the assertion to
   what the real API supports — e.g. assert `refresher.current_token()`
   equals the initial token and `refresher.should_refresh()` is false
   immediately after construction (proving `create_eks_client` wired
   and returned the live refresher that the `AuthRefreshLayer`
   observes), and correct the 4a proof text and the
   `…_refreshes_past_leeway` test name to match; or (b) disclose a
   ≥51-second sleep with an explicit CI-runtime-budget justification
   (spec 06 0002-H4 test expectations); or (c) prescribe a minimal
   initial-token-state seam. As written, `refresh()` early-returns
   (aws_eks.rs:858-863) because `should_refresh()` stays false for
   ~50 s after construction (:806, :981), so the "distinct token"
   assertion cannot pass.
2. Fix step 1's second-call session-token assertion: use a fresh
   kubeconfig for the `None` call (assert "does not contain"), or
   reword; the current "no longer contains" contradicts the
   insert-only semantics at aws_sso.rs:155-169 that the plan says are
   unchanged.
3. Correct step 0/4's mock dev-dep: prescribe
   `aws-smithy-http-client` with `test-util` and the
   `aws_smithy_http_client::test_util::StaticReplayClient` path (or add
   `#[allow(deprecated)]` with justification), and replace the "does
   not add a new crate" claim with the accurate delta
   (`aws-smithy-protocol-test` is net-new; gate-verified post-add).
4. (MINOR) Reword "all nine SDK operations" to eight in the step-4
   per-call review.

## Notes

Mechanical verification performed for this round, beyond the items
flagged above:

- `create_eks_client` body (:965-1054) read in full: presign via
  `generate_eks_token` is the only AWS interaction (client-side signer,
  verified network-free in round 0); an `EksCluster` with a wiremock
  URI endpoint plus stub credentials exercises the full body as the
  plan claims. `kube::Config::from_custom_kubeconfig` (:1035) and
  `rustls_https_connector()` (:1041-1043) are build-time only for an
  `http://` endpoint; kube-client 0.98.0 builds its connector with
  `enforce_http(false)` + `https_or_http()` (config_ext.rs:222/:238),
  so the primary (no-new-seam) path is plausible and the plan's
  last-resort `pub(crate)` TLS seam is an acceptable contingency.
- The returned `refresher` and the layer's `clone_handle()` share
  `Arc` state (:1049, :870-879), so Bearer-header observation via
  wiremock does exercise the same token the returned refresher holds.
- Spec 06 0002-H4's acceptance list and fix-approach wording re-read
  verbatim (:252-286): the nine-function list, the per-call mock
  conditional, the "[dev-dependencies] addition for the chosen mock"
  budget, the `cargo nextest` gate wording, and the "`create_eks_client`
  round-trip (which now includes the refresher from 0002-H2)" phrase
  all match the plan's citations.
- Spec 06 0002-H3's test expectations (:247-250: "(a) a valid
  exec-block kubeconfig, (b) … no exec block … for both inject
  functions") are now met by step 1's five tests plus the two retained
  profile green-path tests (aws_sso.rs:224, :273 confirmed extant and
  profile-only).
- Wiremock's remaining scope (step 6 K8s API server + slice B's
  retained `eks_refresh_layer_…` test) is the spec's per-call fallback
  exactly; the revision no longer asserts blanket authorization.
- Non-goal discipline, `pub` + `#[doc(hidden)]` seam reasoning,
  UI no-edit verification (step 5), and scope fidelity to Slice C's
  row are unchanged from round 0 and still correct.
