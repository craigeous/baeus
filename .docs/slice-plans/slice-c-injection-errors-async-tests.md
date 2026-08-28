# Slice C — AWS credential-injection typed errors + async wizard tests

Status: Plan Review
Target specs: `.docs/spec/06-remediation-highs.md` (§ 0002-H3 "Silent no-op
when exec block is absent in credential injection", § 0002-H4 "Zero async
tests for AWS SDK paths"); gate defined by
`.docs/spec/03-toolchain-and-gate.md`; research authority
`.docs/research/0002-core-client-aws-review.md`.

## Context

Two sibling defects sit at the top of the AWS wizard user flow, both named
as high-severity findings in research 0002 and grouped into spec 06's
**Slice C** row (0002-H3 and 0002-H4). Slice B (landed 2026-08-28 via PR
#7) has already refactored the EKS token surface — `EksTokenRefresher`,
`AuthRefreshLayer`, and the `(kube::Client, EksTokenRefresher)` return
shape from `create_eks_client` now exist in `aws_eks.rs`. Slice C is
the follow-on that spec 06 explicitly orders **after B**, because C's
async tests exercise the SDK-touching functions whose signatures B
introduced.

### 0002-H3 — silent Ok(()) when exec block absent

`crates/baeus-core/src/aws_sso.rs` currently exposes two injection
functions:

- `inject_aws_profile_into_kubeconfig` (aws_sso.rs:53) — used by
  `create_client_from_path` (client.rs:168, the AWS_PROFILE injection
  path used by the standard non-wizard EKS connect at
  app_shell.rs:1848).
- `inject_aws_credentials_into_kubeconfig` (aws_sso.rs:106) — used by
  `create_client_from_path_with_aws_creds` (client.rs:202, the live
  EKS-wizard connect at app_shell.rs:1704 where wizard-obtained
  in-memory credentials are injected into a temp kubeconfig's
  `exec` env so `aws eks get-token` can authenticate).

Both functions walk into `auth_info.auth_info.exec`
(aws_sso.rs:73-97 and :127-171) inside a double-`if let Some`. If either
`auth_info` or the inner `exec` block is `None`, control falls straight
through to the terminal `Ok(())` at aws_sso.rs:100 / :173. The injection
silently succeeds without touching the kubeconfig. Downstream, kube-rs
builds a `Client` against a kubeconfig whose exec plugin still uses
whatever env the caller had (or none), and the first API call returns a
401 / TLS handshake failure that mentions nothing about the missing
exec block.

The EKS wizard UI stores that opaque error string via
`this.connection_errors.insert(ctx, msg)` at app_shell.rs:1750 / :1764,
then surfaces it verbatim in the details panel via the
`connection_errors` map (app_shell.rs:3225 / :7355). Users see a
generic auth failure and cannot tell that their kubeconfig context
lacks an exec plugin at all.

### 0002-H4 — zero async tests for the AWS SDK surface

`aws_eks.rs` and `aws_sso.rs` today contain **no** `#[tokio::test]`
coverage of the SDK-touching async functions (verified 2026-08-28:
the only `#[tokio::test]` blocks in `aws_eks.rs` are slice B's
`EksTokenRefresher` unit tests plus the `AuthRefreshLayer` wiremock
acceptance test; none exercise the wizard entry points named in
research 0002 §4). The nine functions research 0002 §4 enumerates are:

- `sso_register_client` (aws_eks.rs:234)
- `sso_start_device_auth` (aws_eks.rs:257)
- `sso_poll_for_token` (aws_eks.rs:293)
- `sso_list_accounts` (aws_eks.rs:343)
- `sso_get_role_credentials` (aws_eks.rs:416)
- `authenticate_with_access_key` (aws_eks.rs:471)
- `assume_role` (aws_eks.rs:508)
- `create_eks_client` (aws_eks.rs:965 — slice B added
  `eks_refresh_layer_refreshes_token_and_updates_authorization_header`
  at aws_eks.rs:1377, but that test constructs `EksTokenRefresher` and
  layers `AuthRefreshLayer` **manually** and never invokes
  `create_eks_client` itself. Slice C adds a direct-invocation test in
  step 6 to satisfy spec 06 0002-H4 acceptance 4a for this function.)
- `generate_eks_token` (aws_eks.rs:691 — pure client-side signer;
  network-free)

Each function currently calls `aws_config::defaults(...).load().await`
inline (e.g. aws_eks.rs:235, :263, :299, :344, :383, :422, :480, :512,
:621) and constructs its SDK client from the loaded config. To route
these calls to a mock backend the plan adds an internal seam per
function that accepts an `aws_config::SdkConfig` (or a preconstructed
SDK client) so tests can inject a `StaticReplayClient` via
`.http_client(...)` (smithy-native mock — the spec-preferred choice;
see step 4) without duplicating production wiring.

### In scope (Slice C, per spec 06's frozen acceptance)

**0002-H3 typed error + call-site propagation:**

- Introduce a `KubeconfigInjectionError` typed enum in
  `crates/baeus-core/src/aws_sso.rs`. Variant
  `ExecBlockMissing { context: String }` fires when `auth_info.exec`
  is `None` for the resolved context. Existing not-found paths
  (`context` / `AuthInfo` lookup) become their own variants
  (`ContextNotFound`, `AuthInfoNotFound`) so the enum covers the
  full failure surface of the two functions and their Display impl
  can name the offending context in each case.
- Change both `inject_aws_profile_into_kubeconfig` and
  `inject_aws_credentials_into_kubeconfig` from `Result<()>` (anyhow)
  to `Result<(), KubeconfigInjectionError>`.
- Update the two call sites in `client.rs`:
  - `create_client_from_path` at client.rs:168 — propagate via `?`
    and let anyhow wrap the typed error at the boundary. Add a
    `with_context(|| format!("Injecting AWS profile '{profile}' into
    kubeconfig context '{context_name}'"))` frame so anyhow's
    chain-print (`format!("{e:#}")`) prints both the injection
    context and the typed variant Display.
  - `create_client_from_path_with_aws_creds` at client.rs:202 —
    same treatment; wrap with a per-call anyhow context.
- Verify the EKS-wizard UI diagnostic surface (app_shell.rs:1704ff)
  requires no code change: it already renders `format!("{e:#}")`
  (app_shell.rs:1712) into `connection_errors`, so the typed
  Display + anyhow chain lands verbatim. A grep-scope check on the
  UI is part of step 5.
- **The two existing green-path unit tests in `aws_sso.rs`
  (`test_inject_aws_profile_into_kubeconfig`,
  `test_inject_aws_profile_overwrites_existing`,
  `test_inject_missing_context_returns_error`) must continue to
  pass** after the signature change; the plan is a minimal edit
  (adjust `.is_err()` / `.is_ok()` usage — `Result<(), KubeconfigInjectionError>`
  keeps both), no test deletions.

**0002-H4 async tests + minimal API-surface tweaks:**

- Add an inner "`_with_config`" variant for each of the six SSO / SSO-OIDC /
  STS / EKS functions listed under 0002-H4 that currently build an
  `SdkConfig` inline. The public function keeps its current signature
  (backward compatibility) and becomes a one-line wrapper that builds
  the default `SdkConfig` and delegates to the new inner. Tests call
  the inner form directly with an `SdkConfig` whose `http_client` is a
  `StaticReplayClient` (smithy-native mock; see step 4).
- Add `crates/baeus-core/tests/aws_wizard_smoke.rs` as the integration
  test file spec 06 0002-H4 names verbatim, covering at minimum:
  device-auth happy path, `AuthorizationPendingException` retry, token
  exchange, cluster discovery, `authenticate_with_access_key`,
  `assume_role`, and `sso_get_role_credentials` — all via
  `aws-smithy-http-client`'s smithy-native `StaticReplayClient` mock
  (the non-deprecated path in the locked tree; see step 0 for the
  crate-choice rationale).
- Add an inline `#[tokio::test]` in `aws_eks.rs` for `create_eks_client`
  that invokes its body directly through a new
  `create_eks_client_with_initial_ttl_secs` seam (public
  `create_eks_client` becomes a one-line delegate with `ttl_secs = 60`).
  The test asserts the initial Bearer header via a wiremock K8s API
  server, then a distinct refreshed Bearer after a 3-second wall-clock
  sleep drops the seeded 12-second initial expiry inside the 10-second
  refresh leeway. Slice B's `eks_refresh_layer_...` test at
  aws_eks.rs:1377 covers `AuthRefreshLayer` construction manually but
  does not call `create_eks_client`; spec 06 0002-H4 acceptance 4a
  requires this direct-invocation test. See step 6 for the full
  seam prescription and timeline walk.
- Add one inline `#[tokio::test]` in `aws_eks.rs` for `generate_eks_token`
  — a pure client-side signer whose test needs no SDK-mock surface and
  is thin enough to sit inline.
- **Adopt the AWS smithy-native mock as primary; wiremock only where
  the smithy mock cannot cover a specific call.** Spec 06 0002-H4
  fix-approach text: "prefer AWS smithy mock; fall back to `wiremock`
  only if the smithy mock cannot cover a given call." The plan uses
  `aws-smithy-http-client`'s stable `test-util` feature (specifically
  `StaticReplayClient`, attached to `SdkConfig` via `.http_client(...)`)
  as the primary mock transport for every SDK-touching test. A per-call
  review (step 4 test list, below) shows StaticReplayClient covers all
  eight SDK operations exercised by the smoke file — including paginated
  `ListAccounts` and the 400-body `AuthorizationPendingException` case —
  so no per-call wiremock fallback is invoked for the wizard smoke file.
  wiremock (Cargo.toml:49, retained from slice B) continues to serve
  the **K8s API server side** (which is not an AWS SDK call and is
  therefore outside smithy-mock scope) — namely slice B's inline
  `AuthRefreshLayer` acceptance test and the new `create_eks_client`
  test added in step 6.

### Explicit non-goals (deferred per spec 06 Out of scope + slice-plan bounding)

- **The 0002-H2 refresher wiring beyond `create_eks_client`.** Slice B
  landed the refresher; wiring it into `ClusterConnection.token_expiry`
  or migrating the live wizard connect at app_shell.rs:1704 (which
  uses `create_client_from_path_with_aws_creds`, not
  `create_eks_client`) is deferred per spec 06's 2026-08-28
  clarification note and slice B's Notes.
- **Watch cancellation (0002-H1).** Landed in slice B.
- **The medium-severity `get_caller_identity` shell-out replacement
  (research 0002 §5).** Out of scope for spec 06; deferred to a
  later cycle.
- **Migrating `AccessKeyConfig.secret_access_key` /
  `AwsSession.sso_access_token` from `String` to `SecretString`
  (research 0002 §6).** Medium; deferred.
- **`InformerManager` state-machine hardening (research 0002 §7),
  bridge duplicate-key orphan (research 0002 §8), unbounded list
  requests (§9), sequential describe-cluster (§10),
  `sso_get_role_credentials` fabricated ARN (§11).** All medium;
  deferred.
- **UI redesign of the EKS-wizard error panel.** The typed error
  reaches the UI via the existing `format!("{e:#}")` +
  `connection_errors` machinery; no new UI widgets or renderers
  are in scope. Spec 06 0002-H3 acceptance is met when the
  diagnostic message identifies the offending context name —
  Display + anyhow-context achieves that at the injection
  boundary; the UI already renders it.
- **Replacing `StaticReplayClient` with `aws-smithy-mocks-experimental`**
  (or the newer `aws-smithy-mocks`). `aws-smithy-http-client`'s
  `test-util` feature (stable, AWS-native, gives `StaticReplayClient` —
  the non-deprecated locked-tree path spec 06 prefers) covers every SDK
  operation this slice touches; the experimental crate is not needed.
  A future planning cycle may migrate if `aws-smithy-mocks` proves
  more ergonomic.
- **Adding tests for `sso_list_account_roles`** (aws_eks.rs:378).
  Not in research 0002 §4's list. Fine to include if the wizard
  smoke naturally covers the endpoint; not gated.
- **Any refactor beyond the minimum needed to inject a mock config.**
  The `_with_config` seam is the smallest surface change that
  satisfies "constructor-injection or a `SdkConfig` parameter"
  (spec 06). No larger structural rewrite of `aws_eks.rs` /
  `aws_sso.rs` is in scope.

## Steps

Ordered so tests are red before implementation, dependencies land
before consumers, and the H3 and H4 tracks stay disjoint (they touch
different regions of `aws_sso.rs` / `aws_eks.rs` and can be reviewed
independently).

### 0. `crates/baeus-core/Cargo.toml` — dev-dependencies

**Add `aws-smithy-http-client` as a dev-dep with the `test-util` feature
enabled.** Prescribed line:

```toml
aws-smithy-http-client = { version = "1.1", features = ["test-util"] }
```

`aws-smithy-http-client 1.1.12` is already present in `Cargo.lock` as a
transitive dep (verified via `grep aws-smithy-http-client Cargo.lock`);
this line activates its `test-util` feature so `StaticReplayClient` is
compiled into dev/test targets. Spec 06 0002-H4 explicitly authorises
`[dev-dependencies]` addition "for the chosen mock", and this crate is
the AWS smithy-native mock spec 06 prefers over wiremock.

**Why `aws-smithy-http-client`, not `aws-smithy-runtime`.** In the locked
aws-smithy-runtime 1.10.3, the `test_util` module re-export at
`aws_smithy_runtime::client::http::test_util` carries
`#[deprecated = "… Please use the `test-util` feature from
`aws-smithy-http-client` instead"]`. Under
`RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
-D warnings` (ci.yml:75), referencing the deprecated module errors on
lint. The non-deprecated canonical path in the locked tree is
`aws_smithy_http_client::test_util::StaticReplayClient` (defined at
`src/test_util/replay.rs:154` of aws-smithy-http-client 1.1.12). All
step 4 imports must use this path; step 6 does not import
`StaticReplayClient` at all (it uses wiremock — see step 6).

**Dep-tree delta (accurate accounting).** Enabling
`aws-smithy-http-client/test-util` transitively pulls in
`aws-smithy-protocol-test` (present in that crate's feature list under
`test-util`), which is **net-new** to the lockfile (verified: `grep
aws-smithy-protocol-test Cargo.lock` returns nothing pre-slice). The
other transitively-pulled crates under this feature (`tracing-subscriber`,
`http` 0.2, `hyper` 0.14) are already locked. The implementer runs
`cargo tree -p baeus-core --edges dev` and `cargo deny check` after the
add and captures the diff in the commit description; if
`aws-smithy-protocol-test`'s licence or any transitive advisory trips
`cargo deny`, the slice returns Needs Clarification (spec 06 0002-H4
gate). Expected outcome: MIT/Apache-2.0 dual-licence, no advisories —
same class as existing AWS crates in the tree.

`wiremock 0.6` (Cargo.toml:49, from slice B) is retained without change —
step 6's `create_eks_client` test uses it for the K8s API server side
(not an AWS SDK call, hence outside smithy-mock scope; the spec's
per-call fallback condition applies).

### 1. `crates/baeus-core/src/aws_sso.rs` — introduce `KubeconfigInjectionError` typed enum (test-first)

**Red tests first.** Extend the existing `#[cfg(test)] mod tests`
block with five new `#[test]` cases, all synchronous (no SDK
involvement):

- `test_inject_profile_returns_exec_block_missing_when_absent` —
  build a kubeconfig where the resolved `AuthInfo` has `exec: None`,
  call `inject_aws_profile_into_kubeconfig`, assert
  `Err(KubeconfigInjectionError::ExecBlockMissing { context })` with
  `context == "my-cluster"`.
- `test_inject_credentials_returns_exec_block_missing_when_absent` —
  same shape for `inject_aws_credentials_into_kubeconfig`.
- `test_inject_credentials_returns_context_not_found` — kubeconfig
  with no matching context; asserts
  `Err(KubeconfigInjectionError::ContextNotFound { context })`.
- `test_inject_credentials_returns_auth_info_not_found` — context
  references a user with no matching `AuthInfo`; asserts
  `Err(KubeconfigInjectionError::AuthInfoNotFound { user, context })`.
- `test_inject_credentials_sets_aws_env_vars_on_valid_context` —
  green-path proof for `inject_aws_credentials_into_kubeconfig` per
  spec 06 0002-H3 test expectation (a) ("valid exec-block kubeconfig")
  for **both** inject functions. Build a kubeconfig where the resolved
  `AuthInfo` has a valid `exec` block, call
  `inject_aws_credentials_into_kubeconfig(&mut kc, "my-cluster",
  "AKIAEXAMPLE", "secret-key", Some("session-token-1"))`, then assert
  the exec block's `env` vec contains three entries with
  `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN`
  bound to the passed values. Then build a **second, fresh** kubeconfig
  (identical structure, no prior injection) and call the function again
  with `session_token = None`; assert the env vec contains
  `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` but **does not
  contain** `AWS_SESSION_TOKEN`. The fresh-kubeconfig framing is
  required because the insertion path at aws_sso.rs:155-169
  (`if let Some(token) = session_token { … env.push(…) }`) is
  insert-only: it never removes an existing `AWS_SESSION_TOKEN`
  binding, so a "no longer contains" assertion on the same reused
  kubeconfig would fail (the token from the first call would remain).
  This gives regression coverage on the session-token insertion path
  in this step's `match`-based control flow (the mutating body is
  otherwise unchanged). The sibling function is already covered at
  aws_sso.rs:224 / :273; this test brings the credentials function to
  parity.

