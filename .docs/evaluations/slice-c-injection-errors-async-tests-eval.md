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


---

# Evaluation: slice-c-injection-errors-async-tests (re-review, round 3)

Verdict: PASS
Round: 2
Reviewed against: `.docs/spec/06-remediation-highs.md` (§ 0002-H3, §
0002-H4, Slice Breakdown row C), the round-2 evaluation section of
`.docs/evaluations/slice-c-injection-errors-async-tests-eval.md`, the
`git diff fd8f0f5..ac50b19` of the artifact, and the unchanged
post-slice-B tree: `crates/baeus-core/src/{aws_eks.rs, aws_sso.rs}`,
`Cargo.lock`, `.github/workflows/ci.yml`, and the vendored
`aws-smithy-http-client-1.1.12`, `aws-smithy-runtime-1.10.3`,
`kube-client-0.98.0` sources. No reviewed code has changed since round 2
(aws_eks.rs :806/:834-837/:858-863/:965-1054 and slice B's test at
:1377-1478 spot-rechecked verbatim).

## Round-2 findings — resolution status

- [BLOCKER, round 2] *Step 6 refresher assertion mechanically
  impossible (refresh() no-ops below the 10s leeway; 60s hardcoded
  initial TTL).* **Resolved — re-walked the new timeline against the
  real code; it is sound.** The revision prescribes a
  `pub #[doc(hidden)] create_eks_client_with_initial_ttl_secs(cluster,
  credentials, initial_ttl_secs: i64)` seam: the current body
  (aws_eks.rs:965-1054, verified) moves into the inner with the
  hardcoded `now + 60s` at :981 replaced by
  `Duration::seconds(initial_ttl_secs)` (i64 — signature-compatible),
  and public `create_eks_client` becomes a one-line delegate at 60s.
  Verified the gating arithmetic directly: `should_refresh()`
  (:834-837) computes `Utc::now() + 10s >= expires_at` with `>=`, and
  `REFRESH_LEEWAY_SECS = 10` (:806). With the seeded 12s TTL: at
  `t ≈ t0`, `t0+10s < t0+12s` → fast path, no refresh (the early return
  at :861-863 under the `tokio::sync::Mutex` is not reached via
  `AuthRefreshService`); after the disclosed 3s wall-clock sleep,
  `t0+13s >= t0+12s` (13 ≥ 12) → the second `list()` drives
  `AuthRefreshService::call()` into `refresher.refresh()`, the mutex
  re-check passes, and the captured closure (:988-1001) re-invokes
  `generate_eks_token`, yielding a presigned URL distinct at least in
  `X-Amz-Date`/`X-Amz-Signature` 3s later. This mirrors slice B's
  proven pattern exactly (12s seed at :1415-1416, 3s sleep at :1458,
  `.expect(2)` mock at :1386-1396), which passes all three CI legs
  today. The closure's own post-refresh `expires_at = now + 60s` (:998)
  is intentionally unchanged — after the single refresh
  `should_refresh()` goes false again, so exactly two requests hit the
  mock and `.expect(2)` guards phantoms. The returned refresher and the
  layer's `clone_handle()` share `Arc` state (:1049, :870-879), so the
  `current_token()` == bearer-tail assertions hold on both requests.
  The 4a proof text now describes this actual mechanism (seeded TTL +
  sleep + two observed bearers) rather than round 2's impossible
  immediate-`refresh()` call, and the "Why the seam is required"
  paragraph's mechanical argument (:981 hardcode vs :806 leeway vs
  :861-863 early return) checks out line-for-line. The 51s-sleep
  alternative is correctly rejected against spec 06 0002-H4's
  CI-runtime-budget test expectation; the seam is the minimum honest
  fix and is the same shape (pub + `#[doc(hidden)]` inner, unchanged
  public wrapper) spec 06's "minimal API surface tweaks" clause
  authorises. One fixture-level flaw in this test's construction is
  flagged as a new MINOR below; it does not touch the seam, timeline,
  or assertions.
