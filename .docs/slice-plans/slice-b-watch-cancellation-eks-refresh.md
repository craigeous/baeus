# Slice B — Watch Cancellation + EKS Token Refresh

Status: In Progress
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
loops over `kube_runtime::watcher(...).default_backoff()`.
`default_backoff` never yields `None` under normal operation; the outer
task must be aborted to stop the loop, and abort is not observable in
either `ResourceWatchBridge` (`watch.rs`) or `InformerManager`
(`informer.rs`). The live UI code path spawns both loops on the Tokio
handle from `AppShell::start_event_watcher` (app_shell.rs:2197ff, invokes
`watch_events` at app_shell.rs:2240) and `AppShell::start_resource_watcher`
(app_shell.rs:2613ff, invokes `watch_resources` at app_shell.rs:2658) and
relies on channel receiver drop as an implicit shutdown signal — a fragile,
non-deterministic teardown. Disconnecting a cluster leaves the loops
running until the enclosing `TokioHandle` is dropped.

**EKS token refresh (0002-H2) — corrected mental model.** Two distinct EKS
client-construction surfaces exist in the tree, and only one has the
static-TTL defect:

- `create_eks_client` (aws_eks.rs:779) generates a presigned STS URL
  (aws_eks.rs:688 `generate_eks_token` → aws_eks.rs:698
  `build_eks_presigned_token`) with `X-Amz-Expires = "60"` (aws_eks.rs:732)
  and embeds it as a static `token` in a synthesised `Kubeconfig`
  (aws_eks.rs:792-818). The resulting `kube::Client` returns 401 on every
  API call ≥60 s after construction. **This function has zero callers in
  the workspace** (verified 2026-08-26: `grep -Rn 'create_eks_client'
  crates/` returns only the definition; every other match under `crates/`
  is a doc reference). It is currently dead code — but spec 06 H2's
  acceptance criterion is stated in terms of this function ("A
  `kube::Client` produced by `create_eks_client` succeeds on API calls
  made more than 60 seconds after construction"), so the fix target is
  `create_eks_client` regardless of its current caller count.
- `create_client_from_path_with_aws_creds` (client.rs:191-225) is the
  **live EKS wizard connect path**, called from app_shell.rs:1699 during
  the "connect via EKS wizard" flow (app_shell.rs:1643-1770). It reads a
  kubeconfig from disk (written by `generate_eks_kubeconfig_file_with_role`
  at app_shell.rs:12080-12157), injects the wizard's in-memory AWS
  credentials into the kubeconfig's `exec` env via
  `inject_aws_credentials_into_kubeconfig` (aws_sso.rs:106-174), and lets
  kube-rs build a `kube::Client` that shells out to `aws eks get-token`
  on every API call needing a bearer token (app_shell.rs:12126-12136 —
  `command: aws`, `args: - eks - get-token …`). The AWS CLI regenerates
  the presigned STS URL per invocation. **This path does not carry the
  60-second static-TTL defect** — the exec plugin is the refresh
  mechanism. `client.rs:191-225` contains no EKS branch and no call to
  `create_eks_client`.

Round-1 plan evaluation caught the prior draft asserting spec 06 H2 wiring
"through" `create_client_from_path_with_aws_creds`. Spec 06's Affected-files
list does name `client.rs — create_client_from_path_with_aws_creds` under
0002-H2 ("wires the refresher through when constructing EKS clients"), but
against the tree this function does not construct EKS clients — it wraps
kube-rs's default construction over an exec-plugin kubeconfig. The spec's
mental model is inaccurate on that specific point. This slice-plan proceeds
by targeting the fix at `create_eks_client` (the surface named in the
acceptance criterion) and **not** editing `create_client_from_path_with_aws_creds`.
A Notes item flags the discrepancy so a future planner can revisit whether
to (a) migrate the live path to use `create_eks_client` + refresher, or
(b) delete `create_eks_client` in favour of the exec-plugin path. Neither
migration is in scope here — this slice satisfies spec 06 H2's acceptance
criterion by making `create_eks_client` correct at its own boundary, in
line with the eval's guidance ("If the truth is that EKS client creation
itself must be restructured to make tokens refreshable, say so and plan
that restructure within spec 06's H2 scope").

Spec 06 groups these two findings into **Slice B** because they share a
crate (`baeus-core`) and share the async-runtime surface that later Slice
C will exercise with wizard tests. Slice B → Slice C ordering is explicit
in spec 06's Slice Breakdown.

**In scope (0002-H1 and 0002-H2 as their frozen-spec acceptance criteria
prescribe):**

- **0002-H1** — optional cancellation-token parameter on `watch_events`
  and `watch_resources` (spec 06 fix approach: "optional-not-required
  (default `None` -> new token constructed internally, or overload the
  entry points)"). `InformerManager` / `InformerEntry` extended so
  `stop_for_cluster` cancels the per-watcher tokens; `ResourceWatchBridge`
  gets the equivalent surface (spec 06 explicitly names it as an affected
  file). AppShell wires the cancellation tokens through
  `start_event_watcher` and `start_resource_watcher` so `InformerManager::
  stop_for_cluster(cluster_id)` — already called at app_shell.rs:2310 on
  disconnect via `stop_event_watcher` — cancels the live loops
  deterministically. Direct deps added: `tokio-util = { version = "0.7",
  features = ["rt", "sync"] }`.
- **0002-H2** — `create_eks_client` returns `(kube::Client,
  EksTokenRefresher)`; the client is assembled from custom
  `tower::Layer`/`Service` middleware (`AuthRefreshLayer`) over a
  `hyper_util`-based HTTP connector so bearer-token refresh happens
  before outbound requests. Refresh closure captures the AWS
  `Credentials` (which is `Clone` — aws_credential_types::Credentials)
  + cluster name + region and re-invokes `generate_eks_token`. Token
  material is `SecretString` per the spec 06 Invariants clause.
  Direct deps added: `tower = { version = "0.5", features = ["util"] }`,
  `hyper = "1"`, `hyper-util = { version = "0.1", features = ["client",
  "client-legacy", "http1", "http2", "tokio"] }`.

**Explicit non-goals (deferred per spec 06's Out-of-scope section):**

- `InformerManager` state-machine hardening — the medium finding 0002 §7
  ("Associating an `AbortHandle` with each `InformerEntry`"). Spec 06
  0002-H1 scope note calls this out by name. Extending `InformerEntry`
  with a `CancellationToken` is **allowed** by spec 06 0002-H1 Affected
  files: "`informer.rs` — only if `InformerEntry` needs to hold the token
  for stop-by-key routing". That is the exact case here. This is the
  cancellation *surface* — the state-machine consistency of Idle /
  Running / Reconnecting / Stopped / Error transitions remains a later
  cycle.
- `ResourceWatchBridge::register_watcher` orphan-on-duplicate-key cleanup
  (medium finding 0002 §8).
- Silent-no-op error surface for AWS credential injection (0002-H3) —
  belongs to Slice C.
- Async tests for the wider AWS wizard flow (0002-H4) — belongs to Slice
  C. Slice B adds only the tests spec 06 lists under 0002-H1 and 0002-H2.
- Migrating `AccessKeyConfig.secret_access_key` / `.session_token` /
  `AwsSession.sso_access_token` to `SecretString` (medium finding 0002 §6).
- `get_caller_identity` shell-out replacement (medium finding 0002 §5).
- `fetch_dashboard_data` unbounded list requests (medium finding 0002 §9).
- Parallelising `describe_cluster` (medium finding 0002 §10).
- **Migrating the live EKS path** (app_shell.rs:1699 →
  `create_client_from_path_with_aws_creds`) **to use** `create_eks_client`
  + `EksTokenRefresher`. The live path uses `aws eks get-token` exec-plugin
  refresh already; migrating it is a separate design decision that
  belongs to a future planning cycle. This slice does not touch
  app_shell.rs's EKS-connect path (app_shell.rs:1643-1770) or
  `create_client_from_path_with_aws_creds`, and does not add
  `eks_refreshers` state to `AppShell`.
- Deleting `create_eks_client` (the alternative to restructuring). It is
  named in spec 06 0002-H2's acceptance criterion so it must be made
  correct at its own boundary; whether it should also be *used* is out
  of scope.

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order within the slice; tests land alongside the code they exercise per the
constitution's test-first non-negotiable.

Steps are ordered so that:
- Step 0 adds the dependencies without which subsequent steps fail to
  compile.
- Steps 1–3 deliver the 0002-H1 cancellation surface, red tests first, then
  implementation, then caller wiring.
- Steps 4–6 deliver the 0002-H2 refresh mechanism, again red tests first.
- Step 7 is the documentation cross-check.

### 0. `crates/baeus-core/Cargo.toml` — add direct dependencies

Under `[dependencies]`, add:

```toml
tokio-util = { version = "0.7", features = ["rt", "sync"] }
tower = { version = "0.5", features = ["util"] }
hyper = "1"
hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1", "http2", "tokio"] }
```

Under `[dev-dependencies]`, add:

```toml
wiremock = "0.6"
```

Notes constraining this step (correcting the round-1 draft's false
"tokio-util is the only dep" claim):

- **`tokio-util`** — `CancellationToken` lives under the `sync` feature
  (enabled by default); the `rt` feature is spec 06's exact
  prescription. Both are listed explicitly rather than relying on
  defaults, so a future defaults change does not silently break the
  build. Cargo.lock currently carries `tokio-util 0.7.18` (transitive;
  Cargo.lock:7359-7362); the direct-dep add pins to the same
  major-minor. Do not promote to `[workspace.dependencies]` — only
  `baeus-core` needs it in this slice.
- **`tower`** — kube-rs 0.98 re-exports only `kube_client::{api, client,
  config, discovery, error}` and `kube_core` (verified against the
  vendored `kube/src/lib.rs`); it does not re-export `tower`, so the
  `Layer`/`Service` types in step 4 require a direct dep. The `util`
  feature pulls in `ServiceBuilder`, `ServiceExt::oneshot`, and
  `service_fn` — all used in step 4's implementation and step 6's test
  scaffolding. Cargo.lock currently carries `tower 0.5.3` (transitive);
  the direct-dep add pins to the same major-minor.
- **`hyper`** — the `hyper::body::Incoming` body type is the value type
  kube-rs's `Client::new` expects; step 4's layered service returns a
  `Response<Incoming>`. Cargo.lock carries `hyper 1.8.1` (also `hyper
  0.14.32` transitively for other deps — the direct dep pulls
  major-version `1`).
- **`hyper-util`** — provides the `hyper_util::client::legacy::Client`
  builder and `TokioExecutor` needed to construct the underlying HTTP
  service kube-rs's `Client` wraps. Cargo.lock carries `hyper-util
  0.1.20`.
- **`wiremock`** — dev-only. Spec 06 0002-H2 test expectation names "the
  AWS SDK's smithy mock transport or a wiremock STS to prove a second
  `presigned_token` request is issued when the client makes a call after
  expiry." Round-1 draft substituted a canned-closure tower test and
  argued equivalence; the evaluator flagged this as a frozen-spec
  deviation. This revision adopts wiremock, per spec 06's explicit
  fallback. Step 6 details the harness. Because `build_eks_presigned_token`
  is a **pure client-side URL signer** (no network I/O — see aws_eks.rs:
  698-772), wiremock cannot observe STS requests directly; wiremock is
  therefore used as the **Kubernetes API server** the middleware calls
  out to, and the "second presign was issued" observation surface is the
  Authorization header on the outbound request. Step 6 justifies this
  substitution against spec 06's stated observation shape ("a second
  `presigned_token` request is issued") — a differing Authorization
  header is a directly falsifiable observation of the same underlying
  event.
- Cargo.lock is regenerated automatically by `cargo build`; the slice
  does not manually edit Cargo.lock beyond what cargo produces. Deny gate
  (spec 06 A slice's `cargo deny check`) must remain green — none of the
  four new deps are in `deny.toml`'s deny list.

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
past the recursion-limit budget, a new
`crates/baeus-core/tests/client_watch.rs` integration file. Each test:

1. Constructs an always-pending stream (`futures::stream::pending::<Result<
   WatcherEvent<Event>, watcher::Error>>()` for the events variant, or
   `futures::stream::pending::<Result<WatcherEvent<DynamicObject>, ...>>()`
   for the resources variant). Always-pending stream produces no items and
   never terminates, satisfying the type shape.
2. Wraps it in a `CancellationToken` from `tokio_util::sync`.
3. Spawns the inner helper on `tokio::spawn`, obtaining a `JoinHandle`.
   Because the inner helper takes the stream **by value** (see next
   paragraph), the spawned future is `'static` and spawning succeeds.
4. `token.cancel()` and asserts the `JoinHandle` resolves within a bounded
   `tokio::time::timeout(Duration::from_millis(100), handle)`.

Concrete test names:
- `watch_events_inner_stops_on_cancellation`
- `watch_resources_inner_stops_on_cancellation`

**Then extract the inner helpers.** The round-1 draft sketched `mut
stream: Pin<&mut S>` — this cannot be spawned via `tokio::spawn` because
`&mut S` borrows a non-`'static` local. Take `S` by value and `tokio::pin!`
inside the helper, mirroring the current `watch_events` body's existing
`tokio::pin!(stream)`:

```rust
async fn watch_events_inner<S, F>(
    stream: S,
    token: CancellationToken,
    mut on_event: F,
) -> Result<()>
where
    S: Stream<Item = Result<WatcherEvent<Event>, watcher::Error>> + Send + 'static,
    F: FnMut(EventInfo) + Send,
{
    tokio::pin!(stream);
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

**Update the public entry points to optional-cancellation shape (spec 06
conformance).** Spec 06 0002-H1 fix approach requires "optional-not-required
(default `None` -> new token constructed internally, or overload the entry
points)". The round-1 draft made the parameter *required*; that was flagged
as a deviation from a frozen spec acceptance criterion. This revision uses
an `Option<CancellationToken>` parameter with an internal
`unwrap_or_else(CancellationToken::new)`:

```rust
pub async fn watch_events<F>(
    client: &Client,
    cancel: Option<CancellationToken>,
    mut on_event: F,
) -> Result<()>
where
    F: FnMut(EventInfo) + Send,
{
    let token = cancel.unwrap_or_else(CancellationToken::new);
    let events_api: Api<Event> = Api::all(client.clone());
    let watch_config = watcher::Config::default();
    let stream = kube_runtime::watcher(events_api, watch_config).default_backoff();
    watch_events_inner(stream, token, on_event).await
}

pub async fn watch_resources<F>(
    client: &Client,
    kind: &str,
    namespace: Option<&str>,
    cancel: Option<CancellationToken>,
    mut on_change: F,
) -> Result<()>
where
    F: FnMut(Vec<serde_json::Value>) + Send,
{
    /* body constructs stream as before, then calls watch_resources_inner */
}
```

Callers that don't need cancellation pass `None`; callers that do
construct a `CancellationToken` (or receive one from the bridge/manager
per step 2) and pass `Some(token)`. Existing call sites in AppShell (§ step
3) migrate from the current 3-arg shape to the 4-arg shape passing
`Some(token)` — a mechanical edit at two call sites (app_shell.rs:2240,
:2658). No callers outside `baeus-core` currently pass a token, so the
optional-parameter form is backward-compatible for any consumer that
migrates by inserting `None`.

**Rationale for putting `cancel` before the callback.** Rust's `impl
FnMut` generic parameter can consume trailing arguments awkwardly at call
sites; putting the concrete `Option<CancellationToken>` earlier keeps the
closure last, which is idiomatic and matches the existing signature shape.

### 2. Cancellation token storage — `InformerEntry`, `InformerManager`, and `ResourceWatchBridge`

The eval identified that `AppShell` uses `informer_manager: InformerManager`
directly (app_shell.rs:445), and `ResourceWatchBridge` has zero non-test
callers workspace-wide (verified 2026-08-26: `grep -Rn 'ResourceWatchBridge\|
resource_watch_bridge'` under `crates/` returns matches only in
`baeus-core/src/watch.rs` — the file's own definition and its inline tests
at :102, :103, :127, :610). Spec 06 0002-H1 names `watch.rs` as an
affected file, so the bridge still receives its cancellation surface
(preserving spec-consistency and allowing future UI wiring), but the
**live cancellation path** goes through the manager because that is where
the UI already routes.

**Red tests first.** Add these `#[test]` tests (synchronous — no async
needed for the container types themselves):

*In `crates/baeus-core/src/informer.rs`* (inline under existing `#[cfg(test)]
mod tests`):

- `test_set_cancel_token_stores_on_entry` — registers an informer, calls
  the new `set_cancel_token`, asserts subsequent read returns the same
  token (not-cancelled).
- `test_stop_for_cluster_cancels_all_tokens_for_cluster` — registers
  standard watchers on two clusters, calls `set_cancel_token` on each
  entry, calls `stop_for_cluster(cluster_a)`, asserts every cluster_a
  token is cancelled and every cluster_b token is not.

*In `crates/baeus-core/src/watch.rs`* (inline under existing `#[cfg(test)]
mod tests`):

- `test_register_watcher_returns_cancellation_token` — new return shape:
  asserts the returned tuple's second element is a fresh, not-cancelled
  `CancellationToken`.
- `test_stop_watching_cancels_token` — registers, retrieves the token
  from the return, calls `stop_watching`, asserts `token.is_cancelled()`.
- `test_stop_for_cluster_cancels_all_tokens_for_cluster` (bridge variant)
  — the bridge gains a `stop_for_cluster` method mirroring the manager's;
  registers watchers on two clusters, asserts the correct set of tokens
  is cancelled.

**Then modify `InformerEntry` and `InformerManager`.**

```rust
// informer.rs
use tokio_util::sync::CancellationToken;

struct InformerEntry {
    config: InformerConfig,
    state: InformerState,
    cancel: Option<CancellationToken>,
}
```

`InformerManager::register` initialises `cancel: None` (informer is
registered but no watcher task exists yet). Add:

```rust
impl InformerManager {
    /// Attach a cancellation token to a registered informer. Called by the
    /// UI when it spawns a watcher task for the corresponding entry.
    pub fn set_cancel_token(&mut self, id: &Uuid, token: CancellationToken) -> bool {
        if let Some(entry) = self.informers.get_mut(id) {
            entry.cancel = Some(token);
            true
        } else {
            false
        }
    }

    /// Read the token for a registered informer (for callers that need to
    /// clone it into a spawned task).
    pub fn cancel_token(&self, id: &Uuid) -> Option<&CancellationToken> {
        self.informers.get(id).and_then(|e| e.cancel.as_ref())
    }
}
```

Modify the existing `stop_for_cluster` (informer.rs:155) so that, in
addition to the existing state-transition and cache-clearing, it cancels
every attached token:

```rust
pub fn stop_for_cluster(&mut self, cluster_id: &Uuid) {
    let ids: Vec<Uuid> = self.informers_for_cluster(cluster_id);
    for id in &ids {
        if let Some(entry) = self.informers.get(id) {
            if let Some(token) = &entry.cancel {
                token.cancel();
            }
        }
        self.set_state(id, InformerState::Stopped);
    }
    self.clear_cache_for_cluster(cluster_id);
}
```

Also augment `unregister` (informer.rs:68) to cancel the token before
removing the entry, so stop-by-key routing via the bridge propagates.

**Then modify `ResourceWatchBridge`.** Replace the current `watcher_ids:
HashMap<(Uuid, String), Uuid>` with a struct-valued map so both the informer
id and the token are stored together (spec 06 fix approach: "stores the
token alongside each watcher entry"). The existing map key stays `(Uuid,
String)`:

```rust
struct WatcherEntry {
    informer_id: Uuid,
    cancel: CancellationToken,
}

pub struct ResourceWatchBridge {
    informer_manager: InformerManager,
    watcher_ids: HashMap<(Uuid, String), WatcherEntry>,
}

impl ResourceWatchBridge {
    pub fn register_watcher(
        &mut self,
        cluster_id: Uuid,
        kind: &str,
        api_version: &str,
        namespace: Option<&str>,
    ) -> (Uuid, CancellationToken) {
        /* existing body constructs config, calls informer_manager.register(...) */
        let id = self.informer_manager.register(config);
        self.informer_manager.set_state(&id, InformerState::Running);
        let cancel = CancellationToken::new();
        self.informer_manager.set_cancel_token(&id, cancel.clone());
        self.watcher_ids.insert(
            (cluster_id, kind.to_string()),
            WatcherEntry { informer_id: id, cancel: cancel.clone() },
        );
        (id, cancel)
    }

    pub fn stop_watching(&mut self, cluster_id: &Uuid, kind: &str) {
        let key = (*cluster_id, kind.to_string());
        if let Some(entry) = self.watcher_ids.remove(&key) {
            entry.cancel.cancel();
            self.informer_manager.set_state(&entry.informer_id, InformerState::Stopped);
            self.informer_manager.unregister(&entry.informer_id);
        }
        self.informer_manager.update_cache(*cluster_id, kind, Vec::new());
    }

    /// New: matches the manager's `stop_for_cluster` on the bridge surface.
    /// Iterates entries whose cluster_id matches, cancels each token,
    /// removes bridge state, then defers to the manager for cache and
    /// informer-state cleanup.
    pub fn stop_for_cluster(&mut self, cluster_id: &Uuid) {
        let keys: Vec<_> = self
            .watcher_ids
            .keys()
            .filter(|(cid, _)| cid == cluster_id)
            .cloned()
            .collect();
        for key in keys {
            if let Some(entry) = self.watcher_ids.remove(&key) {
                entry.cancel.cancel();
            }
        }
        self.informer_manager.stop_for_cluster(cluster_id);
    }
}
```

**Existing bridge-test compatibility.** The current test module contains
**39** call sites of `bridge.register_watcher(...)` (verified 2026-08-26:
`grep -c 'bridge.register_watcher(' crates/baeus-core/src/watch.rs` = 39
— the round-1 draft's "8 call sites" figure was wrong). Each site binds
the return either directly (`let id = bridge.register_watcher(...)`), by
statement-expression (no binding), or via `assert_ne!(id, Uuid::nil())`.
The tuple return breaks direct-binding sites; the mechanical fix is one
of:

- Unused-token sites: `let (id, _) = bridge.register_watcher(...);` or
  simply drop the `let` if the return is unused.
- Sites that need the id: `let (id, _cancel) = bridge.register_watcher(
  ...);`.
- Sites currently written as an expression statement (e.g.
  `bridge.register_watcher(cluster, "Pod", "v1", None);` on
  lines 149, 228, 247–248, 275–276, etc.): unchanged in shape — the
  tuple return is dropped in expression position.

All 39 sites are inside `#[cfg(test)]` so this is a mechanical test-only
edit. Spec 06 0002-H1 acceptance "backward compatible for callers that
do not care about cancellation" refers to `watch_events` / `watch_resources`
public API; the *bridge* API is internal to `baeus-core` and its callers
are workspace-internal (currently zero non-test), so a tuple-return
adjustment is within the spec's letter.

### 3. `crates/baeus-ui/src/layout/app_shell.rs` — thread cancellation tokens through the two live watchers

Two live call sites, both established in the tree (line numbers verified
2026-08-26):

**`start_event_watcher`** (app_shell.rs:2197ff, watch_events call at
app_shell.rs:2240). The function today:

1. Resolves the cluster id from `context_name`.
2. Reads the cached kube::Client.
3. Registers standard informers via `self.informer_manager.
   register_standard_watchers(cluster_id)` (app_shell.rs:2221), returning
   a `Vec<Uuid>` of informer ids for Namespace / Node / Pod / Event.
4. Marks each as Running (app_shell.rs:2224-2226).
5. Spawns a Tokio task calling `watch_events(&client, callback)`
   (app_shell.rs:2239-2264).

Edit:

- Immediately after step 4, create a fresh `CancellationToken`
  (`tokio_util::sync::CancellationToken::new()`) and register it on each
  of the four informer ids via `self.informer_manager.
  set_cancel_token(id, token.clone())`. This ensures the manager's
  existing `stop_for_cluster` call (at app_shell.rs:2310 inside
  `stop_event_watcher`) cancels the loop.
- Clone the token once more into the Tokio spawn closure.
- Change the `watch_events(&client, callback)` call to
  `watch_events(&client, Some(token), callback)`.

Result: `watch_events` breaks its `tokio::select!` on `token.cancelled()`,
returns `Ok(())` within one poll cycle, and the spawned task ends
deterministically when the UI thread invokes `stop_event_watcher` on
disconnect.

**`start_resource_watcher`** (app_shell.rs:2613ff, `watch_resources` call
at app_shell.rs:2658). Different shape from `start_event_watcher`: this
function does **not** call `register_standard_watchers`; it manages
per-`ResourceListKey` watchers via `self.active_resource_watchers: HashSet<
ResourceListKey>` (see app_shell.rs:2627, :2643, :2718). To attach a
cancellation token to these ad-hoc watchers without introducing a new
sub-struct (spec 06 explicitly bounds UI edits to the mechanical
call-site adjustments), the plan uses a small parallel map:

- Add a new field to `AppShell`: `resource_watch_cancels: HashMap<
  ResourceListKey, CancellationToken>` (single map, no wrapper struct).
  Spec 06 0002-H1's Affected-files list names `watch.rs` and — via the
  0002-H2 note — the wider `baeus-core` surface, and does not bound
  Slice B's UI edits (the "call-site adjustments" phrase in spec 06
  line 595 belongs to 0005-H5, not to Slice B). The parallel-map
  approach is chosen here because it keeps AppShell's per-resource
  watcher state in exactly the same idiom (`HashSet` +
  `HashMap` on `ResourceListKey`) that already exists at
  `app_shell.rs:2627, :2643, :2718`, avoiding an ad-hoc sub-struct
  in this slice; the new field is one added field, well under spec 06
  0003-H1's future 25-field target. Wire it into `AppShell::new` (line
  ~708 area) with `HashMap::new()`.