The existing three tests
(`test_inject_aws_profile_into_kubeconfig`,
`test_inject_aws_profile_overwrites_existing`,
`test_inject_missing_context_returns_error`) stay unchanged in
intent; the final one is retitled to
`test_inject_missing_context_returns_context_not_found` and asserts
the new typed variant. No test deletions.

**Then define the enum.** Add near the top of `aws_sso.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum KubeconfigInjectionError {
    #[error("Kubeconfig context '{context}' not found")]
    ContextNotFound { context: String },
    #[error("AuthInfo '{user}' (referenced by context '{context}') not found")]
    AuthInfoNotFound { user: String, context: String },
    #[error(
        "Kubeconfig context '{context}' has no `exec` block; AWS credential \
         injection requires an exec plugin. Add an `exec` block referencing \
         `aws eks get-token` or select a different context."
    )]
    ExecBlockMissing { context: String },
}
```

Rationale for the three variants (not just `ExecBlockMissing`): spec
06 0002-H3's acceptance criterion "Error message identifies the
offending context name" applies to every failure the function can
signal, not only the exec-missing case. Making all failures typed
keeps the caller's diagnostic surface uniform and removes the
`anyhow::Context::with_context` string-formatting duplication that
today provides the diagnostic for the not-found paths.

**Then refactor the two injection functions.** Change their return
type from `Result<()>` (anyhow) to `Result<(),
KubeconfigInjectionError>`. Replace each `.with_context(|| format!(
"Context '{context_name}' not found …"))?` with an explicit
`.ok_or_else(|| KubeconfigInjectionError::ContextNotFound { context:
context_name.to_string() })?` and its `AuthInfoNotFound` sibling.
Replace the double-`if let Some { … }` fall-through (aws_sso.rs:73
and :127) with an explicit `match auth_info.auth_info` /
`match ai.exec` that returns
`Err(KubeconfigInjectionError::ExecBlockMissing { context })`
when either is `None`. The mutating body (env-var insertion) is
unchanged.