- [MINOR, round 2] *Green-path test's second-call "no longer contains"
  contradicts insert-only semantics.* **Resolved.** Step 1 now
  prescribes a second, fresh kubeconfig for the `session_token = None`
  call with a "does not contain" assertion, and documents why
  (aws_sso.rs:155-169 is insert-only — re-verified: the
  `if let Some(token) = session_token` block only inserts/updates, never
  removes).
- [MINOR, round 2] *Deprecated StaticReplayClient path trips
  `-D warnings`.* **Resolved.** Step 0 now dev-deps
  `aws-smithy-http-client = { version = "1.1", features = ["test-util"] }`
  and step 4 imports `aws_smithy_http_client::test_util::StaticReplayClient`.
  Verified against the vendored tree: the type is defined non-deprecated
  at aws-smithy-http-client 1.1.12 `src/test_util/replay.rs:154`
  (`pub struct StaticReplayClient`), while the
  `aws_smithy_runtime::client::http::test_util` module carries
  `#[deprecated = "… Please use the `test-util` feature from
  `aws-smithy-http-client` instead"]` at aws-smithy-runtime 1.10.3
  `src/client/http.rs:12` exactly as the plan claims. Both crates are in
  `Cargo.lock` at the stated versions.
- [MINOR, round 2] *"does not add a new crate" claim false.*
  **Resolved.** Step 0's "Dep-tree delta (accurate accounting)"
  discloses `aws-smithy-protocol-test` as net-new to the lockfile
  (verified: `grep -c aws-smithy-protocol-test Cargo.lock` = 0 pre-slice;
  the crate is `dep:aws-smithy-protocol-test` under
  aws-smithy-http-client's `test-util` feature), requires a post-add
  `cargo tree` / `cargo deny check` with the diff captured in the commit,
  and gates licence/advisory failure to Needs Clarification. A residual
  inaccuracy in the parenthetical naming is flagged below.
- [MINOR, round 2] *Nine-vs-eight misstatement.* **Resolved.** Both the
  In-scope section and step 4's per-call review now say "all eight SDK
  operations exercised by the smoke file," with an explicit parenthetical
  that the ninth/tenth 0002-H4 functions (`create_eks_client`,
  `generate_eks_token`) live in step 6.

## New findings

- [MINOR] **Step 6's CA fixture must be base64-encoded PEM, not the raw
  PEM const the text describes.** The test body prescribes an
  `EksCluster` whose `certificate_authority_data` is "a valid
  self-signed CA PEM (embedded as a `const &str` … a well-formed
  `-----BEGIN CERTIFICATE-----` payload that rustls-pemfile parses)".
  Verified against the locked kube-client 0.98.0:
  `from_custom_kubeconfig` → `new_from_loader` → `loader.ca_bundle()?`
  (file_loader.rs:113-121) → `load_certificate_authority()` →
  `load_from_base64_or_file` → `base64::engine::general_purpose::
  STANDARD.decode` (file_config.rs:641-646) — unconditionally, with no
  raw-PEM fallback — and only then `certs()` PEM-parses the decoded
  bytes (config/mod.rs:377-388). A raw `-----BEGIN CERTIFICATE-----`
  string fails base64 decode (`-` is outside the alphabet), so
  `create_eks_client_with_initial_ttl_secs(...).await?` would return
  `Err` at construction and the test would fail on its first statement
  if the text is followed literally. The fix is one line — base64-encode
  the PEM, matching EKS's real wire format (`certificate.authority.data`
  is base64, passed through unchanged at aws_eks.rs:660-663) and the
  repo's own fixture convention (`"LS0tLS1..."` at aws_eks.rs:1150).
  Same defect class round 2 rated MINOR (instruction-as-written fails;
  trivial local correction; no change to seam, timeline, assertions,
  counts, or gate).