- In `start_resource_watcher`, after the duplicate-watcher guard (app_shell.
  rs:2627) and before spawning the Tokio task, create a `CancellationToken`,
  clone it into the spawn closure, and insert it into
  `resource_watch_cancels` under `key.clone()`.
- Change the `watch_resources(...)` call at app_shell.rs:2658 to pass
  `Some(token)` in the new fourth positional slot.
- On the cleanup path (app_shell.rs:2716-2719, where the watcher ends and
  the entry is removed from `active_resource_watchers`), also remove the
  entry from `resource_watch_cancels`.
- In `stop_event_watcher` (app_shell.rs:2304-2312), after the existing
  `informer_manager.stop_for_cluster(&cluster_id)` call, iterate
  `resource_watch_cancels` and cancel + remove every entry whose
  `ResourceListKey.cluster_context == context_name`. This is the
  cluster-disconnect hook that stops resource watchers deterministically.

**Do not** introduce a Drop-based cancellation. That would couple lifetimes
across the informer state-machine boundary spec 06 defers to a later cycle.
Cancellation stays caller-initiated: AppShell explicitly cancels tokens on
`stop_event_watcher` invocation, which is already called by every
`on_cluster_connection_lost` and cluster-disconnect path today.

### 4. `crates/baeus-core/src/aws_eks.rs` — introduce `EksTokenRefresher`; refactor `create_eks_client` to return the refresher (test-first)