**Public API compatibility.** `inject_aws_profile_into_kubeconfig`
has two call sites (client.rs:168, test file — verified via
`grep -rn 'inject_aws_profile_into_kubeconfig\|
inject_aws_credentials_into_kubeconfig' crates/`); both are in
this repo. Both accept the return via `?` in an `anyhow::Result`
context, which coerces any `Error`-impl via
`From<T> for anyhow::Error` (thiserror derives the impl). So the
signature change is transparent at the call sites — no `From`
impl needed beyond the `#[derive(thiserror::Error)]`. The
`test_inject_missing_context_returns_error` legacy assertion
`assert!(result.is_err())` remains valid.

### 2. `crates/baeus-core/src/client.rs` — anyhow-context wrap at the injection boundary

The two call sites are already `?`-propagating. Add a
`.with_context(|| ...)` frame on each so that
`format!("{e:#}")` (used at app_shell.rs:1712) prints both the
injection context and the typed variant Display.

`client.rs:168` — `create_client_from_path`:

```rust
if let Some(profile) = aws_profile {
    crate::aws_sso::inject_aws_profile_into_kubeconfig(&mut kubeconfig, context_name, profile)
        .with_context(|| {
            format!("Injecting AWS profile '{profile}' into kubeconfig context \
                     '{context_name}' failed")
        })?;
}
```

`client.rs:202` — `create_client_from_path_with_aws_creds`:

```rust
crate::aws_sso::inject_aws_credentials_into_kubeconfig(
    &mut kubeconfig,
    context_name,
    access_key_id,
    secret_access_key,
    session_token,
)
.with_context(|| {
    format!("Injecting wizard AWS credentials into kubeconfig context \
             '{context_name}' failed")
})?;
```

No signature change on either function (still `anyhow::Result<Client>`).
No new tests here — the H3 behaviour is exercised by step 1's unit
tests on the injection functions plus step 5's UI-surface grep.

### 3. `crates/baeus-core/src/aws_eks.rs` and `aws_sso.rs` — extract `_with_config` seams (mock-injection surface)

**Rationale.** Spec 06 0002-H4 fix-approach: "minimal API surface
tweaks needed only to make the SDK client configurable/injectable
for the mock transport (constructor-injection or a `SdkConfig`
parameter)." The chosen shape is one private (crate-internal)
`_with_config` variant per SDK-touching function that today builds
`aws_config::defaults(...)` inline. The public function's signature
is unchanged — it becomes a one-liner that loads the default
`SdkConfig` and delegates.

For each of the following, add a `pub(crate) async fn NAME_with_config`
variant that takes `&aws_config::SdkConfig` as its first parameter
and moves the SDK-client construction and all logic downstream of
that construction into the inner. The public function preserves
today's `aws_config::defaults(BehaviorVersion::latest())…load().await`
build and delegates:

