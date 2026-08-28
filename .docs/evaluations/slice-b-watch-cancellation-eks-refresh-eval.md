# Evaluation: slice-b-watch-cancellation-eks-refresh.md

Verdict: FAIL
Round: 1
Reviewed against: `.docs/spec/06-remediation-highs.md` (0002-H1, 0002-H2,
Invariants, Slice B row), `.docs/research/0002-core-client-aws-review.md`
(findings 1–2), and the real tree: `crates/baeus-core/src/{client.rs,
watch.rs, aws_eks.rs, cluster.rs, informer.rs}`, `crates/baeus-core/Cargo.toml`,
workspace `Cargo.toml` / `Cargo.lock`, `crates/baeus-ui/src/layout/app_shell.rs`,
and the vendored `kube-client-0.98.0` sources.

## Findings

- [BLOCKER] Step 6's central premise is false — `create_client_from_path_with_aws_creds`
  has no "EKS branch" and never calls `create_eks_client`. Verified at
  `client.rs:191-226`: the function builds an exec-injection kubeconfig and calls
  `Client::try_from(config)`; there is no EKS branch. A workspace-wide grep shows
  `create_eks_client` (`aws_eks.rs:779`) has **zero callers** — it is dead code.
  The plan's 0002-H2 wiring step ("this function must be updated to accept the new
  shape and propagate the refresher") targets a code path that does not exist and
  never identifies where EKS clients are actually constructed in the live path
  (the exec-plugin injection route, which re-generates tokens per invocation via
  the AWS CLI/SDK exec and does not suffer the static-60s-TTL defect the same way).
  A developer cannot execute step 6 as written, and the acceptance criterion 2a
  ("a `kube::Client` produced by `create_eks_client` succeeds on API calls after
  60 s") is left attached to a constructor nothing calls.

- [BLOCKER] Step 4/6 require `tower` (`Service`, `Layer`, `ServiceBuilder`,
  `service_fn` in the step-6 test), `hyper` (`hyper::body::Incoming`), and
  `hyper-util` (`hyper_util::client::legacy::Client`, `TokioExecutor`) — none of
  which are dependencies of `baeus-core` or declared in the workspace
  `[workspace.dependencies]` (verified: `crates/baeus-core/Cargo.toml` and root
  `Cargo.toml`; `kube` 0.98 does not re-export tower/hyper — `kube/src/lib.rs`
  re-exports only `kube_client::{api, client, config, discovery, error}` and
  `kube_core`). Step 0 adds only `tokio-util` and explicitly asserts it is "the
  dependency without which subsequent steps fail to compile"; that claim is false.
  The sketched `AuthRefreshLayer`/`AuthRefreshService` and the client assembly in
  step 4 cannot compile without additional `[dependencies]` entries (tower with
  the `util` feature, hyper, hyper-util) that the plan never names.

- [MAJOR] The 0002-H1 wiring story does not match the UI's real structure.
  Step 3 references `self.resource_watch_bridge.stop_for_cluster(cluster_id)` —
  but `AppShell` has no `resource_watch_bridge` field; it holds
  `informer_manager: InformerManager` directly (`app_shell.rs:445`) and its
  disconnect path calls `self.informer_manager.stop_for_cluster(&cluster_id)`
  (`app_shell.rs:2310`). `ResourceWatchBridge` has zero non-test callers
  anywhere in the workspace, so the plan's aside "Callers today ignore the
  second tuple field via `let (id, _cancel) = ...`" describes callers that do
  not exist. The plan never reconciles the parallel watcher-registration paths
  (`informer_manager.register_standard_watchers` at `app_shell.rs:2221` vs. the
  bridge) and leaves ambiguous where cancellation tokens actually live and who
  cancels them on cluster switch — the core behavioral promise of 0002-H1.

- [MAJOR] Deviation from frozen spec 06 0002-H1 acceptance: spec requires the
  public API of `watch_events` / `watch_resources` to "remain backward
  compatible for callers that do not care about cancellation" (fix approach:
  optional param with internal default, or overload). The plan makes the
  `CancellationToken` parameter **required** and restates acceptance 1c in
  weakened form ("accepts a `CancellationToken` parameter; existing tests
  updated to pass one"). A slice-plan may not rewrite a frozen spec acceptance
  criterion; either implement the spec's optional/overload shape or route the
  deviation through a spec amendment / ADR per spec 06 Decisions.

- [MAJOR] `EksTokenRefresher` design is internally inconsistent: the design note
  mandates `tokio::sync::RwLock` ("because `refresh` is `async`"), but the
  sketched API is synchronous — `should_refresh(&self) -> bool`,
  `current_token(&self) -> SecretString`, `expires_at(&self) -> DateTime<Utc>` —
  which cannot read a `tokio::sync::RwLock` without `.await`. As sketched the
  type does not typecheck; the obvious fix (`std::sync::RwLock` with short
  critical sections) is unremarked, leaving the slice's central new type
  ambiguous at its most safety-relevant point (concurrent readers during
  refresh).

- [MAJOR] Spec 06 0002-H2 test expectation names the AWS smithy mock transport
  or wiremock STS "to prove a second `presigned_token` request is issued". The
  plan substitutes a tower-layer test with a canned closure that never exercises
  presigning at all, and argues equivalence. The substitution is transparently
  flagged with a wiremock fallback offered, but it is still a deviation from the
  frozen test expectation; the revision should either adopt the spec's harness
  or demonstrate the tower test plus one presign-level assertion covers the
  criterion.

- [MINOR] Step 2 claims "8 call sites" of `register_watcher(` in watch.rs tests
  to update for the new tuple return — the actual count is 39 occurrences in the
  `#[cfg(test)]` module. Mechanical edit either way, but the number was not
  verified against the tree.

- [MINOR] `ClusterConnection::set_token_expiry` already exists
  (`cluster.rs:113-115`) and already has coverage (`cluster.rs:588-606`); the
  plan hedges the helper with "if not already write-through" (fine) but the
  promised `test_set_token_expiry_populates_field` duplicates existing tests.
  Also, step 6 never names the one real caller of
  `create_client_from_path_with_aws_creds` (`app_shell.rs:1699`) that would need
  updating for the new return shape (moot given BLOCKER 1).

- [MINOR] `watch_events_inner` sketch takes `stream: Pin<&mut S>`, but the
  step-1 test spawns the helper via `tokio::spawn`, which requires `'static` —
  a future borrowing a stack-pinned stream cannot be spawned. Take `S` by value
  and `tokio::pin!` inside the helper (mirroring the current `watch_events`
  body, which already does `tokio::pin!(stream)`).

- [MINOR] Line-number drift the plan did not self-correct: `X-Amz-Expires` is at
  `aws_eks.rs:732` (plan: 774), `generate_eks_token` at `aws_eks.rs:688`
  (plan: 696). (The plan did correctly self-correct the `SecretString` site to
  `:804`.)

## What verified cleanly (for the record)

- `watch_events` at `client.rs:1138`, `watch_resources` at `client.rs:1195`,
  both `while let Some(...)` over `watcher(...).default_backoff()` — plan's
  problem statement is accurate.
- UI call sites `app_shell.rs:2240` (`watch_events`) and `:2658`
  (`watch_resources`) exist as claimed; `start_event_watcher` (~2193ff) and
  `start_resource_watcher` (~2610ff) confirmed.
- kube / kube-runtime 0.98.0 in `Cargo.lock`; `ConfigExt::{base_uri_layer,
  auth_layer, rustls_https_connector}` confirmed present in vendored
  `kube-client-0.98.0/src/client/config_ext.rs` — the plan's kube-rs API claims
  (with its own hedge) are sound.
- `tokio-util` 0.7.18 at `Cargo.lock:7359-7362`, transitive-only; not a direct
  dep of `baeus-core` — plan's dependency framing for `tokio-util` is accurate.
- `stop_for_cluster` correctly identified as possibly absent from
  `ResourceWatchBridge` (it exists only on `InformerManager`,
  `informer.rs:155`) — the plan's hedge was warranted.
- `token_expiry: Option<DateTime<Utc>>` at `cluster.rs:39` confirmed.
- Test inventory (+10) is internally consistent, and the step-1/2/4 tests are
  genuine behavior tests, not tautologies. SecretString/zeroize handling in
  `TokenState` satisfies spec 06's security invariant; no token logging
  introduced.

## Required changes (for FAIL)

1. Re-derive the 0002-H2 wiring from the real tree: determine where EKS clients
   are actually constructed on the live path (exec-injection route in
   `create_client_from_path_with_aws_creds` vs. the dead `create_eks_client`),
   state explicitly that `create_eks_client` is currently uncalled, and rewrite
   step 6 to name the real construction/call sites the refresher threads
   through (including `app_shell.rs:1699`). If the live path's exec plugin
   already refreshes per-call, say so and re-scope the fix to what spec 06 H2
   actually requires — raising a spec clarification if the spec's model of the
   code is wrong.
2. Add the missing dependency declarations to step 0 (tower with `util`,
   hyper, hyper-util — exact features named), or restructure step 4 to build the
   client via APIs that need no new deps, and drop the false claim that
   `tokio-util` is the only dependency required.
3. Rewrite step 3 to match the real UI structure: name `informer_manager` /
   the actual `AppShell` fields and disconnect path (`app_shell.rs:2310`),
   state that `ResourceWatchBridge` is currently UI-unwired, and give one
   unambiguous design for where tokens are stored and who cancels them on
   cluster disconnect/switch.
4. Bring the `watch_events` / `watch_resources` signature change inside spec
   06's letter (optional-not-required token, or overload) — or explicitly route
   the required-parameter deviation through the spec's amendment path.
5. Fix the `EksTokenRefresher` lock design: pick `std::sync::RwLock` (or make
   the getters async) so the sketched API typechecks.
6. Either adopt spec 06 H2's named test harness (smithy mock / wiremock) or add
   one presign-level assertion to the tower-layer test and justify the
   substitution against the frozen test expectation.
7. Correct the "8 call sites" figure (actual: 39) and the stale line numbers
   (`aws_eks.rs:732`, `:688`).

## Notes

The plan is well-organized, honestly hedged in several places (stop_for_cluster,
SecretString line, kube API drift), and its problem statement for 0002-H1 is
accurate. But its 0002-H2 half is planned against a imagined call graph
(`create_eks_client` is dead code; the named wiring function has no EKS branch),
and its dependency analysis is materially incomplete. Both are exactly the
class of mechanical-truth failures a slice-plan must not carry into
development — the developer would discover them in the first hour, mid-edit,
with the plan offering no guidance for the real structure.