**Red tests first.** Add three `#[tokio::test]` tests to `aws_eks.rs`'s
`#[cfg(test)] mod tests` (or a new `crates/baeus-core/tests/aws_eks_refresh.rs`
if inline hits any budget constraint):

- `should_refresh_returns_true_when_within_ten_seconds_of_expiry` — the
  pure-function unit test spec 06 names verbatim under 0002-H2 test
  expectations. Constructs an `EksTokenRefresher` with a canned initial
  `TokenState { expires_at = now + 9s, ... }`; asserts `should_refresh()`
  returns true. Then with `expires_at = now + 11s`; asserts false. Then
  with `expires_at = now - 1s` (already expired); asserts true.
- `refresher_produces_fresh_token_on_call` — injects a test-only refresh
  closure that returns a monotonic counter as the token string, wraps in
  `EksTokenRefresher`, calls `.refresh().await` twice, asserts
  `.current_token()` returns two different `SecretString` values across
  the calls. **Precondition on the closure's returned `expires_at`.** The
  in-flight guard in `refresh()` re-checks `should_refresh()` after
  acquiring the mutex; if the closure's returned `expires_at` is
  outside the 10-second leeway, the second `.refresh().await` short-
  circuits and observes the first-refresh token, and the two-different-
  tokens assertion fails. The test therefore constructs the closure
  to return `expires_at = Utc::now() + seconds(5)` (well inside the
  leeway) so both refreshes are seen through. The initial `TokenState`
  passed to `EksTokenRefresher::new` uses the same +5 s expiry so the
  first `.refresh()` call also has `should_refresh() == true`.