- [MINOR] **Step 0's "other transitively-pulled crates" parenthetical
  names the wrong crates for the crate it prescribes.** The text says
  the other pulls under the feature are "`tracing-subscriber`, `http`
  0.2, `hyper` 0.14" — that is the closure of *aws-smithy-runtime*'s
  `test-util` feature (vendored Cargo.toml:72-77, which does pull
  `dep:tracing-subscriber`). The prescribed
  `aws-smithy-http-client/test-util` (vendored Cargo.toml:113-123)
  actually pulls `dep:serde`, `dep:serde_json`, `dep:indexmap`,
  `dep:bytes`, `dep:http-1x`, `dep:http-body-1x`,
  `aws-smithy-runtime-api/http-1x`, `aws-smithy-types/http-body-1-x`,
  `tokio/rt` — all already locked (indexmap, http 1.x, http-body 1.x,
  serde/serde_json/bytes confirmed in `Cargo.lock`). The load-bearing
  claim — only `aws-smithy-protocol-test` is net-new, everything else
  already locked, `cargo deny check` gates the delta — remains true;
  only the illustrative parenthetical is stale (carried over from the
  round-2 eval's description of the previous crate choice).

## Required changes (for FAIL)

None — no BLOCKERs and no MAJORs. The two MINORs above are one-line
corrections at implementation (base64-encode the CA const; fix the
parenthetical's crate names) and do not require re-planning.

## Notes

Fresh-attack pass performed for this round, beyond the flagged items:

- **Seam-composition check (step 3 `_with_config` vs step 6
  `_with_initial_ttl_secs`).** No overlap or conflict: step 3's eight
  seams cover exactly the functions that build `aws_config::defaults`
  inline (:235/:263/:299/:344/:383/:422/:480/:512/:621 — re-spot-checked);
  `create_eks_client` builds no `SdkConfig` (its only AWS interaction is
  the client-side presign via `generate_eks_token`), so it is correctly
  absent from step 3's list, and the TTL seam is a disjoint surface.
  The Notes section's two-seam discussion (pub + `#[doc(hidden)]`
  uniformity, inline-test placement making `pub(crate)` technically
  sufficient for step 6) is coherent, and the last-resort TLS-bypass
  seam's composition is stated unambiguously (the http-client seam takes
  both an initial TTL and a pre-built client; the public wrapper passes
  60s + the default TLS-backed client).
- **Timeline re-walk (independent).** t0 defined at constructor return
  while `expires_at` is set milliseconds earlier inside the constructor
  only widens the refresh-path margin (t0+13+δ ≥ t0+12) and leaves ~2s
  of slack on the fast path (t0+10+δ < t0+12 for any sub-second δ) —
  identical margins to slice B's in-tree test that passes CI today.
  The `>=` comparison in `should_refresh()` matches the plan's
  "13 ≥ 12" presentation. No new arithmetic errors introduced.
- **HTTP-mock feasibility re-verified.** kube-client 0.98.0's
  `rustls_https_connector()` builds `HttpConnector` with
  `enforce_http(false)` (config_ext.rs:222) and
  `https_or_http()` (config_ext.rs:238), so an `http://` wiremock URI
  passes through the assembled service without TLS — the primary
  no-TLS-seam path is plausible and the fallback seam is an acceptable
  contingency. Slice B's analogous pattern (aws_eks.rs:1421-1435) uses
  the same `enforce_http(false)` mechanism.
- **Test-count consistency.** +15 = 5 (step 1) + 8 (step 4) + 2
  (step 6), consistent across Verification §3, the gate table, step 4's
  own count note ("8 tests, one fewer than the pre-fold 9"), and 4a's
  7-of-nine-smoke + 2-inline accounting (the smoke file's 8th test
  covers `discover_clusters_in_region`, outside the nine — correctly
  excluded from the 4a count but included in the +15).
- **Spec 06 0002-H4 re-read verbatim.** The fix-approach minimum
  ("`create_eks_client` round-trip (which now includes the refresher
  from 0002-H2)"), the nine-function list, the per-call mock
  conditional, the `[dev-dependencies]` budget, and the nextest gate
  wording are all as the plan cites. The TTL seam stays within the
  "minimal API surface tweaks" shape (one scalar parameter, unchanged
  public wrapper, `#[doc(hidden)]`), and is the resolution path the
  round-2 evaluation's Required change 1(c) explicitly offered; for
  `create_eks_client` a `SdkConfig` parameter would be meaningless (no
  SDK client is built), so the spec's parenthetical does not apply to
  this function.
