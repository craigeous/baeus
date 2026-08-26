# Slice B — Watch Cancellation + EKS Token Refresh

Status: Plan Review
Target specs: `.docs/spec/06-remediation-highs.md` (§ 0002-H1 "Cancellation
surface on `watch_events` / `watch_resources`", § 0002-H2 "EKS bearer token
60-second TTL with no refresh path"); gate defined by
`.docs/spec/03-toolchain-and-gate.md`; research authority
`.docs/research/0002-core-client-aws-review.md`.

## Context

Two structural gaps sit in `baeus-core` today, each named as a high-severity
finding in research note 0002 and prescribed by spec 06's Slice B row:

**Watch cancellation (0002-H1).** `client.rs:1138` (`watch_events`) and
`client.rs:1195` (`watch_resources`) are infinite `while let Some(...)`
loops over `kube_runtime::watcher(...).default_backoff()`. `default_backoff`
never yields `None` under normal operation; the outer task must be aborted
to stop the loop, and abort is not observable in `ResourceWatchBridge`
(`watch.rs`). Callers in `crates/baeus-ui/src/layout/app_shell.rs:2240` and
`app_shell.rs:2658` spawn the loops on the Tokio handle and rely on channel
receiver drop as an implicit shutdown signal — a fragile, non-deterministic
teardown. Disconnecting a cluster leaves the loops running until the
enclosing `TokioHandle` is dropped.

**EKS token refresh (0002-H2).** `aws_eks.rs:774` sets `X-Amz-Expires = "60"`
on the presigned STS URL; `aws_eks.rs:779` (`create_eks_client`) generates
one token at construction time and embeds it in a static `Kubeconfig`
(`aws_eks.rs:792-818`). The resulting `kube::Client` returns 401 on every
API call ≥60 s after construction. `ClusterConnection.token_expiry`
(`cluster.rs:39`) is present in the model but no code populates it and
nothing watches it for EKS clients built through this path. There are no
tests that observe what happens when the token lapses (research 0002 §2).

Spec 06 groups these two into **Slice B** because they share a crate
(`baeus-core`), share the file `client.rs`, and share the async-runtime
surface that later Slice C will exercise with wizard tests. Slice B →
Slice C ordering is explicit in spec 06's Slice Breakdown.

**In scope (exactly the two findings 0002-H1 and 0002-H2, and only their
prescribed surfaces):**

- **0002-H1** — cancellation-token parameter added to `watch_events` and
  `watch_resources`; `ResourceWatchBridge::register_watcher` stores the
  token per watcher entry so `stop_watching` / `stop_for_cluster` can cancel
  the loop deterministically. `tokio-util` added as a `baeus-core`
  dependency (spec 06 A2 explicitly directs this addition — currently
  transitive in `Cargo.lock`, not a direct dep).
- **0002-H2** — `create_eks_client` returns a `(kube::Client,
  EksTokenRefresher)` pair (or an equivalent typed handle carrying both);
  a tower-layer refresh mechanism regenerates the presigned STS token
  before expiry so long-running kube requests do not 401.
  `ClusterConnection.token_expiry` populated. Refresh closure captures
  `SdkConfig` + cluster ARN; produces `SecretString`, not `String`
  (spec 06 Invariants: "Any refresh callback for 0002-H2 handles
  `SecretString`, not `String`").

**Explicit non-goals (deferred per spec 06's Out-of-scope section):**

- `InformerManager` / `AbortHandle` state-machine hardening (medium
  finding 0002 §7) — spec 06 0002-H1 scope note calls this out by name.
  Slice B adds the cancellation *surface* only; state-machine consistency
  is a later cycle.
- `ResourceWatchBridge::register_watcher` orphan-on-duplicate-key cleanup
  (medium finding 0002 §8).
- Silent-no-op error surface for AWS credential injection (0002-H3) —
  belongs to Slice C.
- Async tests for the wider AWS wizard flow (0002-H4) — belongs to Slice
  C. Slice B adds the two tests spec 06 lists under 0002-H1 and 0002-H2
  test expectations, no more.
- Migrating `AccessKeyConfig.secret_access_key` / `.session_token` /
  `AwsSession.sso_access_token` to `SecretString` (medium finding 0002 §6).
- `get_caller_identity` shell-out replacement (medium finding 0002 §5).
- `fetch_dashboard_data` unbounded list requests (medium finding 0002 §9).
- Parallelising `describe_cluster` (medium finding 0002 §10).
- Any UI-crate edits beyond the mechanical call-site adjustments named in
  step 3 below (passing a token through from the two existing callers in
  `app_shell.rs`). No sub-struct refactor of AppShell — that is Slice G.

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order within the slice; tests land alongside the code they exercise per the
constitution's test-first non-negotiable.

Steps are ordered so that:
- Step 0 adds the dependency without which subsequent steps fail to
  compile.
- Steps 1–3 deliver the 0002-H1 cancellation surface, red tests first, then
  implementation, then caller wiring.
- Steps 4–7 deliver the 0002-H2 refresh mechanism, again red tests first.
- Step 8 is the documentation cross-check.

### 0. `crates/baeus-core/Cargo.toml` — add `tokio-util` dependency

Under `[dependencies]`, add:

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

Notes constraining this step:

- Spec 06 0002-H1 fix approach names the version pattern (`version = "*"`)
  and the `rt` feature. This plan pins to `0.7` (the current major-minor
  in `Cargo.lock` at line 7359-7362) because a floating `"*"` would defeat
  the deny-gate's transitive-version discipline slice A established. Spec
  06 permits a concrete version; `*` was illustrative.
- The `rt` feature is what spec 06 names. `CancellationToken` itself
  actually lives under the default `sync` feature (also enabled by
  default); `rt` is not required for cancellation but does not conflict.
  Slice-plan honours spec 06's `rt` selection to keep behaviour identical
  to what the spec author verified.
- Cargo.lock is regenerated automatically by cargo on next build; the
  slice does not manually edit Cargo.lock beyond what `cargo build`
  produces.
- Do not promote to `[workspace.dependencies]` — only `baeus-core` needs
  the dep in this slice. Promotion is a separate scope call and belongs
  to a future slice if a second crate needs it.

### 1. `crates/baeus-core/src/client.rs` — refactor `watch_events` and `watch_resources` into testable inner loops (test-first)

**Rationale for the refactor.** Adding a cancellation token to the public
signatures is not sufficient to test cancellation — the tests need to
inject a stream (kube's real watcher stream needs a live API server).
Extract each `while let Some(...)` body into an inner helper that takes an
already-constructed `Stream` and the cancellation token, then have the
public entry point construct the kube watcher and call the helper. This is
a bounded refactor: no observable behaviour change beyond the new
cancellation semantics.

**Red tests first.** Add two `#[tokio::test]` tests to `client.rs` (inline
under the existing `#[cfg(test)] mod tests`) or, if inline pushes the file
past the recursion-limit budget, a new `crates/baeus-core/tests/client_watch.rs`
integration file. Each test:

1. Constructs an empty `tokio_stream::wrappers::UnboundedReceiverStream`
   (or an equivalent `futures::stream::pending::<Result<...>>()` — an
   always-pending stream that satisfies the type shape but produces no
   items).
2. Wraps it in a `CancellationToken`.
3. Spawns the inner helper on `tokio::spawn`, obtaining a `JoinHandle`.
4. `token.cancel()` and asserts the `JoinHandle` resolves within a bounded
   `tokio::time::timeout(Duration::from_millis(100), handle)`.

Concrete test names:
- `watch_events_inner_stops_on_cancellation`
- `watch_resources_inner_stops_on_cancellation`

**Then extract the inner helpers.** For `watch_events`:

```rust
async fn watch_events_inner<S, F>(
    mut stream: Pin<&mut S>,
    token: CancellationToken,
    mut on_event: F,
) -> Result<()>
where
    S: Stream<Item = Result<WatcherEvent<Event>, watcher::Error>> + Send,
    F: FnMut(EventInfo) + Send,
{
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => return Ok(()),
            next = stream.try_next() => {
                match next.context("Event watcher stream error")? {
                    Some(event) => { /* existing match arms unchanged */ }
                    None => return Ok(()),
                }
            }
        }
    }
}
```

`biased;` in `tokio::select!` ensures the cancellation branch is polled
first each iteration — spec 06 acceptance ("cancelling the token stops the
watch loop within one poll cycle").

For `watch_resources`, mirror the same shape with the `DynamicObject`
stream item type.

**Update the public entry points.** Both `watch_events` and
`watch_resources` gain a `CancellationToken` parameter. Spec 06 permits
either "optional-not-required (default `None` → new token constructed
internally)" or an overload. This plan makes the parameter **required and
explicit** to avoid a footgun (a caller who passes `None` gets no
cancellation — a hidden regression from today's behaviour). Callers that
don't need cancellation pass `CancellationToken::new()` inline; the type
is cheap and clonable.

Signatures after step 1:

```rust
pub async fn watch_events<F>(
    client: &Client,
    token: CancellationToken,
    mut on_event: F,
) -> Result<()>
where
    F: FnMut(EventInfo) + Send,
{ /* body constructs stream, calls watch_events_inner */ }

pub async fn watch_resources<F>(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
    token: CancellationToken,
    mut on_change: F,
) -> Result<()>
where
    F: FnMut(Vec<serde_json::Value>) + Send,
{ /* body constructs stream, calls watch_resources_inner */ }
```

**Rationale for putting `token` before the callback.** Rust's `impl FnMut`
generic parameter can consume trailing arguments awkwardly at call sites;
putting the concrete `CancellationToken` earlier keeps the closure last,
which is idiomatic and matches the existing signature shape.

### 2. `crates/baeus-core/src/watch.rs` — store `CancellationToken` on watcher entries; wire `stop_watching` / `stop_for_cluster` to cancel

**Red tests first.** Add three `#[test]` tests (synchronous — no async
needed for the bridge itself) to the existing `#[cfg(test)] mod tests`:

- `test_register_watcher_returns_cancellation_token` — asserts the
  returned handle exposes a `CancellationToken` that is not-cancelled.
- `test_stop_watching_cancels_token` — registers, retrieves the token
  before stop, calls `stop_watching`, asserts `token.is_cancelled()`.
- `test_stop_for_cluster_cancels_all_tokens_for_cluster` — registers two
  kinds on the same cluster and one on a second cluster, calls
  `stop_for_cluster(cluster_a)`, asserts the two cluster_a tokens are
  cancelled and the cluster_b token is not.

**Then modify the bridge.** `ResourceWatchBridge` today stores
`watcher_ids: HashMap<(Uuid, String), Uuid>`. Extend it to also store a
per-watcher `CancellationToken`. Two clean shapes:

- **Shape A (preferred):** replace the value type with a struct:
  ```rust
  struct WatcherEntry {
      informer_id: Uuid,
      cancel: CancellationToken,
  }
  watcher_ids: HashMap<(Uuid, String), WatcherEntry>,
  ```
  `register_watcher` returns `(Uuid, CancellationToken)` — the informer
  id (backward-compatible) plus the token the caller passes into
  `watch_events` / `watch_resources`.
- **Shape B:** parallel `HashMap<(Uuid, String), CancellationToken>`.
  Rejected as noisier — two maps that must stay in sync.

Use Shape A. `register_watcher`'s new signature:

```rust
pub fn register_watcher(
    &mut self,
    cluster_id: Uuid,
    kind: &str,
    api_version: &str,
    namespace: Option<&str>,
) -> (Uuid, CancellationToken)
```

Callers today ignore the second tuple field via `let (id, _cancel) = ...`
where they don't need it. The two `app_shell.rs` callers (§ step 3) use
the token.

`stop_watching(&mut self, cluster_id: &Uuid, kind: &str)` — before
removing the entry, call `entry.cancel.cancel()`. `stop_for_cluster` (add
this method if it does not exist — verify against the current `watch.rs`;
spec 06 names it but the current file may not have it) iterates all
entries whose `cluster_id` matches, cancels each token, and removes the
entries.

**Existing test compatibility.** The current tests
(`test_register_watcher_returns_uuid`, etc.) call
`bridge.register_watcher(...)` and bind the result directly to an
`assert_ne!(id, Uuid::nil())`. The signature change makes those tests
fail to compile until they destructure the tuple:
`let (id, _) = bridge.register_watcher(...);`. Update every existing
inline test call site accordingly — 8 call sites, mechanical edit. Spec
06 acceptance: "public API of `watch_events` / `watch_resources` remains
backward compatible for callers that do not care about cancellation".
Note that the *bridge* API is internal to `baeus-core` — its callers are
inside the workspace, not third-party — so a small tuple-destructure
change at call sites is within the letter of that rule.

### 3. `crates/baeus-ui/src/layout/app_shell.rs` — thread the token through the two existing callers

Two call sites today, both established (line numbers 2240 and 2658 at
time of planning):

**`start_event_watcher`** (line 2197ff) currently spawns
`watch_events(&client, callback)` on the Tokio handle. Edit:

1. Before the `tokio_handle.spawn(async move { ... })` block, register
   the watcher via the bridge (or via a new `cancel` field on the
   informer state the AppShell keeps). Retrieve the `CancellationToken`.
2. Clone the token into the async closure.
3. Pass the token as the new second positional argument to
   `watch_events(&client, token, callback)`.
4. On the GPUI-thread side (the `cx.spawn` block), when
   `on_cluster_connection_lost` fires or when `stop_event_watcher` is
   called (introduce this method if it does not exist — one-liner
   calling `self.resource_watch_bridge.stop_for_cluster(cluster_id)`),
   the bridge cancels the token, the `tokio::select!` inside
   `watch_events_inner` returns `Ok(())` within one poll cycle, and the
   spawned Tokio task ends deterministically.

**`start_resource_watcher`** (line 2613ff) — identical shape, but for
`watch_resources`. Both callers already spawn the watch loop under
`tokio_handle.spawn`; this slice does not change *where* the loops run,
only *how* they stop.

**Do not** attempt to introduce a Drop-based cancellation on
`ResourceWatchBridge` in this slice. That would couple the bridge's
lifetime to the loops and cross the informer state-machine boundary spec
06 defers to a later cycle. Cancellation is caller-initiated in this
slice — the AppShell explicitly calls `stop_watching` / `stop_for_cluster`
on cluster disconnect.

### 4. `crates/baeus-core/src/aws_eks.rs` — introduce `EksTokenRefresher`; refactor `create_eks_client` to expose refresh (test-first)

**Red tests first.** Add three `#[tokio::test]` tests to
`aws_eks.rs`'s `#[cfg(test)] mod tests` (or a new
`crates/baeus-core/tests/aws_eks_refresh.rs` if inline hits the
recursion limit):

- `should_refresh_returns_true_when_within_ten_seconds_of_expiry` — the
  pure-function unit test spec 06 names verbatim under 0002-H2 test
  expectations. Passes a `now = Utc::now()` and an `expires_at = now +
  Duration::seconds(9)`; asserts true. Then `expires_at = now +
  Duration::seconds(11)`; asserts false. Then `expires_at = now -
  Duration::seconds(1)` (already expired); asserts true.
- `refresher_produces_fresh_token_on_call` — injects a test-only refresh
  closure (see next paragraph on injectability) that returns a monotonic
  counter as the token string, wraps it in `EksTokenRefresher`, calls
  `.refresh().await` twice, asserts the two returned `SecretString`
  values differ.
- `refresher_surfaces_typed_error_on_failure` — refresh closure returns
  `Err(...)`; `.refresh().await` returns `EksTokenRefreshError` (new
  error variant, see below), not an opaque `anyhow::Error`.

**Design of `EksTokenRefresher`.**

```rust
pub struct EksTokenRefresher {
    /// Current token, protected for concurrent readers.
    inner: Arc<RwLock<TokenState>>,
    /// Closure that regenerates the token from AWS credentials + cluster ARN.
    refresh_fn: Arc<dyn Fn() -> BoxFuture<'static, Result<TokenState, EksTokenRefreshError>>
        + Send + Sync>,
}

pub struct TokenState {
    pub token: SecretString,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum EksTokenRefreshError {
    #[error("Failed to regenerate EKS presigned token: {0}")]
    PresignFailed(String),
    #[error("AWS credentials expired and cannot be refreshed")]
    CredentialsExpired,
}

impl EksTokenRefresher {
    pub fn new(initial: TokenState, refresh_fn: /* Arc<dyn Fn ...> */) -> Self { ... }
    pub fn should_refresh(&self) -> bool { ... }
    pub async fn refresh(&self) -> Result<(), EksTokenRefreshError> { ... }
    pub fn current_token(&self) -> SecretString { ... }
    pub fn expires_at(&self) -> DateTime<Utc> { ... }
}
```

Notes constraining this design:

- `RwLock` (not `Mutex`) because reads (each API call reads the token)
  outnumber writes (refresh once per 60 s). `tokio::sync::RwLock`
  because `refresh` is `async`.
- `refresh_fn` is a boxed async closure. In production, it captures the
  cluster's `SdkConfig` + cluster ARN and calls
  `generate_eks_token(cluster_name, credentials, region)` — the existing
  function at `aws_eks.rs:696`. In tests, it captures a counter and
  returns a canned `TokenState`.
- `SecretString` per spec 06 Invariants: "Any refresh callback for
  0002-H2 handles `SecretString`, not `String`". Spec 06 also notes the
  crate already uses `SecretString` correctly at `aws_eks.rs:853` (was
  actually `:804` in current tree) — this slice keeps that pattern.
- The `10s` refresh threshold is spec 06's exact number
  ("`should_refresh` returns true when `Utc::now() + 10s >= expires_at`").
  Encode as a `const REFRESH_LEEWAY: Duration = Duration::seconds(10);`
  at module scope so tests and production share the constant.

**`create_eks_client` refactor.** Change the return type from
`Result<kube::Client>` to `Result<(kube::Client, EksTokenRefresher)>`.
The refresher wraps a closure that captures `SdkConfig` (or a clonable
`Credentials` provider) + cluster ARN + region so the closure can call
`generate_eks_token` on demand.

Inside `create_eks_client`, after generating the initial token, construct
the `EksTokenRefresher`:

```rust
let refresh_fn: Arc<dyn Fn() -> BoxFuture<...> + Send + Sync> = {
    let credentials = credentials.clone(); // Credentials is Clone (aws_credential_types::Credentials)
    let cluster_name = cluster.name.clone();
    let region = cluster.region.clone();
    Arc::new(move || {
        let credentials = credentials.clone();
        let cluster_name = cluster_name.clone();
        let region = region.clone();
        Box::pin(async move {
            let token = generate_eks_token(&cluster_name, &credentials, &region)
                .await
                .map_err(|e| EksTokenRefreshError::PresignFailed(e.to_string()))?;
            Ok(TokenState {
                token: SecretString::new(token.into()),
                expires_at: Utc::now() + Duration::seconds(60),
            })
        })
    })
};
```

**Wire the refresh into the `kube::Client`.** kube-rs 0.98 exposes
`kube::client::ConfigExt::base_uri_layer()` and
`ConfigExt::auth_layer()` — the latter returns an `Option<AuthLayer>`
built from `AuthInfo`. For a token that lives in a static `AuthInfo`,
`auth_layer()` does not refresh. This slice therefore builds a custom
tower layer, `AuthRefreshLayer`, that:

1. On every outbound request, reads `refresher.expires_at()`.
2. If `refresher.should_refresh()` returns true, awaits `refresher.refresh()`.
3. Reads `refresher.current_token()` (freshly refreshed or cached).
4. Sets the `Authorization: Bearer <token>` header on the outbound
   request.

Concretely:

```rust
use tower::{Layer, Service};

#[derive(Clone)]
struct AuthRefreshLayer {
    refresher: EksTokenRefresher, // Clone-able via internal Arcs
}

impl<S> Layer<S> for AuthRefreshLayer { /* wraps into AuthRefreshService */ }

#[derive(Clone)]
struct AuthRefreshService<S> {
    inner: S,
    refresher: EksTokenRefresher,
}

impl<S, ReqBody> Service<http::Request<ReqBody>> for AuthRefreshService<S>
where
    S: Service<http::Request<ReqBody>> + Clone + Send + 'static,
    ReqBody: Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = /* box + Send + Sync, per kube-rs conventions */;
    type Future = /* boxed */;
    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future { /* refresh-or-read + set header + forward */ }
    fn poll_ready(...) { self.inner.poll_ready(...) }
}
```

Assemble the client with the layer applied over kube-rs's default HTTP
service. The kube-rs 0.98 pattern for a custom-service kube::Client is:

```rust
let kube_config = kube::Config::from_custom_kubeconfig(kubeconfig, &Default::default()).await?;
let default_ns = kube_config.default_namespace.clone();
let https = kube_config.rustls_https_connector()?; // ConfigExt
let hyper_service = hyper_util::client::legacy::Client::builder(TokioExecutor::new())
    .build::<_, hyper::body::Incoming>(https);
let service = ServiceBuilder::new()
    .layer(kube_config.base_uri_layer())
    .layer(AuthRefreshLayer { refresher: refresher.clone() })
    .service(hyper_service);
let client = kube::Client::new(service, default_ns);
```

**Note on kube-rs surface.** `ConfigExt::rustls_https_connector()` and
`base_uri_layer()` are the documented extension surface. If the exact
method names have shifted in kube 0.98, the developer verifies via
`cargo doc --open -p kube` before wiring. Slice-plan does not
speculate about method renames — the developer confirms against the
real API and adjusts imports accordingly. The concrete APIs matter
because the acceptance criterion depends on the refresh middleware
actually intercepting requests; a mis-wire that puts the layer *after*
kube-rs's default auth handling leaves the stale static token in
control. Verify with a `curl`-style assertion in the second integration
test (§ step 6).

Do **not** strip the static token from the `Kubeconfig` even though
`AuthRefreshLayer` overrides it — leaving it means that if the layer is
ever removed or its ordering shifts, the client still authenticates with
an initial token (fail-safe fallback for the first 60 s). The layer's
`Authorization` header write must therefore *overwrite* any existing
header, not append.

### 5. `crates/baeus-core/src/cluster.rs` — populate `token_expiry`

`ClusterConnection.token_expiry: Option<DateTime<Utc>>` (line 39) exists
but nothing writes it today. On EKS connection establishment (step 6
below), the caller sets `token_expiry = Some(refresher.expires_at())`
after `create_eks_client` returns.

Add one small helper if the field is not already write-through:

```rust
impl ClusterConnection {
    pub fn set_token_expiry(&mut self, expires_at: DateTime<Utc>) {
        self.token_expiry = Some(expires_at);
    }
}
```

Add one unit test in the existing `#[cfg(test)] mod tests` (if the file
has one — verify; if not, no test is added, per constitution's exception
"pure workflow/documentation change" does not apply here, so a single
`test_set_token_expiry_populates_field` test is added inline).

The `EksTokenRefresher` handle itself is **not** stored on
`ClusterConnection` in this slice — the AppShell (or the EKS-connection
state) owns the refresher for the lifetime of the kube::Client. The
`token_expiry` field is for UI observability (status bar / cluster info
row) and diagnostic logging, per research 0002 §2's note that the field
"is present but nothing watches it".

### 6. `crates/baeus-core/src/client.rs` — thread the refresher through `create_client_from_path_with_aws_creds`

The EKS branch of `create_client_from_path_with_aws_creds` (line 199 per
spec 06 0002-H3 affected files — verify current line at planning time)
today calls `create_eks_client` and gets back a `kube::Client`. After
step 4, `create_eks_client` returns `(kube::Client, EksTokenRefresher)`;
this function must be updated to accept the new shape and propagate the
refresher to whichever caller wants observability.

Two return-shape options:

- **Shape A (preferred):** `create_client_from_path_with_aws_creds`
  returns `Result<(kube::Client, Option<EksTokenRefresher>)>`. Non-EKS
  branches return `Ok((client, None))`. Callers that only need the
  client destructure with `let (client, _) = ...`.
- **Shape B:** introduce a `ClientWithMeta` struct wrapping both.
  Cleaner long-term but adds a new type to the public surface for
  minimal gain in this slice.

Use Shape A. It preserves the existing `?` propagation shape and lets
callers ignore the refresher until they need it.

**Second integration test.** Add
`crates/baeus-core/tests/aws_eks_refresh_integration.rs` (new file). This
test does **not** hit the real STS endpoint. It:

1. Constructs an `EksTokenRefresher` directly (bypassing
   `create_eks_client`) with a test refresh closure that returns
   incrementing token strings and a `now + 5 s` expiry (well under the
   10 s leeway so the next call triggers refresh).
2. Constructs a fake tower service (`tower::service_fn(|req|
   async { ... })`) that records the `Authorization` header of each
   request and returns a stub response.
3. Wraps the fake service with `AuthRefreshLayer { refresher }`.
4. Sends two requests to the layered service (via `.oneshot()`).
5. Asserts the two recorded `Authorization` values differ (second
   request triggered a refresh; token changed).

This is the "second `presigned_token` request is issued when the client
makes a call after expiry" acceptance test spec 06 lists under 0002-H2 —
implemented as a pure-tower unit test rather than a live STS mock,
because live STS mocking requires either `aws-smithy-mocks-experimental`
(unstable API surface at time of planning) or `wiremock` (external HTTP
dep, larger surface). Spec 06 explicitly allows the "AWS SDK's smithy
mock transport (or wiremock as fallback)" — this plan proposes a third
option (tower-layer black-box test with an injected fake refresh
closure) because it isolates the acceptance criterion ("a second
presign is issued") from the AWS SDK's specific mock API, which is
still churning across `aws-smithy-*` crate versions. The result is a
faster, more stable test that verifies the same behaviour.

**If the plan evaluator judges the tower-layer test insufficient,** the
fallback is `wiremock` (already permitted by spec 06) — add
`wiremock = "0.6"` to `[dev-dependencies]` and stand up a mock STS
endpoint. Slice-plan defaults to the tower-layer approach; the evaluator
can request the switch as a revision.

### 7. `crates/baeus-ui/src/layout/app_shell.rs` — record refresher lifetime alongside the EKS client

This is the smallest of the UI edits — spec 06 explicitly bounds it
("no sub-struct refactor of AppShell — that is Slice G"). Two touchpoints:

1. Wherever the AppShell stores `active_clients: HashMap<String,
   kube::Client>` (grep for the field name — the AppShell today caches
   kube clients per context), extend it in a minimal way:
   - Preferred: add a parallel
     `eks_refreshers: HashMap<String, EksTokenRefresher>` map. On EKS
     cluster connect, insert the refresher; on cluster disconnect,
     remove and drop it (dropping releases the AWS credentials Arcs).
   - Rejected alternative: wrap the client + refresher in a struct and
     replace `active_clients`. That is a Slice G decomposition, not
     Slice B scope.
2. On successful EKS connect, call
   `cluster_connection.set_token_expiry(refresher.expires_at())` so the
   `ClusterConnection` model reflects the initial expiry.

No render code changes. No new UI-facing indicator in this slice — the
existing status bar reads `token_expiry` if that indicator was already
wired (spec-independent).

### 8. Cross-check documentation references

Grep for stale prose. Two known claims to check:

- `.docs/spec/03-toolchain-and-gate.md` — no watch or EKS refs.
  **No change.**
- `.docs/spec/02-architecture.md` — describes the kube-rs client
  layer. Verify with `grep -n 'watch\|EKS\|cancellation'
  .docs/spec/02-architecture.md`. If matches describe the current
  no-cancellation shape, spec 02 is stale — but spec 02 is Draft-status
  and frozen against slice-plan edits per the planner-role contract.
  The finalize pass records a spec 02 revision as pending, per the
  slice A precedent.
- `.docs/ADR/0002-kube-rs-client.md` — the ADR that constrains this
  surface. Verify no consequence statement contradicts the new
  cancellation surface. If so, the slice is out of scope (spec 06
  Decisions: "If, during slice-planning, any of these fixes surfaces a
  genuine open decision …, the planner must pause and raise a new ADR
  before the slice-plan proceeds"). Slice-plan reader: confirm before
  approving.

If step 8 discovers a spec or ADR that must be edited to make an
in-slice acceptance criterion checkable, that is a spec revision — stop
and raise `Needs Clarification` rather than editing the frozen artifact
from a slice-plan.

## Verification

### Local (pre-push)

Run each command from repo root on the slice branch. All must pass
before the branch is pushed for review:

1. **Format** — `cargo fmt --all -- --check`.
2. **Lint** — `RUST_MIN_STACK=268435456 cargo clippy --workspace
   --all-targets -- -D warnings`.
3. **Test** — `RUST_MIN_STACK=268435456 cargo test --workspace`. The
   test count must **increase** by exactly:
   - 2 tests from step 1 (`watch_events_inner_stops_on_cancellation`,
     `watch_resources_inner_stops_on_cancellation`)
   - 3 tests from step 2 (bridge cancellation tests)
   - 3 tests from step 4 (`should_refresh_*`,
     `refresher_produces_fresh_token_on_call`,
     `refresher_surfaces_typed_error_on_failure`)
   - 1 test from step 5 (`test_set_token_expiry_populates_field`)
   - 1 integration test from step 6
     (`aws_eks_refresh_integration.rs`, one `#[tokio::test]`)
   - Total: **+10 tests**.
   Constitution invariant: "no decrease in test count" — this slice
   *adds* ten, satisfying the invariant strictly.
4. **Deny** — `cargo deny check` (green post-slice-A).

### Remote (on the PR)

The CI matrix (macOS / Linux / Windows on the pinned toolchain — assumes
Slice A2 has landed; if not, this slice runs on floating `@stable` and
carries the drift risk it would otherwise inherit) must pass all four
gate steps on all three legs. No new workflow edits from Slice B.

### Slice-specific acceptance (from spec 06, restated)

**0002-H1 acceptance:**
1a. Cancelling the token stops the watch loop within one poll cycle
    (test asserts observable via `JoinHandle` completing within a
    100 ms `tokio::time::timeout`).
1b. `ResourceWatchBridge::stop_watching` cancels the associated loop
    (bridge test asserts `token.is_cancelled()` post-stop).
1c. Public API of `watch_events` / `watch_resources` accepts a
    `CancellationToken` parameter; existing tests updated to pass one.

**0002-H2 acceptance:**
2a. A `kube::Client` produced by `create_eks_client` succeeds on API
    calls made after simulated 60 s elapse. Verified via the
    tower-layer integration test in step 6: the layered service
    issues a second refresh-triggered token when
    `refresher.should_refresh()` returns true.
2b. Refresh failure surfaces as `EksTokenRefreshError`, not opaque
    `anyhow::Error` (test:
    `refresher_surfaces_typed_error_on_failure`).
2c. Fast-path: when `expires_at` is comfortably in the future, no
    refresh call is issued (verified by the tower-layer test's
    first-request assertion — first token is the initial, not a
    refreshed value).

### Gate (spec 03 + slice-specific)

| Step | Command |
|------|---------|
| format | `cargo fmt --all -- --check` |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` (+10 tests) |
| deny | `cargo deny check` |

## Notes

_None._ If a role has a clarifying question, add a dated entry here and
set the artifact status to `Needs Clarification` per the loom role
contract.