- `refresher_surfaces_typed_error_on_failure` — refresh closure returns
  `Err(EksTokenRefreshError::PresignFailed(...))`; asserts `.refresh().
  await` returns the typed error variant, not an opaque `anyhow::Error`.

**Design of `EksTokenRefresher` — corrected lock model.** The round-1
draft sketched `Arc<tokio::sync::RwLock<TokenState>>` with `should_refresh(
&self) -> bool` and `current_token(&self) -> SecretString` — synchronous
getters that cannot read a `tokio::sync::RwLock` without `.await`. This
revision uses `std::sync::RwLock` for the token state (critical sections
are microseconds — a clone of a `SecretString` and a `DateTime<Utc>` copy)
and a `tokio::sync::Mutex<()>` guard to serialise concurrent refreshes so
two simultaneous `.refresh()` calls do not double-presign:

```rust
use std::sync::{Arc, RwLock};
use std::pin::Pin;
use std::future::Future;
use secrecy::SecretString;

pub struct TokenState {
    pub token: SecretString,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

type RefreshFuture = Pin<Box<dyn Future<Output = Result<TokenState, EksTokenRefreshError>> + Send>>;
type RefreshFn = Arc<dyn Fn() -> RefreshFuture + Send + Sync>;

pub struct EksTokenRefresher {
    inner: Arc<RwLock<TokenState>>,
    refresh_fn: RefreshFn,
    in_flight: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, thiserror::Error)]
pub enum EksTokenRefreshError {
    #[error("Failed to regenerate EKS presigned token: {0}")]
    PresignFailed(String),
    #[error("Refresh state lock poisoned")]
    LockPoisoned,
}

const REFRESH_LEEWAY: chrono::Duration = chrono::Duration::seconds(10);

impl EksTokenRefresher {
    pub fn new(initial: TokenState, refresh_fn: RefreshFn) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            refresh_fn,
            in_flight: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Sync — reads a short-lived std::sync::RwLock.
    pub fn should_refresh(&self) -> bool {
        let state = self.inner.read().expect("lock poisoned");
        chrono::Utc::now() + REFRESH_LEEWAY >= state.expires_at
    }

    /// Sync — clones out the current token (SecretString clone is cheap).
    pub fn current_token(&self) -> SecretString {
        let state = self.inner.read().expect("lock poisoned");
        state.token.clone()
    }

    pub fn expires_at(&self) -> chrono::DateTime<chrono::Utc> {
        let state = self.inner.read().expect("lock poisoned");
        state.expires_at
    }

    /// Async — serialised via `in_flight`, so overlapping callers wait on
    /// the same refresh rather than each issuing a fresh presign.
    pub async fn refresh(&self) -> Result<(), EksTokenRefreshError> {
        let _guard = self.in_flight.lock().await;
        // Re-check after acquiring the guard — a previous holder may have
        // just refreshed. `should_refresh` is fast (single read + timestamp).
        if !self.should_refresh() {
            return Ok(());
        }
        let new_state = (self.refresh_fn)().await?;
        let mut w = self.inner.write().map_err(|_| EksTokenRefreshError::LockPoisoned)?;
        *w = new_state;
        Ok(())
    }
}
```