- **Trivial citation drift (not a finding).** Step 0 cites "ci.yml:75"
  for the clippy line; clippy is at ci.yml:81 (:75-76 is the
  `cargo deny check` step). The nextest citation (ci.yml:86) is exact.
- **rustls availability for the CA fallback.** `rustls.workspace = true`
  is a direct dependency (crates/baeus-core/Cargo.toml:30), so the "CA
  data note" fallback's "already a workspace dep" claim is accurate
  (whether rustls itself generates certs is beside the point — the
  primary const path, once base64-encoded, suffices).


---

# Code evaluation (round 3)

# Evaluation: slice-c-injection-errors-async-tests (code review)

Verdict: FAIL
Round: 3
Reviewed against: `.docs/slice-plans/slice-c-injection-errors-async-tests.md`
(Implemented), `.docs/spec/06-remediation-highs.md` (§ 0002-H3, § 0002-H4),
`.docs/research/0002-core-client-aws-review.md` (§4), the prior plan-eval rounds
in `.docs/evaluations/slice-c-injection-errors-async-tests-eval.md`, the
review-findings artifact `.docs/evaluations/slice-c-injection-errors-review-
findings.md`, the commit diff `git diff 246434b..HEAD` on branch
`slice/c-injection-errors` (HEAD = 20b5857), the vendored
aws-smithy-http-client-1.1.12 sources, and a fresh gate re-run on the pinned
Rust 1.98.0 toolchain (rust-toolchain.toml, matching CI's
dtolnay/rust-toolchain@1.98.0).

## Gate re-run (verified, not trusted)

| Step | Command | Result |
|------|---------|--------|
| format | `cargo fmt --all -- --check` (and plain `cargo fmt --check`) | **FAIL, exit 1** — one diff: `crates/baeus-core/tests/aws_wizard_smoke.rs:334-335` |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 (0 errors; only cargo-level future-incompat notes for transitive deps) |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` | PASS, exit 0 — **3706 passed, 0 failed** across 77 suites; all 15 new tests individually confirmed present and passing |
| deny | `cargo deny check` | PASS, exit 0 — advisories ok, bans ok, licenses ok, sources ok |

The 15 new tests were each grepped out of the test log (`... ok`): 5 in
aws_sso.rs, 8 in tests/aws_wizard_smoke.rs, 2 inline in aws_eks.rs. Net +15
confirmed by inspection: no test deletions (the three `is_aws_sso_auth_error`
tests and the two profile green-path tests were moved below the new tests, not
removed; the retitled `test_inject_missing_context_returns_context_not_found`
is a rename with a strengthened typed-variant assertion).

## Findings

- [BLOCKER] **The format gate is red at HEAD; the recorded "gate green
  (Implemented)" evidence does not match reality.** `cargo fmt --all --
  --check` (and the project gate's plain `cargo fmt --check`) exits 1 on the
  pinned 1.98.0 toolchain — the same toolchain CI runs. The single violation
  is in the slice's own new file: `crates/baeus-core/tests/aws_wizard_smoke.rs`
  lines 334-335 split
  `let list_body = r#"{"clusters":["cluster-1","cluster-2"],"nextToken":null}"#.to_string();`
  across two lines; rustfmt requires it joined (89 chars < max_width=100).
  Rubric: "A red gate is an automatic BLOCKER"; severity.md: "the gate is red"
  is a BLOCKER. CI's format leg would fail identically. One-line mechanical
  fix (`cargo fmt`), but the gate must actually be green before landing.
- [MINOR] **The eight StaticReplayClient smoke tests never call
  `assert_requests_match`, so the `stub_request` URIs are decorative.**
  Verified in the vendored aws-smithy-http-client 1.1.12
  (`src/test_util/replay.rs:215-235`): request/expected matching runs only
  inside `assert_requests_match`; during a call the client records the actual
  request and pops the next queued response unconditionally. Consequently the
  tests prove the response path end-to-end (real SDK operation builders,
  response deserialization, error mapping — the 400 `__type:
  AuthorizationPendingException` → `SsoTokenResult::Pending` mapping and the
  two-page `nextToken` pagination would both fail if the functions mishandled
  them) but would NOT catch a regression in request construction (wrong URI,
  missing query param, wrong next-token propagation). The plan prescribed
  exactly this shape, so this is not plan-divergent; it bounds what the smoke
  suite guards. Not a tautology — response assertions flow through the real
  SDK parsers and the functions' own parsing logic — hence MINOR, not MAJOR.
