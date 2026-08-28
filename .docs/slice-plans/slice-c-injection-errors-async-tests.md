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
- `create_eks_client` (aws_eks.rs:965 — already covered by slice B's
  `eks_refresh_layer_refreshes_token_and_updates_authorization_header`,
  the wiremock acceptance test at aws_eks.rs:1377)
- `generate_eks_token` (aws_eks.rs:691 — pure client-side signer;
  network-free)

Each function currently calls `aws_config::defaults(...).load().await`
inline (e.g. aws_eks.rs:235, :263, :299, :344, :383, :422, :480, :512,
:621) and constructs its SDK client from the loaded config. To route
these calls to a mock backend the plan adds an internal seam per
function that accepts an `aws_config::SdkConfig` (or a preconstructed
SDK client) so tests can inject an `.endpoint_url(mock_uri)` config
without duplicating production wiring.

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
  the inner form directly with a wiremock-endpoint-configured
  `SdkConfig`.
- Add `crates/baeus-core/tests/aws_wizard_smoke.rs` as the integration
  test file spec 06 0002-H4 names verbatim, covering at minimum:
  device-auth happy path, `AuthorizationPendingException` retry, token
  exchange, cluster discovery, `authenticate_with_access_key`,
  `assume_role`, and `sso_get_role_credentials`. `create_eks_client`
  already has the slice-B wiremock acceptance test in the same crate
  — Slice C does not re-cover it (spec 06's list is a *minimum*, not
  a duplication requirement).
- Add one inline `#[tokio::test]` in `aws_eks.rs` for `generate_eks_token`
  — a pure client-side signer whose test needs no SDK-mock surface and
  is thin enough to sit inline.
- **Reuse `wiremock` (already dev-dep from slice B) as the mock
  transport.** Spec 06 0002-H4 fix-approach text: "prefer AWS smithy
  mock; fall back to `wiremock` only if the smithy mock cannot cover
  a given call." The smithy-mocks-experimental crate is not currently
  a dep; adding it would churn the dep tree for surface parity with
  wiremock's request-observation model (which slice B has already
  vetted through the deny gate). Spec 06 explicitly authorises
  wiremock as fallback for the observation-surface it names — this
  slice takes that fallback for the whole wizard smoke file.

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
- **Replacing wiremock with `aws-smithy-mocks-experimental`** for the
  wizard smoke tests. Spec 06 explicitly authorises the wiremock
  fallback; this slice takes it. A future planning cycle may
  migrate.
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

### 0. `crates/baeus-core/Cargo.toml` — dev-dependency check

**No new dependencies.** `wiremock 0.6` is already a dev-dep from
slice B (Cargo.toml:49). Verify (`cargo tree -p baeus-core --edges
dev` or equivalent) that no additional dev-deps are needed for the
wizard smoke tests. If a test in step 4 uncovers a hard need (e.g.
`serde_json` used only under `#[cfg(test)]` — currently a
non-test dep, so this is a formality), add it here as a single
line with a version pin matching existing workspace policy.

`cargo deny check` must remain green — no crate additions in this
step means no fresh advisory / licence surface to review.

### 1. `crates/baeus-core/src/aws_sso.rs` — introduce `KubeconfigInjectionError` typed enum (test-first)