- `sso_register_client(region)` →
  `sso_register_client_with_config(sdk_config)` (aws_eks.rs:234).
- `sso_start_device_auth(region, client_id, client_secret, start_url)` →
  `sso_start_device_auth_with_config(sdk_config, client_id,
  client_secret, start_url)` (aws_eks.rs:257).
- `sso_poll_for_token(region, client_id, client_secret, device_code)` →
  `sso_poll_for_token_with_config(sdk_config, client_id,
  client_secret, device_code)` (aws_eks.rs:293).
- `sso_list_accounts(region, access_token)` →
  `sso_list_accounts_with_config(sdk_config, access_token)`
  (aws_eks.rs:343).
- `sso_get_role_credentials(region, access_token, account_id,
  role_name)` → `sso_get_role_credentials_with_config(sdk_config,
  access_token, account_id, role_name, region)` (aws_eks.rs:416 —
  `region` retained explicitly because it is embedded in
  `AwsSession.region` and the fabricated `identity_arn`, and
  `SdkConfig` in test may not reflect the same value).
- `authenticate_with_access_key(config)` →
  `authenticate_with_access_key_with_config(sdk_config, config)`
  (aws_eks.rs:471).
- `assume_role(config, source_credentials)` →
  `assume_role_with_config(sdk_config, config)` (aws_eks.rs:508 —
  the source credentials become the SdkConfig's credentials
  provider in the public wrapper; the inner takes an already-built
  `SdkConfig`).
- `discover_clusters_in_region(credentials, region)` →
  `discover_clusters_in_region_with_config(sdk_config, region)`
  (aws_eks.rs:617 — inner needed for step 4's cluster-discovery
  test).

Visibility: `pub(crate)` so tests in `tests/aws_wizard_smoke.rs`
(same crate boundary — integration tests see `pub` items only)
require the inner functions to be `pub` **or** the smoke test uses
a `#[path = "..."]`-inlined helper. The cleaner choice is:

- Mark the inner functions **`pub`** (not `pub(crate)`) with a
  `#[doc(hidden)]` attribute and a doc comment
  ("Internal seam for test injection; public consumers should use
  the non-`_with_config` form."). This matches how kube-rs and
  aws-sdk-* expose their own test hooks and lets the smoke test
  live in `tests/` as an integration test (spec 06's exact
  file-placement prescription).

No new public API for regular consumers — the wrapper is the API,
and it is unchanged. Slice B added `EksTokenRefresher` as
`pub`; the pattern is consistent.

**No test additions in this step.** The seams are behaviour-preserving
refactors verified by the existing (green) pre-slice tests and by
step 4's newly-added smoke tests. Every public function's signature
stays the same, and every existing caller in the workspace (verified
via `grep -rn 'sso_register_client\|sso_start_device_auth\|
sso_poll_for_token\|sso_list_accounts\|sso_get_role_credentials\|
authenticate_with_access_key\|assume_role\|
discover_clusters_in_region' crates/ | grep -v 'src/aws_eks.rs'`) —
predominantly `app_shell.rs` and the EKS-wizard render module —
compiles unchanged.

### 4. `crates/baeus-core/tests/aws_wizard_smoke.rs` — new integration test file (test-first, complete surface)

Create the file spec 06 0002-H4 names verbatim. **Primary mock:**
`aws_smithy_http_client::test_util::StaticReplayClient` (the AWS
smithy-native mock, gated behind the `test-util` feature of
`aws-smithy-http-client` added in step 0). This is the non-deprecated
canonical path in the locked tree (aws-smithy-http-client 1.1.12; the
`aws_smithy_runtime::client::http::test_util` re-export is
`#[deprecated]` in aws-smithy-runtime 1.10.3 and would fail `-D warnings`
— see step 0). Each test builds a `Vec<ReplayEvent>` with expected
request/response pairs, constructs a `StaticReplayClient` from that vec,
and attaches it to an `aws_config::SdkConfig` via
`.http_client(replay_client.clone())`, then passes the config to the
`_with_config` inner. `SdkConfig` is otherwise built with
`aws_config::defaults(BehaviorVersion::latest())`
`.region(Region::new("us-east-1"))`
`.credentials_provider(Credentials::new("AKIA…", "secret", None, None,
"test"))` (or `.no_credentials()` for the OIDC bootstrap flows) and
`.load().await`. Response bodies are inline `serde_json::json!({...})`
values matching each SDK operation's wire format (verified against the
`aws-sdk-*` crate documentation for the version pinned in `Cargo.lock`);
for STS's query-protocol operations (`GetCallerIdentity`, `AssumeRole`)
the body is an XML string literal per the SDK's response schema.