Notes constraining this design:

- Critical sections in `std::sync::RwLock` are strictly `Clone` +
  timestamp compare / assign — no `.await` inside the lock, so no
  blocking-in-async-context risk. Poisoned-lock semantics are split
  by call site rather than uniformly typed: the two request-path
  getters (`should_refresh`, `current_token`) treat poisoning as an
  unrecoverable invariant break and `.expect("lock poisoned")` —
  standard Rust `RwLock` practice, and appropriate given that a
  poisoned lock here means a prior `refresh()` writer panicked and
  no forward progress is possible on the token state. The async
  `refresh()` method itself, which *is* the writer, maps its own
  write-guard `PoisonError` to `EksTokenRefreshError::LockPoisoned`
  rather than propagating a bare panic — so the writer-side failure
  is typed, while readers fail loudly. The design note earlier drafted
  a uniform typed-error story; this revision documents the split
  explicitly so the sketched signatures and the rationale are
  consistent.
- `refresh_fn` is a boxed async closure. In production, it captures the
  cluster's `Credentials` (which `impl Clone`) + cluster name + region
  and calls `generate_eks_token` (aws_eks.rs:688). In tests, it captures
  a counter and returns a canned `TokenState`.
- `SecretString` per spec 06 Invariants: "Any refresh callback for
  0002-H2 handles `SecretString`, not `String`". `SecretString::new(token.
  into())` is the existing pattern at aws_eks.rs:804.
- The 10-second refresh threshold is spec 06's exact number
  ("`should_refresh` returns true when `Utc::now() + 10s >= expires_at`");
  encoded as `const REFRESH_LEEWAY: Duration = seconds(10)` so tests and
  production share the constant.

**`create_eks_client` refactor.** Change the return type from
`Result<kube::Client>` to `Result<(kube::Client, EksTokenRefresher)>`.
Build the initial `TokenState { token, expires_at = Utc::now() +
seconds(60) }` and the refresh closure:

```rust
let creds = credentials.clone();  // aws_credential_types::Credentials: Clone
let cluster_name = cluster.name.clone();
let region = cluster.region.clone();
let refresh_fn: RefreshFn = Arc::new(move || {
    let creds = creds.clone();
    let cluster_name = cluster_name.clone();
    let region = region.clone();
    Box::pin(async move {
        let token = generate_eks_token(&cluster_name, &creds, &region)
            .await
            .map_err(|e| EksTokenRefreshError::PresignFailed(e.to_string()))?;
        Ok(TokenState {
            token: SecretString::new(token.into()),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
        })
    })
});
let initial_token = generate_eks_token(&cluster.name, credentials, &cluster.region).await?;
let initial_state = TokenState {
    token: SecretString::new(initial_token.into()),
    expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
};
let refresher = EksTokenRefresher::new(initial_state, refresh_fn);
```

**Wire the refresh into the `kube::Client`** via a tower middleware
that intercepts every outbound request. kube-rs 0.98's
`ConfigExt::rustls_https_connector()` and `ConfigExt::base_uri_layer()`
are the documented extension points for building a custom-service client
(verified in the vendored `kube-client-0.98.0/src/client/config_ext.rs`):

```rust
use tower::{Layer, Service, ServiceBuilder};
use hyper_util::client::legacy::Client as HttpClient;
use hyper_util::rt::TokioExecutor;