- [MINOR] **`aws-smithy-types = "1"` dev-dep added beyond step 0's prescribed
  line.** Step 0 prescribes only `aws-smithy-http-client = { version = "1.1",
  features = ["test-util"] }`. The implementation also dev-deps
  `aws-smithy-types` (used for `SdkBody` in the smoke file). Harmless and
  within spec 06 0002-H4's dev-dep budget, but a plan-text deviation.
- [MINOR] **The net-new lockfile set is larger than step 0's "only
  aws-smithy-protocol-test net-new" disclosure.** `git diff 246434b..HEAD --
  Cargo.lock` adds: aws-smithy-protocol-test, pretty_assertions, yansi, diff,
  ciborium, ciborium-io, ciborium-ll, cbor-diag, bs58, separator, roxmltree
  (0.20.0). `cargo tree -i` confirms all trace to aws-smithy-protocol-test
  (dev-transitive under the `test-util` feature); nothing enters production
  dependency graphs; `cargo deny check` passes. This is the round-3 plan-eval
  "stale parenthetical" MINOR surfacing in fact: the load-bearing claims
  (licence-clean, deny-gated, dev-only) all hold; only the accounting prose
  was optimistic.

## Required changes (for FAIL)

1. Run `cargo fmt` (joins the two-line `let list_body = ...` in
   `crates/baeus-core/tests/aws_wizard_smoke.rs`), re-run the full gate, and
   amend the Implemented commit so the recorded "gate green" claim is true.
   No other code change is required.

## Notes

Everything not flagged above was verified mechanically and checks out:

- **0002-H3 typed errors — every previously-silent path now errors.**
  `KubeconfigInjectionError` (aws_sso.rs) has exactly the three prescribed
  variants, each Display-interpolating the offending context name. Both
  injection functions now return `Result<(), KubeconfigInjectionError>`; the
  double-`if let Some` fall-through is replaced by `ok_or_else` chains that
  return `ExecBlockMissing` when either `auth_info` or `exec` is `None` — no
  path reaches `Ok(())` without performing the injection. Context lookup →
  `ContextNotFound`, auth-info lookup → `AuthInfoNotFound` (the profile
  function previously lacked the context name in this message; the typed
  variant now carries both `user` and `context`). Mutating env-insertion
  bodies are byte-for-byte equivalent to the pre-slice logic.
- **Call-site anyhow-context wraps name the offending context** — client.rs
  `create_client_from_path` ("Injecting AWS profile '{profile}' into
  kubeconfig context '{context_name}' failed") and
  `create_client_from_path_with_aws_creds` ("Injecting wizard AWS credentials
  into kubeconfig context '{context_name}' failed"), both via
  `.with_context(...)?`, so `format!("{e:#}")` at app_shell.rs:1712 prints the
  wrap plus the typed Display. Step 5's UI no-edit claim spot-verified:
  `format!("{e:#}")` at app_shell.rs:1712, 18 `connection_errors` matches,
  no baeus-ui changes in the diff.
- **The eight `_with_config` seams exist** (`sso_register_client`,
  `sso_start_device_auth`, `sso_poll_for_token`, `sso_list_accounts`,
  `sso_get_role_credentials`, `authenticate_with_access_key`, `assume_role`,
  `discover_clusters_in_region`), each `pub` + `#[doc(hidden)]` with the
  test-injection doc-comment, each taking `&aws_config::SdkConfig` first, and
  each public wrapper preserving its signature and delegating after building
  the default config (region/credentials/no-credentials wiring unchanged —
  e.g. `assume_role` still injects `source_credentials` into the default
  config before delegating; `sso_get_role_credentials` retains the explicit
  `region` parameter per the plan). `authenticate_with_access_key_with_config`'s
  local `credentials` is live (returned in `AwsSession`), not dead code.