**Red tests first.** Extend the existing `#[cfg(test)] mod tests`
block with four new `#[test]` cases, all synchronous (no SDK
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

Create the file spec 06 0002-H4 names verbatim. Each test builds a
wiremock `MockServer` with fixture responses for the specific SDK
operation under test, constructs an `aws_config::SdkConfig` via
`aws_config::defaults(BehaviorVersion::latest())`
`.endpoint_url(mock_server.uri())`
`.region(Region::new("us-east-1"))`
`.credentials_provider(Credentials::new("AKIA…", "secret", None, None,
"test"))` (or `.no_credentials()` for the OIDC bootstrap flows),
`.load().await`, and passes that `SdkConfig` to the `_with_config`
inner. Fixture responses live inline in each `#[tokio::test]` as
`serde_json::json!({...})` values matching the SDK operation's wire
format (verified against the `aws-sdk-*` crate documentation for the
version pinned in `Cargo.lock`).

Test set (each is a single `#[tokio::test]` unless noted):

- **`sso_register_client_returns_client_id_and_secret`** — wiremock
  serves the `RegisterClient` OIDC response
  `{"clientId": "cid-1", "clientSecret": "csec-1", …}` on
  `POST /client/register`. Asserts the tuple returned matches.
- **`sso_start_device_auth_returns_device_and_user_codes`** —
  serves the `StartDeviceAuthorization` response with `deviceCode`,
  `userCode`, `verificationUri`, `verificationUriComplete`,
  `expiresIn: 600`, `interval: 5`. Asserts the parsed
  `SsoDeviceAuth` values (including `poll_interval` matching
  5 s and `expires_at` roughly `now + 600 s`).
- **`sso_poll_for_token_returns_pending_on_authorization_pending`** —
  serves `CreateToken` with a `400`-plus-error-body of
  `{"error": "authorization_pending", ...}`. Asserts the return
  is `SsoTokenResult::Pending` (the existing code path at
  aws_eks.rs:327 matches on `is_authorization_pending_exception`;
  wiremock's status + body faithfully reproduces the SDK's
  deserialised service error).
- **`sso_poll_for_token_returns_success_on_valid_grant`** — serves
  `CreateToken` with `{"accessToken": "at-1", "expiresIn": 3600,
  …}`. Asserts the return is `SsoTokenResult::Success` with the
  expected access token and a near-1-hour expiry.
- **`sso_list_accounts_paginates_and_returns_all`** — first
  `ListAccounts` response returns two accounts and a
  `nextToken`; second call (matched by presence of `next_token=`)
  returns two more accounts and no `nextToken`. Asserts four
  accounts in the returned vec.
- **`sso_get_role_credentials_returns_session`** — serves
  `GetRoleCredentials` with a `roleCredentials` object containing
  `accessKeyId`, `secretAccessKey`, `sessionToken`, `expiration`.
  Asserts the returned `AwsSession` carries those credentials and
  the region.
- **`authenticate_with_access_key_returns_session`** — wiremock
  routes STS `GetCallerIdentity` (`POST /` with
  `Action=GetCallerIdentity` form body) to a stub returning
  `<GetCallerIdentityResponse><…><Arn>…</Arn></…>`. Asserts the
  returned `AwsSession.account_id`, `.identity_arn` match.
- **`assume_role_returns_session_with_temporary_credentials`** —
  wiremock routes STS `AssumeRole` to a stub returning
  `<AssumeRoleResponse><…>` with an `AssumedRoleUser.Arn` and
  temporary `Credentials`. Asserts the returned `AwsSession`
  carries the expected ARN and expiry.
- **`discover_clusters_in_region_returns_described_clusters`** —
  first request `ListClusters` returns two names; two follow-up
  `DescribeCluster` requests each return an `EksCluster` payload.
  Asserts the returned `Vec<EksCluster>` has both, in the right
  region, with certificate-authority data present.
- **`generate_eks_token_returns_prefixed_and_base64_url_encoded`** —
  inline in `aws_eks.rs` (see step 6); this test *does not* need
  wiremock — the signer is client-side.

**Wiremock matchers.** Each mock uses
`wiremock::matchers::{method, path, header, body_string_contains}`
as needed to disambiguate multi-op flows (e.g. `ListAccounts`
paginated). For STS's form-encoded body, use
`body_string_contains("Action=GetCallerIdentity")` /
`body_string_contains("Action=AssumeRole")` — the SDK sends URL-encoded
form data for query-protocol operations.

**Region parameter.** All tests use `"us-east-1"`. `SdkConfig`'s
`.endpoint_url(mock_uri)` overrides the region-derived endpoint the
SDK would otherwise pick, so no per-service host mismatch matters.

**Concurrency and time.** No wall-clock sleeps are needed in this
file (unlike slice B's refresher test). Each test spins its own
`MockServer` (`MockServer::start().await`) — wiremock guarantees
per-test isolation.

**Error-typing acceptance:** spec 06 0002-H4 does not itself add a
"typed error" acceptance to this file — the acceptance is one
`#[tokio::test]` per function plus the `AuthorizationPendingException`
retry case. Test count for the file: 9 tests.

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

### 6. `crates/baeus-core/src/aws_eks.rs` — inline `generate_eks_token` sanity test

Add one `#[tokio::test]` inline (in the existing `#[cfg(test)] mod
tests` block) since the function is a pure client-side signer with
no SDK-mock surface:

- **`generate_eks_token_returns_prefixed_and_base64_url_encoded`** —
  construct `Credentials::new("AKIA…", "secret", None, None,
  "test-provider")`, call
  `generate_eks_token("my-cluster", &creds, "us-east-1").await?`,
  assert the returned string starts with `"k8s-aws-v1."` and that
  the tail (post-prefix) decodes as base64url to a
  `https://sts.us-east-1.amazonaws.com/?…` URL containing
  `Action=GetCallerIdentity`, `X-Amz-Signature=`, and the correct
  `X-Amz-Credential` prefix. This satisfies spec 06 0002-H4's
  "At least one `#[tokio::test]` exists for every function
  listed" without needing SDK mocking.

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
3. **Test** — `RUST_MIN_STACK=268435456 cargo test --workspace`. The
   test count must **increase** by exactly:
   - 4 tests from step 1 in `aws_sso.rs`
     (`test_inject_profile_returns_exec_block_missing_when_absent`,
     `test_inject_credentials_returns_exec_block_missing_when_absent`,
     `test_inject_credentials_returns_context_not_found`,
     `test_inject_credentials_returns_auth_info_not_found`) — the
     existing three green-path tests continue to pass (one is
     retitled but not deleted).
   - 9 tests from step 4 in `crates/baeus-core/tests/aws_wizard_smoke.rs`
     (one per operation listed in step 4).
   - 1 test from step 6 in `aws_eks.rs`
     (`generate_eks_token_returns_prefixed_and_base64_url_encoded`).
   - Total: **+14 tests**.
   Constitution invariant: "no decrease in test count" — this slice
   *adds* fourteen, satisfying the invariant strictly.
4. **Deny** — `cargo deny check` (green post-slice-A; no dep additions
   in this slice, so the surface is unchanged).

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
    unaffected. **Proven by the retained green-path tests
    (`test_inject_aws_profile_into_kubeconfig`,
    `test_inject_aws_profile_overwrites_existing`) — both continue
    to compile and pass unchanged.**
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
    smoke file covers seven; step 6's inline covers `generate_eks_token`;
    slice B's inline `eks_refresh_layer_refreshes_token_and_updates_authorization_header`
    covers `create_eks_client`. Nine of nine.**
4b. The device-auth polling test asserts the retry-on-pending
    behaviour. **Proven by
    `sso_poll_for_token_returns_pending_on_authorization_pending`.**
4c. Tests run under the existing test gate without requiring live
    AWS credentials or network access. **Proven by wiremock
    localhost-only mock servers and stubbed credentials in every
    smoke test.**

### Gate (spec 03 + slice-specific)

| Step   | Command                                                                          |
|--------|----------------------------------------------------------------------------------|
| format | `cargo fmt --all -- --check`                                                     |
| lint   | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` |
| test   | `RUST_MIN_STACK=268435456 cargo test --workspace` (+14 tests)                    |
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

- **2026-08-28 — Wiremock over smithy-mocks-experimental.** Spec 06
  0002-H4's "prefer AWS smithy mock; fall back to `wiremock`" gives
  the planner discretion to select. This plan selects wiremock for
  three reasons: (1) it is already vetted through the deny gate as
  a slice-B dev-dep; (2) `aws-smithy-mocks-experimental` (or the
  newer `aws-smithy-runtime`'s `test-util` feature with
  `StaticReplayClient`) is not currently on the tree — adding it
  is a fresh advisory/licence surface for `cargo deny check` and a
  larger dep-tree delta than this slice warrants; (3) slice B's
  precedent shows wiremock cleanly covers request-observation flows
  including request-body content-matching (the substitution for
  the STS presigned-URL observation). The evaluator may request a
  smithy-mocks migration as a revision if the wiremock harness
  proves insufficient for any specific SDK operation.

- **2026-08-28 — `_with_config` seam surface (`pub` + `#[doc(hidden)]`).**
  Spec 06 0002-H4 authorises "constructor-injection or a `SdkConfig`
  parameter" but does not prescribe visibility. Because integration
  tests under `crates/baeus-core/tests/` are external to the crate
  they cannot see `pub(crate)` items — the `_with_config` inners
  must be `pub` (with `#[doc(hidden)]` and a doc-comment noting the
  test-injection contract) for the smoke tests to link. This is the
  minimum accessible visibility, and it does not add a caller
  surface consumers of `baeus-core` are expected to use (the public
  wrappers remain the sanctioned entry points). If a plan evaluator
  judges `pub` too broad, the alternative is to move the smoke
  tests inline under `#[cfg(test)] mod tests` in `aws_eks.rs` — but
  spec 06 0002-H4 names `crates/baeus-core/tests/aws_wizard_smoke.rs`
  as a specific file target, and moving the tests inline would
  deviate from that prescription.

- **2026-08-28 — No spec 06 amendment queued from Slice C.** Unlike
  slice B, which queued a 0002-H2 Affected-files clarification and a
  Slice-B-row amendment (see `.docs/slice-plans/archive/slice-b-
  watch-cancellation-eks-refresh.md#notes`), Slice C's scope is
  fully executable against spec 06 as written today (including the
  2026-08-28 clarification note). No dated amendment or clarification
  is queued by this slice-plan; finalize should update
  `.docs/status/` (roadmap / progress / handoff) to reflect Slice C
  landing but should not touch spec 06.