#[derive(Clone)]
struct AuthRefreshLayer {
    refresher: Arc<EksTokenRefresher>,
}

impl<S> Layer<S> for AuthRefreshLayer {
    type Service = AuthRefreshService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        AuthRefreshService { inner, refresher: self.refresher.clone() }
    }
}

#[derive(Clone)]
struct AuthRefreshService<S> {
    inner: S,
    refresher: Arc<EksTokenRefresher>,
}

impl<S, B> Service<http::Request<B>> for AuthRefreshService<S>
where
    S: Service<http::Request<B>, Response = http::Response<hyper::body::Incoming>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut req: http::Request<B>) -> Self::Future {
        let refresher = self.refresher.clone();
        let mut inner = self.inner.clone();
        Box::pin(async move {
            if refresher.should_refresh() {
                refresher.refresh().await.map_err(|e| Box::new(e) as _)?;
            }
            let token = refresher.current_token();
            // SecretString::expose_secret is the sanctioned read.
            let header_value = format!("Bearer {}", secrecy::ExposeSecret::expose_secret(&token));
            req.headers_mut().insert(
                http::header::AUTHORIZATION,
                header_value.parse().map_err(|e: http::header::InvalidHeaderValue| Box::new(e) as _)?,
            );
            inner.call(req).await.map_err(Into::into)
        })
    }
}
```

Assemble the client with the layer applied:

```rust
let kube_config = kube::Config::from_custom_kubeconfig(kubeconfig, &Default::default()).await?;
let default_ns = kube_config.default_namespace.clone();
let https = kube_config.rustls_https_connector()?;
let http_service = HttpClient::builder(TokioExecutor::new())
    .build::<_, hyper::body::Incoming>(https);
let service = ServiceBuilder::new()
    .layer(kube_config.base_uri_layer())
    .layer(AuthRefreshLayer { refresher: Arc::new(refresher.clone_handle()) })
    .service(http_service);
let client = kube::Client::new(service, default_ns);
Ok((client, refresher))
```

`EksTokenRefresher::clone_handle` is a `pub fn clone_handle(&self) ->
EksTokenRefresher` that clones the internal `Arc`s (same `inner`,
`refresh_fn`, and `in_flight` — all `Arc`). This lets the middleware and
the returned `refresher` share state while both are `Send + 'static`.

**API drift caveat.** kube-rs 0.98's `ConfigExt::rustls_https_connector`
and `base_uri_layer` are the documented extension surface; if the exact
method names have shifted, the developer verifies via `cargo doc --open
-p kube` before wiring and adjusts imports. The slice-plan does not
speculate about renames — it names the surface the vendored source
exposes today.

The static `token` field in the synthesised `Kubeconfig`
(aws_eks.rs:804) remains in the code but is **inert** under the custom-
service assembly above: the layered service pipeline does not apply
kube-rs's `auth_layer`, so the kubeconfig-embedded token is never
consulted for outbound requests — the middleware's `AUTHORIZATION`
header write is the sole authentication path. Removing the static
`token` from the kubeconfig is out of scope for this slice (it stays a
diagnostic hint for any future reader who inspects the config), but
must not be described as a "fail-safe fallback" — there is no path in
which its absence would degrade to the initial token; requests would
simply carry no `Authorization` header at all. The middleware uses
`HeaderMap::insert` (not `append`) so no duplicate-header case arises
regardless.

### 5. `crates/baeus-core/src/cluster.rs` — reuse existing `set_token_expiry`

`ClusterConnection.set_token_expiry` already exists at cluster.rs:113
(verified: signature `pub fn set_token_expiry(&mut self, expiry: DateTime<
Utc>)`; already tested at cluster.rs:598, :606, :766, :770). **No new
method or test is added.** The round-1 draft proposed both; both were
duplicative.