**Per-call mock coverage review (spec 06's conditional).** Each test
below was reviewed against `StaticReplayClient`'s coverage model
(deterministic request → response replay, arbitrary HTTP status, body
matching, headers accessible). All eight SDK operations exercised by
this file are coverable; **no wiremock fallback is invoked for any
test in this file**. (The ninth and tenth 0002-H4 functions —
`create_eks_client`, `generate_eks_token` — live in step 6, one
wiremock-backed for its K8s-API-server surface, one mock-free.)

Test set (each is a single `#[tokio::test]` unless noted):

- **`sso_register_client_returns_client_id_and_secret`** — replay one
  `RegisterClient` OIDC response
  `{"clientId": "cid-1", "clientSecret": "csec-1", …}` (HTTP 200).
  Asserts the tuple returned matches. Single r/r — StaticReplayClient
  natively covers.
- **`sso_start_device_auth_returns_device_and_user_codes`** — replay
  a `StartDeviceAuthorization` response with `deviceCode`, `userCode`,
  `verificationUri`, `verificationUriComplete`, `expiresIn: 600`,
  `interval: 5`. Asserts the parsed `SsoDeviceAuth` values (including
  `poll_interval` matching 5 s and `expires_at` roughly `now + 600 s`).
  Single r/r — covered.
- **`sso_poll_for_token_returns_pending_then_success`** — folded
  two-step test (per plan-eval MINOR #5) that proves 4b's
  retry-on-pending wording directly. Replay two `CreateToken` events on
  the same client instance: first responds `400` with body
  `{"error": "authorization_pending", …}`, second responds `200` with
  `{"accessToken": "at-1", "expiresIn": 3600, …}`. Test invokes
  `sso_poll_for_token_with_config` twice against the same
  `SdkConfig`+client, asserts the first return is
  `SsoTokenResult::Pending` (matching `is_authorization_pending_exception`
  at aws_eks.rs:325-331) and the second is `SsoTokenResult::Success`
  with the expected access token and a near-1-hour expiry. Two r/r
  events — StaticReplayClient covers deterministically.
- **`sso_list_accounts_paginates_and_returns_all`** — replay two
  events: first `ListAccounts` response returns two accounts and a
  `nextToken`; second returns two more accounts and no `nextToken`.
  Asserts four accounts in the returned vec. StaticReplayClient
  matches events in request order — the function's inner pagination
  loop is deterministic, so replay ordering is sufficient.
- **`sso_get_role_credentials_returns_session`** — replay one
  `GetRoleCredentials` response with a `roleCredentials` object
  containing `accessKeyId`, `secretAccessKey`, `sessionToken`,
  `expiration`. Asserts the returned `AwsSession` carries those
  credentials and the region.
- **`authenticate_with_access_key_returns_session`** — replay one STS
  `GetCallerIdentity` XML response
  (`<GetCallerIdentityResponse><…><Arn>…</Arn></…>`). Asserts the
  returned `AwsSession.account_id`, `.identity_arn` match.
  StaticReplayClient body-matches on the XML — query-protocol
  covered.
- **`assume_role_returns_session_with_temporary_credentials`** —
  replay one STS `AssumeRole` XML response with an
  `AssumedRoleUser.Arn` and temporary `Credentials`. Asserts the
  returned `AwsSession` carries the expected ARN and expiry.
- **`discover_clusters_in_region_returns_described_clusters`** —
  replay three events in order: `ListClusters` returns two names,
  then two `DescribeCluster` responses each returning an `EksCluster`
  payload. Asserts the returned `Vec<EksCluster>` has both, in the
  right region, with certificate-authority data present.
  StaticReplayClient's deterministic ordering matches the function's
  sequential describe loop.

**Endpoint routing.** All tests use `Region::new("us-east-1")` and
attach the `StaticReplayClient` via `SdkConfig.http_client(...)`, so
the SDK routes all outbound HTTP through the replay client — no live
network calls, no per-service host mismatch.

**Concurrency and time.** No wall-clock sleeps are needed. Each test
constructs its own `StaticReplayClient` (`StaticReplayClient::new(...)`)
— test isolation is per-`SdkConfig`, matching wiremock's per-`MockServer`
model.

**Error-typing acceptance:** spec 06 0002-H4 does not itself add a
"typed error" acceptance to this file — the acceptance is one
`#[tokio::test]` per function plus the `AuthorizationPendingException`
retry case. Test count for the file: 8 tests (one fewer than the
pre-fold count of 9, per plan-eval MINOR #5 folding
pending+success).

### 5. `crates/baeus-ui/src/layout/app_shell.rs` — verify no UI edit is needed for H3 diagnostic surface

**No code edit.** This step is a verification pass, not a
modification:

1. `grep -n 'connection_errors' crates/baeus-ui/src/layout/app_shell.rs`
   (already run 2026-08-28; matches at :498, :734, :1668, :1750, :1764,
   :1907, :1966, :2091, :2532, :2825, :3132, :3148, :3199, :3224, :3225,
   :3230, :3235, :7355).
2. Verify the EKS-wizard error rendering at app_shell.rs:1704ff routes
   through `format!("{e:#}")` (line :1712) into
   `connection_errors.insert(ctx, enriched)` (line :1750). anyhow's
   chain-print (`{e:#}`) walks the entire causal chain, which for the
   ExecBlockMissing case is:
   `Injecting wizard AWS credentials into kubeconfig context '<ctx>' failed:
    Kubeconfig context '<ctx>' has no `exec` block; AWS credential
    injection requires an exec plugin. Add an `exec` block referencing
    `aws eks get-token` or select a different context.`
3. Confirm the details-panel render at app_shell.rs:7355 emits the
   error string as-is (no truncation / no keyword-filter that would
   strip the diagnostic).

If any of the three checks fail — e.g. the details panel truncates
the message under a hard length limit — the slice-plan **stops** and
returns Needs Clarification: the spec's H3 acceptance ("Error
message identifies the offending context name") would then require
a UI edit that spec 06 does not scope into Slice C. Based on the
current tree (verified 2026-08-28) no such truncation exists; step 5
is expected to be a pure grep-and-eyeball verification.

### 6. `crates/baeus-core/src/aws_eks.rs` — inline `#[tokio::test]` for `generate_eks_token` and `create_eks_client`

Add two `#[tokio::test]` cases inline (in the existing
`#[cfg(test)] mod tests` block, alongside slice B's refresher tests):

- **`generate_eks_token_returns_prefixed_and_base64_url_encoded`** —
  pure client-side signer test (no SDK-mock surface). Construct
  `Credentials::new("AKIA…", "secret", None, None,
  "test-provider")`, call
  `generate_eks_token("my-cluster", &creds, "us-east-1").await?`,
  assert the returned string starts with `"k8s-aws-v1."` and that
  the tail (post-prefix) decodes as base64url to a
  `https://sts.us-east-1.amazonaws.com/?…` URL containing
  `Action=GetCallerIdentity`, `X-Amz-Signature=`, and the correct
  `X-Amz-Credential` prefix.

- **`create_eks_client_returns_bearer_token_and_refreshes_past_leeway`**
  — end-to-end invocation of `create_eks_client` (aws_eks.rs:965)
  itself, satisfying spec 06 0002-H4 acceptance 4a's "at least one
  `#[tokio::test]` exists for every function listed" for
  `create_eks_client` specifically. The fix-approach text names a
  "`create_eks_client` round-trip (which now includes the refresher
  from 0002-H2)" — this test proves both halves: (i) `create_eks_client`
  returns a live `kube::Client` whose outbound requests carry the
  presigned bearer, and (ii) the refresh closure captured by
  `create_eks_client` regenerates a fresh presigned token when the
  10-second leeway elapses, exactly the "which now includes the
  refresher" phrase.

  **Seam prescription (initial-TTL seed).** Add a new inner variant
  alongside slice B's `create_eks_client` at aws_eks.rs:965. It is
  the minimum surface change needed to make the initial-token TTL
  test-configurable; every existing caller continues to invoke the
  public wrapper unchanged. Mirrors step 3's `_with_config` seam
  discipline (`pub` + `#[doc(hidden)]` + doc-comment naming the
  test-injection contract):

  ```rust
  /// Test-injection seam for `create_eks_client`. Public consumers must
  /// use `create_eks_client`, which delegates here with a 60-second
  /// initial TTL. The parameter exists only so async wizard tests can
  /// seed an initial expiry outside/inside the 10-second refresh leeway
  /// deterministically (spec 06 0002-H4 acceptance 4a for
  /// `create_eks_client`).
  #[doc(hidden)]
  pub async fn create_eks_client_with_initial_ttl_secs(
      cluster: &EksCluster,
      credentials: &Credentials,
      initial_ttl_secs: i64,
  ) -> Result<(kube::Client, EksTokenRefresher)> { /* body */ }
  ```

  Refactor: move the current `create_eks_client` body (aws_eks.rs:965-1054)
  into the new inner and replace the hardcoded
  `chrono::Utc::now() + chrono::Duration::seconds(60)` at aws_eks.rs:981
  with `chrono::Utc::now() + chrono::Duration::seconds(initial_ttl_secs)`.
  The public `create_eks_client(cluster, credentials)` becomes a one-line
  wrapper: `create_eks_client_with_initial_ttl_secs(cluster, credentials,
  60).await`. The refresh closure's own `expires_at` at aws_eks.rs:998
  (`now + 60s`) is intentionally left as 60s — the test observes exactly
  one refresh, and the post-refresh expiry is irrelevant to any
  assertion. This keeps the seam minimal (one added parameter, one
  call-site change).

  **Test body — round-trip through `create_eks_client_with_initial_ttl_secs`.**
  Build an `EksCluster` with a **valid self-signed CA PEM** (embedded as
  a `const &str` at the top of the test — a well-formed
  `-----BEGIN CERTIFICATE-----` payload that rustls-pemfile parses; no
  external cert generation needed and no new dev-deps), a `name`/`region`/
  `arn` of `"test-cluster"`/`"us-east-1"`/
  `"arn:aws:eks:us-east-1:123456789012:cluster/test-cluster"`, and an
  `endpoint` pointing at a wiremock `MockServer` URI (which accepts any
  request and returns an empty `NamespaceList`, mirroring slice B's
  `eks_refresh_layer_...` mock at aws_eks.rs:1387-1396,
  `.expect(2)`). Construct
  `Credentials::new("AKIAEXAMPLE", "secretkey", None, None, "test")` and
  call `let (client, refresher) = create_eks_client_with_initial_ttl_secs(
  &cluster, &credentials, 12).await?;`. This exercises the function's
  full body: `generate_eks_token` invocation, refresh closure
  construction, `kube::Config::from_custom_kubeconfig` build,
  `rustls_https_connector()` build, and `ServiceBuilder` stack assembly
  — the public `create_eks_client` is a one-line delegate to this
  function, so coverage of the seam is coverage of the public entry
  point.

  **Timeline walk (mirrors slice B aws_eks.rs:1397-1478 exactly).** Let
  `t0` be the instant the constructor returns. The refresher holds
  `expires_at = t0 + 12s`. The 10-second `REFRESH_LEEWAY_SECS`
  (aws_eks.rs:806) means `should_refresh()` compares `now + 10s` against
  `expires_at`.

  1. At `t ≈ t0` (fast path):
     `now + 10s = t0 + 10s < expires_at = t0 + 12s` → `should_refresh()`
     is false. Call
     `let api: kube::Api<Namespace> = kube::Api::all(client.clone());`
     followed by `api.list(&Default::default()).await` — one outbound
     HTTP request. `AuthRefreshService::call()` (aws_eks.rs:927-950)
     observes `should_refresh() == false`, skips the refresh call, reads
     `current_token()`, and inserts `Bearer <initial-token>` into the
     Authorization header. Wiremock records request 1.
     - Assert `mock_server.received_requests().await.len() == 1`.
     - Assert `requests[0].headers["authorization"]` starts with
       `"Bearer k8s-aws-v1."`. Save that string as `bearer_1`.
     - Assert `refresher.should_refresh()` is false.
     - Assert `refresher.current_token()`'s exposed secret equals the
       tail of `bearer_1` after the `"Bearer "` prefix (initial token
       untouched, no refresh has fired).

  2. Sleep 3s (real wall-clock — this test does NOT use
     `tokio::time::pause`; slice B's own test uses the same 3s wall-clock
     sleep at aws_eks.rs:1458):
     `tokio::time::sleep(std::time::Duration::from_secs(3)).await;`

  3. At `t ≈ t0 + 3s` (refresh path):
     `now + 10s = t0 + 13s ≥ expires_at = t0 + 12s` (13 ≥ 12) →
     `should_refresh()` is true. Call
     `api.list(&Default::default()).await` again — one outbound HTTP
     request. `AuthRefreshService::call()` observes
     `should_refresh() == true` and enters `refresher.refresh().await`.
     `refresh()` re-checks under the `tokio::sync::Mutex` (aws_eks.rs:859-863),
     `should_refresh()` remains true, and it invokes the refresh closure
     captured by `create_eks_client_with_initial_ttl_secs`
     (aws_eks.rs:988-1001) — that closure calls `generate_eks_token`
     which produces a fresh presigned URL (differing from the initial at
     least in `X-Amz-Date` and therefore `X-Amz-Signature`). The service
     reads the newly written `current_token()` and inserts a new
     `Bearer <refreshed-token>` header. Wiremock records request 2.
     - Assert `mock_server.received_requests().await.len() == 2` (the
       `.expect(2)` on the mock verifies no phantom requests).
     - Save `requests[1].headers["authorization"]` as `bearer_2`.
     - Assert `bearer_2` starts with `"Bearer k8s-aws-v1."`.
     - Assert `bearer_2 != bearer_1` (distinct presigned URLs, proving
       the refresh closure regenerated).
     - Assert `refresher.current_token()`'s exposed secret equals the
       tail of `bearer_2` after `"Bearer "` (refresher state reflects
       the fresh token).

  Total wall-clock budget: ≈3s per CI leg (matching slice B's own
  wiremock acceptance test, which currently passes on all three legs
  in ci.yml:86). No `tokio::time::pause` or `tokio::time::advance` is
  used — those would fast-forward the refresher's `chrono::Utc::now()`
  check only if `chrono` were driven by the paused tokio clock, which
  it is not (`chrono::Utc::now()` reads system time), so real-time
  sleep is the only mechanism that advances `should_refresh()`.

  **Why the seam is required (mechanical argument).** Without it,
  `create_eks_client` hardcodes `expires_at = now + 60s`
  (aws_eks.rs:981) against `REFRESH_LEEWAY_SECS = 10` (aws_eks.rs:806):
  immediately after construction `should_refresh()` compares `now + 10s`
  against `now + 60s`, which is false, so `refresh()` would early-return
  at aws_eks.rs:861-863 without invoking the closure. Proving
  "regeneration past the 10-second leeway" would then require either
  (a) a ~51s wall-clock sleep (violating spec 06 0002-H4's "run within
  CI's existing runtime budget" across three CI legs — and step 4's
  "No wall-clock sleeps are needed" was written for the smoke file, not
  this test; step 6's 3s sleep here is intentional and disclosed) or
  (b) a `pub(crate)` `AuthRefreshLayer` construction test that
  side-steps `create_eks_client` (slice B's existing test at
  aws_eks.rs:1377 is exactly that shape and does NOT satisfy 4a for
  `create_eks_client` — the point round 1's BLOCKER surfaced). The
  seam is the minimum honest fix.

  **Wiremock scope note (spec 06 conditional).** This test uses
  wiremock — not `StaticReplayClient` — because the mock surface is
  the **K8s API server**, not an AWS SDK operation. Per spec 06 0002-H4:
  "prefer AWS smithy mock; fall back to `wiremock` only if the smithy
  mock cannot cover a given call." A smithy-native mock is scoped to
  aws-smithy-runtime HTTP clients (i.e. the AWS SDK's outbound calls);
  it cannot mock a `kube::Client`'s outbound HTTP to a K8s API server,
  since kube-rs uses `hyper_util` directly (not the smithy runtime).
  wiremock is therefore the sanctioned fallback for this specific call.

  **CA data note.** `rustls_https_connector()` parses the CA data at
  build time; the test's embedded self-signed PEM must parse (a well-
  formed cert is sufficient — validity of the signature chain is not
  checked at build time). If a viable const is unavailable, the
  implementer generates one at test time via `rustls`'s test helpers
  (already a workspace dep) without adding new deps. The test does
  **not** validate the mock server's TLS cert against the CA (the mock
  is HTTP; the test issues an `http://` request against the mock URI
  and slice B's pattern of `HttpConnector::enforce_http(false)` is
  reproduced by driving the `list()` call through the assembled
  service).

  **Alternative (fallback path if the TLS layer refuses HTTP against
  the mock):** if `create_eks_client`'s hard-wired
  `rustls_https_connector()` cannot be persuaded to speak to a plain
  wiremock, the implementer adds a **second minimal test-only seam**
  in addition to `create_eks_client_with_initial_ttl_secs`:
  `pub #[doc(hidden)] async fn create_eks_client_with_http_client(...)`
  that accepts a pre-built `hyper_util::client::legacy::Client`
  (bypasses `rustls_https_connector`) and is called only from this test.
  The public `create_eks_client` signature is unchanged (the two seams
  compose: the http-client seam takes both an initial TTL and a
  pre-built http client, and the public function passes 60s + a
  default TLS-backed client). This TLS-bypass seam is intentionally the
  *last resort*; the primary path is a direct call to
  `create_eks_client_with_initial_ttl_secs` against the wiremock URI,
  which slice B's own working pattern (aws_eks.rs:1421-1435) verifies
  is feasible against an HTTP mock endpoint under kube-client 0.98.0's
  `enforce_http(false)` + `https_or_http()` connector build
  (vendored `config_ext.rs:222/:238`, cross-checked by the round-2
  eval).

### 7. Cross-check documentation references

Grep for stale prose. Known claims to check:

- `.docs/spec/06-remediation-highs.md` — the 2026-08-28 clarification
  note under 0002-H2 is unchanged by this slice. The `create_eks_client`
  refactor named in the Affected-files list of 0002-H2 landed in slice
  B; Slice C does not touch it. No change.
- `.docs/spec/02-architecture.md` — describes the kube-rs client
  layer. Verify with `grep -n 'AWS\|SSO\|EKS' .docs/spec/02-
  architecture.md`. If matches describe the un-typed injection
  behaviour, spec 02 is stale — but spec 02 is Draft-status and
  frozen against slice-plan edits per the planner-role contract.
  The finalize pass records a spec 02 revision as pending, per the
  slice A precedent.
- `.docs/ADR/0002-kube-rs-client.md` — verify no consequence
  statement contradicts the new typed error surface at the
  injection boundary. If so, the slice is out of scope (spec 06
  Decisions clause: "If, during slice-planning, any of these
  fixes surfaces a genuine open decision …, the planner must pause
  and raise a new ADR before the slice-plan proceeds"). Expected:
  no contradiction, because the typed error is at the aws_sso.rs
  boundary — below the ADR 0002 kube-rs surface.

If step 7 discovers a spec or ADR that must be edited to make an
in-slice acceptance criterion checkable, that is a spec revision —
stop and raise `Needs Clarification` rather than editing the frozen
artifact from a slice-plan.

## Verification

### Local (pre-push)

Run each command from repo root on the slice branch. All must pass
before the branch is pushed for review:

1. **Format** — `cargo fmt --all -- --check`.
2. **Lint** — `RUST_MIN_STACK=268435456 cargo clippy --workspace
   --all-targets -- -D warnings`.
3. **Test** — `RUST_MIN_STACK=268435456 cargo nextest run --workspace`
   (matching CI's actual command at ci.yml:86; `cargo test --workspace`
   remains a locally acceptable substitute since wiremock's
   `MockServer` and StaticReplayClient tests are process-model-agnostic).
   The test count must **increase** by exactly:
   - 5 tests from step 1 in `aws_sso.rs`
     (`test_inject_profile_returns_exec_block_missing_when_absent`,
     `test_inject_credentials_returns_exec_block_missing_when_absent`,
     `test_inject_credentials_returns_context_not_found`,
     `test_inject_credentials_returns_auth_info_not_found`,
     `test_inject_credentials_sets_aws_env_vars_on_valid_context`) —
     the existing three green-path tests continue to pass (one is
     retitled but not deleted).
   - 8 tests from step 4 in `crates/baeus-core/tests/aws_wizard_smoke.rs`
     (one per operation listed in step 4, with pending+success folded
     into a single `sso_poll_for_token_returns_pending_then_success`
     per plan-eval MINOR #5).
   - 2 tests from step 6 in `aws_eks.rs`
     (`generate_eks_token_returns_prefixed_and_base64_url_encoded` and
     `create_eks_client_returns_bearer_token_and_refreshes_past_leeway`).
   - Total: **+15 tests**.
   Constitution invariant: "no decrease in test count" — this slice
   *adds* fifteen, satisfying the invariant strictly.
4. **Deny** — `cargo deny check`. Enabling
   `aws-smithy-http-client`'s `test-util` feature transitively pulls
   in `aws-smithy-protocol-test` (net-new to `Cargo.lock` — verified
   pre-slice via `grep aws-smithy-protocol-test Cargo.lock`). Same
   crate family (AWS Smithy), MIT/Apache-2.0 dual-licence expected;
   `cargo deny check` must still pass. If it doesn't, stop and raise
   `Needs Clarification` per spec 06 0002-H4's `[dev-dependencies]`
   budget clause.

### Remote (on the PR)

The CI matrix (macOS / Linux / Windows on the pinned toolchain from
slice A2) must pass all four gate steps on all three legs.

### Slice-specific acceptance (from spec 06, restated)

**0002-H3 acceptance:**

3a. Calling either inject function with a context lacking an exec
    block returns an explicit typed error, not `Ok(())`. **Proven
    by step 1's `test_inject_profile_returns_exec_block_missing_when_absent`
    and `test_inject_credentials_returns_exec_block_missing_when_absent`.**
3b. Existing callers that expect `Ok(())` on the happy path are
    unaffected. **Proven by (i) the retained green-path tests for
    `inject_aws_profile_into_kubeconfig`
    (`test_inject_aws_profile_into_kubeconfig`,
    `test_inject_aws_profile_overwrites_existing`) — both continue
    to compile and pass unchanged; and (ii) step 1's new green-path
    test for `inject_aws_credentials_into_kubeconfig`
    (`test_inject_credentials_sets_aws_env_vars_on_valid_context`),
    which covers the "valid exec-block kubeconfig" case spec 06
    0002-H3 test expectation (a) demands for "both inject functions"
    and asserts the rewritten `match`-based env-insertion body (aws_
    sso.rs:127-171 in its new form) preserves the pre-slice env-var
    mutation semantics.**
3c. Error message identifies the offending context name. **Proven
    by the Display impl on `KubeconfigInjectionError::ExecBlockMissing`
    (interpolates `{context}`) plus step 2's anyhow-context wrap
    at both call sites (also interpolates `{context_name}`); step
    5 confirms the message reaches the UI diagnostic surface via
    `format!("{e:#}")`.**

**0002-H4 acceptance:**

4a. At least one `#[tokio::test]` exists for every function listed
    in spec 06 0002-H4 (`sso_register_client`, `sso_start_device_auth`,
    `sso_poll_for_token`, `sso_list_accounts`, `sso_get_role_credentials`,
    `authenticate_with_access_key`, `assume_role`, `create_eks_client`,
    `generate_eks_token`). **Proven by the file layout: step 4's
    smoke file covers seven of the SSO / SSO-OIDC / STS operations
    (with pending+success folded into a single test that still
    proves `sso_poll_for_token`); step 6's two inline tests cover
    `generate_eks_token` and `create_eks_client`. The `create_eks_client`
    test invokes the function's body directly through the
    `create_eks_client_with_initial_ttl_secs` seam (the public
    `create_eks_client` is a one-line delegate to it, so covering
    the seam covers the entry point) and asserts on both the initial
    Bearer header (via the mock K8s API server) and a distinct
    refreshed Bearer after a deterministic 3-second wall-clock sleep
    that walks the `expires_at = t0 + 12s` refresher across the
    10-second leeway boundary (see step 6's timeline walk). Nine of
    nine, with `create_eks_client` proven by a test that literally
    exercises its body — not by inference from slice B's
    `AuthRefreshLayer` construction test at aws_eks.rs:1377.**
4b. The device-auth polling test asserts the retry-on-pending
    behaviour. **Proven by
    `sso_poll_for_token_returns_pending_then_success` — a single
    test replaying a 400 `authorization_pending` then a 200 success
    against the same client, asserting `SsoTokenResult::Pending`
    on the first call and `SsoTokenResult::Success` on the second
    (per plan-eval MINOR #5, this folded shape proves the
    pending→success retry sequence more directly than two isolated
    tests).**
4c. Tests run under the existing test gate without requiring live
    AWS credentials or network access. **Proven by
    `aws_smithy_http_client::test_util::StaticReplayClient` (in-process
    replay, no network) for all SDK-touching tests and wiremock
    (localhost-only mock server, no external routes) for step 6's
    K8s API server test; every test injects stubbed credentials.
    The CI gate at ci.yml:86 runs `cargo nextest run --workspace` —
    both StaticReplayClient and wiremock's `MockServer` are
    per-test-isolated (per-`SdkConfig`/per-`MockServer` instance),
    so they operate identically under nextest's per-process test
    model and cargo test's shared-process model. Step 6's `create_eks_client`
    test carries a disclosed 3-second `tokio::time::sleep` (mirroring
    slice B's existing wiremock acceptance test at aws_eks.rs:1458);
    the smoke file has no wall-clock sleeps.**

### Gate (spec 03 + slice-specific)

| Step   | Command                                                                          |
|--------|----------------------------------------------------------------------------------|
| format | `cargo fmt --all -- --check`                                                     |
| lint   | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` |
| test   | `RUST_MIN_STACK=268435456 cargo nextest run --workspace` (+15 tests)             |
| deny   | `cargo deny check`                                                               |

## Notes

- **2026-08-28 — Spec 06's `create_client_from_path_with_aws_creds`
  Affected-files entry (for 0002-H3) is accurate against the tree.**
  Unlike the same file's inaccurate 0002-H2 mention (documented in
  slice B's Notes and the spec's own 2026-08-28 clarification), the
  H3 reference is correct: `create_client_from_path_with_aws_creds`
  at client.rs:192 *does* call
  `inject_aws_credentials_into_kubeconfig` (client.rs:202) and
  therefore *does* need to propagate the typed error. No spec
  clarification is queued from Slice C.

- **2026-08-28 — Additional injection function in scope.** Spec 06
  0002-H3 fix-approach explicitly names both `inject_aws_profile_into_
  kubeconfig` and `inject_aws_credentials_into_kubeconfig` in its
  "both inject functions" phrasing. The first is called by
  `create_client_from_path` (client.rs:168) — used by the standard
  non-wizard EKS connect path (app_shell.rs:1848). Both call sites
  receive the anyhow-context wrap in step 2.

- **2026-08-28 — Smithy-native mock (`StaticReplayClient`) over
  wiremock for SDK-touching tests.** Spec 06 0002-H4's "prefer AWS
  smithy mock; fall back to `wiremock` only if the smithy mock cannot
  cover a given call" is a per-call, coverage-based condition — not
  a blanket authorization. This plan honours it by:
  (1) adopting `aws-smithy-http-client`'s `test-util` feature (stable,
  AWS-native, gives `StaticReplayClient` — the smithy-native mock spec
  06 prefers, at the non-deprecated path
  `aws_smithy_http_client::test_util::StaticReplayClient`) as the
  primary transport for the eight SDK-touching tests in
  `tests/aws_wizard_smoke.rs`. Enabling this feature transitively
  pulls in `aws-smithy-protocol-test` (net-new to `Cargo.lock`;
  disclosed in step 0 and gated by `cargo deny check`); the other
  transitively-pulled crates (`tracing-subscriber`, `http` 0.2,
  `hyper` 0.14) are already locked. (2) A per-call review (step 4
  test list) confirms StaticReplayClient covers every SDK operation
  the smoke file touches; **no wiremock fallback is invoked for any
  test in that file.** (3) wiremock is retained as the
  spec-sanctioned fallback for exactly one call the smithy mock
  demonstrably **cannot** cover: step 6's `create_eks_client` test,
  which needs to mock the **K8s API server** (a `hyper_util` HTTP
  target, not an aws-smithy-runtime target), so smithy-native mocks
  are out of scope by construction. This is the per-call fallback
  spec 06 permits. Slice B's inline `AuthRefreshLayer` test also
  continues to use wiremock (same rationale).

- **2026-08-28 — Why `aws-smithy-http-client` rather than
  `aws-smithy-runtime` for the mock crate.** In the locked
  aws-smithy-runtime 1.10.3 the `test_util` module at
  `aws_smithy_runtime::client::http::test_util` carries
  `#[deprecated = "… Please use the `test-util` feature from
  `aws-smithy-http-client` instead"]` over its re-export
  (vendored `src/client/http.rs:12-14`). Under
  `cargo clippy --workspace --all-targets -- -D warnings`
  (ci.yml:75) the deprecated path errors on lint. The non-deprecated
  canonical import for the locked tree is
  `use aws_smithy_http_client::test_util::StaticReplayClient;`
  (aws-smithy-http-client 1.1.12 vendored
  `src/test_util/replay.rs:154`), so the dev-dep goes on
  `aws-smithy-http-client` (already transitively locked at 1.1.12)
  with the `test-util` feature. Enabling that feature is net-new for
  `aws-smithy-protocol-test` — the disclosure in step 0's dep-tree
  delta and the `cargo deny check` gate cover the licence/advisory
  surface.

- **2026-08-28 — Two-seam `_with_config` / `_with_initial_ttl_secs`
  seam surface (`pub` + `#[doc(hidden)]`).**
  Spec 06 0002-H4 authorises "constructor-injection or a `SdkConfig`
  parameter" but does not prescribe visibility. Two seam families
  appear in this slice:
  (1) The step-3 `_with_config` inners for the eight SDK-touching
      functions in the smoke file. Because integration tests under
      `crates/baeus-core/tests/` are external to the crate they
      cannot see `pub(crate)` items — those inners must be `pub`
      (with `#[doc(hidden)]` and a doc-comment naming the
      test-injection contract) for the smoke tests to link.
  (2) The step-6 `create_eks_client_with_initial_ttl_secs` seam.
      The `create_eks_client` test lives inline in aws_eks.rs, so
      `pub(crate)` would technically suffice; the plan nevertheless
      keeps the same `pub` + `#[doc(hidden)]` shape to mirror the
      step-3 discipline and keep the whole test-injection surface
      consistent (one seam family for reviewers to audit, not two).
  Neither seam adds a caller surface consumers of `baeus-core` are
  expected to use (the public wrappers remain the sanctioned entry
  points). If a plan evaluator judges `pub` too broad on either
  seam, the alternative is to move the smoke tests inline under
  `#[cfg(test)] mod tests` in `aws_eks.rs` — but spec 06 0002-H4
  names `crates/baeus-core/tests/aws_wizard_smoke.rs` as a specific
  file target, and moving those tests inline would deviate from that
  prescription. The initial-TTL seam has no equivalent alternative:
  it is the only mechanically satisfiable path to a
  `create_eks_client` refresh assertion within CI's runtime budget
  (see step 6's mechanical argument).

- **2026-08-28 — No spec 06 amendment queued from Slice C.** Unlike
  slice B, which queued a 0002-H2 Affected-files clarification and a
  Slice-B-row amendment (see `.docs/slice-plans/archive/slice-b-
  watch-cancellation-eks-refresh.md#notes`), Slice C's scope is
  fully executable against spec 06 as written today (including the
  2026-08-28 clarification note). No dated amendment or clarification
  is queued by this slice-plan; finalize should update
  `.docs/status/` (roadmap / progress / handoff) to reflect Slice C
  landing but should not touch spec 06.