- **`create_eks_client_with_initial_ttl_secs` seam + 60s delegate** — public
  `create_eks_client` is a one-line delegate with `initial_ttl_secs = 60`; the
  hardcoded `now + 60s` became `Duration::seconds(initial_ttl_secs)`; the
  refresh closure's own 60s expiry is intentionally unchanged. Exactly the
  adjudicated shape.
- **The two evaluator implementation notes are resolved.** (1) Base64 CA
  fixture: `TEST_CA_B64` is base64(PEM) with a comment citing kube-client
  0.98.0's unconditional base64 decode (file_config.rs:641-646) — the round-3
  MINOR is correctly handled. (2) Feature parenthetical: the plan's stale
  crate-name parenthetical is not a code matter; the actual dep tree is
  dev-only and deny-clean (see MINOR #3 above for the accurate accounting).
- **H4 acceptance — `create_eks_client` is directly exercised.**
  `create_eks_client_returns_bearer_token_and_refreshes_past_leeway` (inline
  in aws_eks.rs) invokes `create_eks_client_with_initial_ttl_secs(&cluster,
  &credentials, 12)` — the full body of the public entry point (the public
  wrapper is a pure delegate, so covering the seam covers
  `create_eks_client`). The round-1 BLOCKER (no test invoking the function)
  is resolved through the round-3-adjudicated seam.
- **Step-6 timeline implements the adjudicated t0+12s → 3s sleep →
  exactly-one-refresh → counter==1 sequence** and would FAIL if refresh were
  broken (conceptual mutation check): seeded TTL 12s vs `REFRESH_LEEWAY_SECS =
  10`; first `list()` asserts `requests.len() == 1`, `Bearer k8s-aws-v1.`
  header, `!refresher.should_refresh()`, and
  `current_token()` == bearer_1 tail; 3s wall-clock sleep (mirroring slice B's
  proven pattern); second `list()` asserts `requests.len() == 2`,
  `bearer_2 != bearer_1` (mutation: closure never fires ⇒ bearer_2 == bearer_1
  ⇒ assert_ne fails; premature refresh at request 1 ⇒ the
  `!should_refresh()` and token-match assertions fail; refresher state not
  updated ⇒ the `current_token()` == bearer_2 assertion fails); wiremock
  `.expect(2)` guards phantom requests. The `generate_eks_token` test asserts
  the `k8s-aws-v1.` prefix and base64url-decodes the tail to check
  `Action=GetCallerIdentity`, `X-Amz-Signature`, and the credential prefix.
- **Green-path injection test asserts real env-var behavior** —
  `test_inject_credentials_sets_aws_env_vars_on_valid_context` asserts
  `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN` values
  after injection with `Some(token)`, then re-injects into a **fresh**
  kubeconfig with `None` and asserts `AWS_SESSION_TOKEN` absent — exactly the
  fresh-kubeconfig framing the round-2 eval demanded against the insert-only
  semantics.
- **Scope discipline** — `git diff 246434b..HEAD --stat` touches only:
  baeus-core (Cargo.toml, aws_eks.rs, aws_sso.rs, client.rs,
  tests/aws_wizard_smoke.rs), Cargo.lock, the slice-plan status line, and the
  orchestrator's review-findings artifact. Zero spec/ADR edits. No drive-by
  changes.
- **Review-findings adjudication** (per rubric): `/security-review`
  ran-clean — independently spot-confirmed: typed-error Display paths
  interpolate only context/user/profile names (never keys/tokens); injection
  writes to the in-memory kubeconfig only; seams accept caller-built configs
  while production wrappers still resolve real credentials; the TTL seam
  seeds only client-side refresh timing. Confirmed clean, no findings to map.
  `/code-review` skipped: command-unavailable — informational only; not read
  as a clean review; this evaluation performed the code-review dimension
  directly.
- **Prior-round accounting** — plan-eval rounds 1 and 2 FAILed; the round-3
  plan review PASSed at Round 2 (repeats the FAIL number). The standing
  counter is 2; this code-review FAIL increments it to Round 3.