The only edit to cluster.rs is a doc-comment addition on `set_token_expiry`
noting that it is **intended for future callers of `create_eks_client`**
that construct `ClusterConnection` records for refresher-backed clients.
Post-slice-B there is no such caller — `create_eks_client` remains
uncalled workspace-wide (the finalize spec 06 clarification queued in
step 7 acknowledges this gap), so `ClusterConnection.token_expiry`
stays unpopulated by this slice. The doc comment must therefore be
written in the future tense ("will be populated by callers that hold an
`EksTokenRefresher`") — not the present tense — so it does not falsely
imply a call relationship that does not exist yet. No behaviour change.

### 6. Wiring `create_eks_client`'s new return shape — and what does not need wiring

**No edits to `create_client_from_path_with_aws_creds`.** Verified against
the tree (client.rs:191-225): the function has no EKS branch, no call to
`create_eks_client`, and no need for an `EksTokenRefresher`. It exists to
build a kube::Client whose kubeconfig uses an `exec` plugin (`aws eks
get-token`) — the exec plugin is the refresh mechanism for that path. Spec
06 0002-H2's Affected-files list names this function, but against the
tree the spec's model is inaccurate on that specific line. The finalize
follow-up (§ step 7) queues a spec 06 clarification.

**No edits to `AppShell` for EKS refresher storage.** Because the live
EKS-connect path (app_shell.rs:1699) uses
`create_client_from_path_with_aws_creds` — not `create_eks_client` — no
`EksTokenRefresher` is produced along that path. The round-1 draft's
proposed `eks_refreshers: HashMap<String, EksTokenRefresher>` field is
therefore dropped from this slice. The refresher becomes a value any
future caller of `create_eks_client` may store; this slice does not add
such a caller.

**Second acceptance test — wiremock-K8S-API + counter refresh (step 6's
own test).** Spec 06 0002-H2 test expectation names wiremock as a fallback
for the AWS SDK smithy mock transport; because `build_eks_presigned_token`
is a pure client-side URL signer (no STS network I/O — see aws_eks.rs:
698-772), a wiremock-STS harness observes nothing. The evaluator flagged
substituting a canned-closure tower test as a deviation from the frozen
test expectation ("prove a second `presigned_token` request is issued
when the client makes a call after expiry"). This revision restores the
spec's stated observation surface by using **wiremock as the Kubernetes
API server the middleware calls out to** and a **counter-based refresh
closure** that increments a shared counter on each call, so both surfaces
are observed:

Add `crates/baeus-core/tests/aws_eks_refresh_integration.rs` (new file):

1. Start a `wiremock::MockServer` with a single mount rule:
   - `Mock::given(any()).respond_with(ResponseTemplate::new(200).
     set_body_json(json!({"kind": "Status", ...})))` — accept any GET,
     record every incoming request via
     `MockServer::received_requests()`.
2. Construct an `EksTokenRefresher` directly (bypassing
   `create_eks_client`, which reaches AWS SDK internals). Choose the
   initial `TokenState.expires_at` **outside** the 10-second
   `REFRESH_LEEWAY` so `should_refresh()` returns false on the first
   request — the fast path — and only becomes true after the
   sleep advances wall-clock time past `expires_at - 10s`. Concretely:
   `TokenState { token = "token-0", expires_at = Utc::now() +
   seconds(12) }`. Refresh closure captures an `Arc<AtomicUsize>`
   counter, increments on each call, and returns
   `TokenState { token = format!("token-{}", n), expires_at =
   Utc::now() + seconds(60) }` (well outside the leeway so the new
   token in turn takes the fast path). `n` is the value read after the
   increment; the first refresh yields `n == 1`.
3. Build a real `kube::Client` pointing at `mock_server.uri()` (kube-rs
   accepts a bare URL for the API server), with the `AuthRefreshLayer`
   installed as in step 4. This exercises the *actual middleware code
   path*, not a mock of it.
4. `let _ = client.list::<...>().await;` — first request. Assert
   `mock_server.received_requests().len() == 1`; assert the recorded
   `Authorization` header on request 0 equals `Bearer token-0` exactly
   — proving the fast path (spec 06 acceptance 2c) and that no
   spurious refresh fires before the leeway triggers.
5. Sleep past the leeway (`tokio::time::sleep(Duration::from_secs(3)).
   await`). Given the +12 s initial expiry, after a 3 s sleep the
   remaining lifetime is 9 s, which is inside the 10-second leeway;
   the next `should_refresh()` call therefore returns true. Bounded
   at single-digit seconds. Wall-clock sleep is required here because
   `Utc::now()` reads the system clock, and `tokio::time::advance`
   does not move `Utc::now()`.
6. `let _ = client.list::<...>().await;` — second request. Assert
   `mock_server.received_requests().len() == 2`; assert the recorded
   `Authorization` header on request 1 equals `Bearer token-1`
   exactly — proving one refresh fired between the two API calls.
7. Assert `counter.load(Ordering::SeqCst) == 1` — exactly one presign
   was issued (the second one; the fast-path first request did not
   refresh).

Justification of this substitution against the frozen spec 06 test
expectation: spec 06 defines the observation as "a second
`presigned_token` request is issued." In the tree, `presigned_token`
generation is `build_eks_presigned_token` — a synchronous, in-process,
network-free function. There is no network signal for wiremock to
observe at that boundary. The only externally-observable consequence
of a second presign is a different `Authorization` header value on the
next outbound Kubernetes API request. This test asserts exactly that,
plus the counter increment, plus that the middleware fired both
requests through a real `kube::Client + AuthRefreshLayer` stack — the
observation surface spec 06's acceptance criterion 2a *actually
cares about* ("a `kube::Client` produced by `create_eks_client`
succeeds on API calls made more than 60 seconds after construction").

If a plan evaluator judges the wiremock-K8S-API substitution
insufficient, the fallback is `aws-smithy-mocks-experimental` for the
STS boundary — but that crate's API has been churning across
`aws-smithy-*` releases and pinning it responsibly is more disruptive
than this test warrants. Slice-plan defaults to the wiremock harness;
the evaluator may request the smithy-mock switch as a revision.

### 7. Cross-check documentation references

Grep for stale prose. Known claims to check:

- `.docs/spec/03-toolchain-and-gate.md` — no watch or EKS refs.
  **No change.**
- `.docs/spec/02-architecture.md` — describes the kube-rs client layer.
  Verify with `grep -n 'watch\|EKS\|cancellation' .docs/spec/02-
  architecture.md`. If matches describe the current no-cancellation
  shape, spec 02 is stale — but spec 02 is Draft-status and frozen
  against slice-plan edits per the planner-role contract. The finalize
  pass records a spec 02 revision as pending, per the slice A precedent.
- `.docs/ADR/0002-kube-rs-client.md` — the ADR that constrains this
  surface. Verify no consequence statement contradicts the new
  cancellation surface. If so, the slice is out of scope (spec 06
  Decisions: "If, during slice-planning, any of these fixes surfaces a
  genuine open decision …, the planner must pause and raise a new ADR
  before the slice-plan proceeds"). Slice-plan reader: confirm before
  approving.
- **Spec 06 0002-H2 Affected-files inaccuracy.** Spec 06 lists
  `create_client_from_path_with_aws_creds` as an EKS-client-constructor;
  the tree shows it is an exec-plugin path with no EKS branch. This
  slice does not edit spec 06 (frozen; ADR 0005), but the finalize pass
  queues a spec 06 clarification / amendment to reconcile the
  Affected-files list with the tree. Escalate as Needs Clarification
  only if a future slice B follow-up depends on that reconciliation
  landing first.

If step 7 discovers a spec or ADR that must be edited to make an in-slice
acceptance criterion checkable, that is a spec revision — stop and raise
`Needs Clarification` rather than editing the frozen artifact from a
slice-plan.

## Verification

### Local (pre-push)

Run each command from repo root on the slice branch. All must pass before
the branch is pushed for review:

1. **Format** — `cargo fmt --all -- --check`.
2. **Lint** — `RUST_MIN_STACK=268435456 cargo clippy --workspace
   --all-targets -- -D warnings`.
3. **Test** — `RUST_MIN_STACK=268435456 cargo test --workspace`. The test
   count must **increase** by exactly:
   - 2 tests from step 1 (`watch_events_inner_stops_on_cancellation`,
     `watch_resources_inner_stops_on_cancellation`).
   - 2 tests from step 2 in `informer.rs`
     (`test_set_cancel_token_stores_on_entry`,
     `test_stop_for_cluster_cancels_all_tokens_for_cluster`).
   - 3 tests from step 2 in `watch.rs`
     (`test_register_watcher_returns_cancellation_token`,
     `test_stop_watching_cancels_token`,
     `test_stop_for_cluster_cancels_all_tokens_for_cluster` — bridge
     variant).
   - 3 tests from step 4 (`should_refresh_returns_true_when_within_ten_
     seconds_of_expiry`, `refresher_produces_fresh_token_on_call`,
     `refresher_surfaces_typed_error_on_failure`).
   - 1 integration test from step 6
     (`aws_eks_refresh_integration.rs`, one `#[tokio::test]`).
   - Total: **+11 tests**.
   Note: step 5 no longer adds a duplicate `set_token_expiry` test
   (existing coverage at cluster.rs:598, :606 is retained). Constitution
   invariant: "no decrease in test count" — this slice *adds* eleven,
   satisfying the invariant strictly.
4. **Deny** — `cargo deny check` (green post-slice-A; new deps must not
   trip advisories or licence rules).

### Remote (on the PR)

The CI matrix (macOS / Linux / Windows on the pinned toolchain — assumes
Slice A2 has landed; if not, this slice runs on floating `@stable` and
carries the drift risk it would otherwise inherit) must pass all four
gate steps on all three legs.

### Slice-specific acceptance (from spec 06, restated)

**0002-H1 acceptance:**

1a. Cancelling the token stops the watch loop within one poll cycle
    (test asserts observable via `JoinHandle` completing within a
    100 ms `tokio::time::timeout`).
1b. `ResourceWatchBridge::stop_watching` cancels the associated loop
    (bridge test asserts `token.is_cancelled()` post-stop); the live
    UI path via `InformerManager::stop_for_cluster` cancels every
    per-cluster token (informer test).
1c. Public API of `watch_events` / `watch_resources` remains **backward
    compatible for callers that do not care about cancellation** —
    accomplished via `Option<CancellationToken>` with an internal
    `None → CancellationToken::new()` default. Callers that pass `None`
    get today's behaviour (a fresh, never-signalled token — loop runs
    to error or stream end). This satisfies spec 06 0002-H1 fix approach
    ("optional-not-required (default `None` -> new token constructed
    internally, or overload the entry points)") verbatim.

**0002-H2 acceptance:**

2a. A `kube::Client` produced by `create_eks_client` succeeds on API
    calls made after simulated ≥60 s elapse — the step-6 integration
    test asserts this by seeding the refresher with an initial
    `expires_at = Utc::now() + 12s` (outside the 10-second leeway),
    sleeping 3 s (which lands the remaining lifetime *inside* the
    leeway), and issuing a second real `kube::Client` request through
    the `AuthRefreshLayer`. Request 0 carries `Bearer token-0` (fast
    path); request 1 carries `Bearer token-1` (post-refresh);
    `counter == 1` (exactly one presign issued between the two API
    calls).
2b. Refresh failure surfaces as `EksTokenRefreshError`, not opaque
    `anyhow::Error` (test:
    `refresher_surfaces_typed_error_on_failure`).
2c. Fast-path: when `expires_at` is comfortably in the future,
    `should_refresh()` returns false and no refresh call is issued
    (verified by
    `should_refresh_returns_true_when_within_ten_seconds_of_expiry`'s
    false-branch case, plus step 6's *first* API call, which is
    seeded with `expires_at = Utc::now() + 12s` and asserted to
    carry the initial `Bearer token-0` with the counter still at
    zero — proof that the fast path fired without issuing a
    presign).

### Gate (spec 03 + slice-specific)

| Step | Command |
|------|---------|
| format | `cargo fmt --all -- --check` |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` (+11 tests) |
| deny | `cargo deny check` |

## Notes

- **2026-08-26 — Spec 06 0002-H2 Affected-files inaccuracy (informational,
  not a Needs Clarification).** Spec 06 0002-H2 names
  `crates/baeus-core/src/client.rs — create_client_from_path_with_aws_creds`
  under Affected files, describing it as "wires the refresher through
  when constructing EKS clients." Against the tree,
  `create_client_from_path_with_aws_creds` (client.rs:191-225) has no
  EKS branch, does not call `create_eks_client`, and produces a
  kube::Client via kube-rs's default construction over an exec-plugin
  kubeconfig (aws eks get-token — which refreshes tokens per invocation).
  The live EKS-connect UI path (app_shell.rs:1699) uses this function
  and therefore does not carry the static-60s-TTL defect 0002-H2
  describes. `create_eks_client` (aws_eks.rs:779) — which does carry
  the defect — has zero non-doc callers workspace-wide.

  This slice satisfies spec 06 0002-H2's stated acceptance criterion
  (2a: "A `kube::Client` produced by `create_eks_client` succeeds on
  API calls made more than 60 seconds after construction") by
  restructuring `create_eks_client` and leaving
  `create_client_from_path_with_aws_creds` untouched. The eval's guidance
  ("raise a spec clarification if the spec's model of the code is
  wrong") is deferred to finalize as a spec 06 clarification queue item
  rather than a Needs Clarification pause here, because the misalignment
  does not block landing an acceptance-criterion-satisfying fix. A
  future planner may choose to (a) migrate the live path to use
  `create_eks_client + EksTokenRefresher`, or (b) delete `create_eks_client`
  in favour of the exec-plugin path — either resolution rewrites the
  Affected-files list.

  Riding along with the same finalize clarification: spec 06 0002-H2's
  Affected-files expectation "`token_expiry` field now populated" stays
  **unmet** by this slice. `ClusterConnection.token_expiry` remains
  `None` post-slice-B because `create_eks_client` is uncalled — the
  new refresher's `expires_at()` accessor is available for any future
  caller to feed into `set_token_expiry`, but the wiring itself is out
  of scope. This gap is intrinsic to the Affected-files inaccuracy
  above (until a live caller exists, the field cannot be populated)
  and resolves along with whichever migration option (a) or (b) the
  future planner chooses.

- **2026-08-26 — Spec 06 Slice Breakdown row for Slice B does not
  name `app_shell.rs`.** Spec 06's Slice B row lists only
  `crates/baeus-core/src/{client.rs, aws_eks.rs, cluster.rs, watch.rs}`
  and `crates/baeus-core/Cargo.toml` as Primary files. This slice
  edits `crates/baeus-ui/src/layout/app_shell.rs` at two call sites
  (`start_event_watcher` at :2240, `start_resource_watcher` at :2658),
  adds one new field (`resource_watch_cancels`), and wires
  cancellation across `start_event_watcher`, `start_resource_watcher`,
  and `stop_event_watcher` — the round-1 evaluation required this
  wiring, and spec 06 0002-H1's fix approach ("public entry points
  accept a `CancellationToken`; propagate to informer + watch bridge;
  cancel on cluster switch / disconnect") is not satisfiable without
  it. Slices D, F, and G's rows list `app_shell.rs` explicitly when
  their fixes require UI edits; the Slice B row was drafted before
  the plan-eval round-1 UI-wiring guidance and does not include it.
  Because an Approved spec is frozen (ADR 0005), the correction is
  handled as a **dated additive amendment** to spec 06 in the same
  finalize pass that queues the 0002-H2 Affected-files clarification
  above — the Slice B row is amended to add
  `crates/baeus-ui/src/layout/app_shell.rs` (start_event_watcher,
  start_resource_watcher, stop_event_watcher call-site wiring and
  the `resource_watch_cancels` field) to its Primary files list. No
  other section of spec 06 is altered. This is called out here so
  that finalize does not merge the plan without carrying the
  amendment through.
