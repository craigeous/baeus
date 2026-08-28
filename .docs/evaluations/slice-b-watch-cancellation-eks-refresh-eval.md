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


---

# Evaluation: slice-b-watch-cancellation-eks-refresh.md

Verdict: FAIL
Round: 2
Reviewed against: `.docs/spec/06-remediation-highs.md` (0002-H1, 0002-H2,
Invariants, Slice B row), `.docs/research/0002-core-client-aws-review.md`
(finding 2), the round-1 evaluation
(`.docs/evaluations/slice-b-watch-cancellation-eks-refresh-eval.md`), the
diff `82f19f4..HEAD` of the artifact, and the real tree:
`crates/baeus-core/src/{client.rs, aws_eks.rs, aws_sso.rs, cluster.rs,
informer.rs, watch.rs}`, `crates/baeus-core/Cargo.toml`, workspace
`Cargo.toml` / `Cargo.lock`, `crates/baeus-ui/src/layout/app_shell.rs`, and
the vendored `kube-client-0.98.0` sources.

## Round-1 findings — resolution status (verified against the tree)

- **BLOCKER 1 (step-6 premise false / dead `create_eks_client`) — RESOLVED.**
  The revision's corrected mental model verifies mechanically:
  `grep -Rn 'create_eks_client' crates/` returns only the definition
  (`aws_eks.rs:779`) — zero callers, dead code, as the plan now states.
  `create_client_from_path_with_aws_creds` (`client.rs:191-225`) contains
  no EKS branch and no `create_eks_client` call — it reads a kubeconfig,
  injects exec-env credentials via `aws_sso.rs:106`, and builds via
  `Client::try_from`. Its single live caller is `app_shell.rs:1699`, and
  the tree itself documents that path's immunity (`app_shell.rs:1644`:
  "This uses `aws eks get-token` which refreshes tokens automatically (no
  60s expiry)"; exec-plugin kubeconfig written at
  `app_shell.rs:12080`). Research 0002 finding 2 states the defect entirely
  in terms of `create_eks_client`, and spec 06 0002-H2 acceptance
  criterion 2a is likewise phrased ("A `kube::Client` produced by
  `create_eks_client` succeeds on API calls made more than 60 seconds after
  construction"). Because the live path carries no defect, fixing the dead
  constructor does not leave a live defect unaddressed — there is none.
  Targeting `create_eks_client` is the faithful reading of both
  authorities, the discrepancy with spec 06's Affected-files list is
  transparently flagged, and a spec 06 clarification is queued for
  finalize. This matches round-1 required change 1's permitted path.

- **BLOCKER 2 (missing tower/hyper/hyper-util deps) — RESOLVED.** Step 0
  now declares `tower = { version = "0.5", features = ["util"] }`,
  `hyper = "1"`, `hyper-util = { version = "0.1", features = ["client",
  "client-legacy", "http1", "http2", "tokio"] }`, and
  `tokio-util = { version = "0.7", features = ["rt", "sync"] }` in
  `crates/baeus-core/Cargo.toml` (the correct manifest), plus dev-dep
  `wiremock = "0.6"`. Lock versions corroborate (`tower 0.5.3`,
  `hyper-util 0.1.20`, `tokio-util 0.7.18` at `Cargo.lock:7465/:3577/:7359`).
  `http = "1"` and `futures` are already direct deps, covering the
  middleware and the `futures::stream::pending` test streams. The false
  "tokio-util is the only dep" claim is explicitly retracted. kube 0.98's
  non-re-export of tower is re-verified against the vendored
  `kube/src/lib.rs`.

- **MAJOR (`resource_watch_bridge` field / UI wiring) — RESOLVED.** The
  revision verifies `ResourceWatchBridge` has zero non-test callers
  (workspace grep returns only `watch.rs` self-references), names the real
  UI structure — `informer_manager: InformerManager` at `app_shell.rs:445`,
  `register_standard_watchers` at `:2221`, Running marks at `:2223-2226`,
  `watch_events` spawn at `:2240`, `stop_event_watcher` at `:2304-2312`
  calling `informer_manager.stop_for_cluster` at `:2310`,
  `start_resource_watcher` at `:2613` with `watch_resources` at `:2658` and
  `active_resource_watchers` at `:2627/:2643/:2718` — all confirmed against
  the tree. Token storage and cancellation responsibility are now
  unambiguous: manager entries for standard watchers, a new
  `resource_watch_cancels` map for ad-hoc resource watchers, cancelled from
  the single disconnect path (`stop_event_watcher`'s only caller,
  `app_shell.rs:1927`). `ResourceListKey.cluster_context: String`
  (`app_shell.rs:60`) makes the step-3 iteration type-correct.

- **MAJOR (required CancellationToken vs spec 06 H1 backward-compat) —
  RESOLVED.** `Option<CancellationToken>` with internal
  `unwrap_or_else(CancellationToken::new)` matches spec 06's "optional-
  not-required (default `None` -> new token constructed internally)"
  verbatim. Acceptance 1c is restated at full strength. Only two live call
  sites exist (`app_shell.rs:2240`, `:2658`; the `baeus-plugins` matches
  are a different `watch_resources` on a plugin context), so the "mechanical
  edit at two call sites" claim is accurate.

- **MAJOR (RwLock/sync-getter contradiction) — RESOLVED.** The revised
  `EksTokenRefresher` uses `std::sync::RwLock` for the synchronous getters
  and a `tokio::sync::Mutex<()>` to serialise refreshes, with a re-check
  after guard acquisition. The sketch typechecks conceptually; no `.await`
  is held across the std lock.

- **MAJOR (canned tower test substituting for the spec-named harness) —
  MOSTLY RESOLVED, but see new [MAJOR] 1 below.** The revision adopts
  wiremock (one of spec 06's two named harnesses), points it at the
  Kubernetes API server rather than STS for a documented technical reason
  (`build_eks_presigned_token` is a pure client-side signer with no network
  I/O — verified, `aws_eks.rs:698-772`), and observes the second presign
  via the outbound `Authorization` header plus a refresh counter through a
  real `kube::Client + AuthRefreshLayer` stack. The substitution
  justification against the frozen test expectation is explicit and
  reasoned, satisfying round-1 required change 6 in kind.

- **MINORs — RESOLVED.** The "8 call sites" figure is corrected to 39 and
  verified (`grep -c 'bridge.register_watcher(' watch.rs` = 39). The
  duplicate `set_token_expiry` test is dropped (existing coverage at
  `cluster.rs:598/:606` acknowledged; the helper itself confirmed at
  `cluster.rs:113`). The `Pin<&mut S>`-spawn defect is fixed — the inner
  helper takes the stream by value with an explicit `'static` bound and
  `tokio::pin!` inside. Stale line references are corrected and verified
  (`X-Amz-Expires` at `aws_eks.rs:732`, `generate_eks_token` at `:688`,
  `build_eks_presigned_token` at `:698`, `SecretString` at `:804`).

## Findings (round 2 — fresh)

- [MAJOR] Step 6's integration-test recipe is arithmetically unsatisfiable
  as written, and contradicts itself and the Verification section. The step
  specifies initial `TokenState { expires_at = Utc::now() + seconds(2) }`,
  a refresh closure returning `expires_at = now + 60s`, a 3-second sleep,
  and then asserts (bullet 6) the second request carries a *higher* token
  counter than the first and (bullet 7) `counter >= 2`. With a 10-second
  `REFRESH_LEEWAY`, `expires_at = now+2s` makes `should_refresh()` true
  immediately: request 1 refreshes (counter=1, fresh 60 s expiry), and
  request 2 — 3 s later — takes the fast path with the *same* token.
  Bullets 6 and 7 both fail. The alternative reading hinted in bullet 2's
  own parenthetical ("using an initially-not-should-refresh state", e.g.
  `now+12s`) makes request 1 a fast-path and request 2 the sole refresh —
  bullet 6 then passes but bullet 7 still fails (counter == 1). No single
  initial state satisfies the sketched assertions, and bullet 2 is
  internally contradictory ("well inside the 10-second REFRESH_LEEWAY so
  the very first `should_refresh` returns true — **but** the test ...
  us[es] an initially-not-should-refresh state" — both cannot hold for
  `now+2s`). The Verification section restates a third, different variant
  ("two-second expiry + three-second sleep ... second request's
  `Authorization` header differs from the first"), which is also false
  under the 2 s figure (both requests carry the post-refresh token). This
  is the slice's flagship evidence for frozen acceptance criterion 2a —
  the test round 1 demanded be made genuine — and an independent reader
  implementing it literally gets a red test and no guidance: the plan
  supplies three mutually inconsistent parameterisations and none that
  works. The correction is small and deterministic (initial expiry outside
  the leeway, e.g. `now+12s`; assert the first request carries `token-0`,
  the second `token-1`, and `counter == 1`), but the plan must state it,
  because the whole point of the test is which assertions prove 2a. The
  same sloppiness infects step 4's `refresher_produces_fresh_token_on_call`:
  calling `.refresh().await` twice and expecting two different tokens
  requires the canned closure's returned `expires_at` to remain within the
  leeway (otherwise the second call short-circuits on the re-check and
  returns the same token) — a precondition the plan never states.

- [MINOR] Slice B's row in spec 06's Slice Breakdown names only
  `crates/baeus-core/src/{client.rs, aws_eks.rs, cluster.rs, watch.rs}` and
  `crates/baeus-core/Cargo.toml` as primary files; the plan edits
  `app_shell.rs` (two call-site changes, one new
  `resource_watch_cancels: HashMap<ResourceListKey, CancellationToken>`
  field, and wiring across `start_event_watcher`,
  `start_resource_watcher`, and `stop_event_watcher`). The round-1
  evaluation required this wiring and the finding is not remediated
  without it, and "Primary crates / files" is not exhaustive (slices D/F/G
  show the spec lists `app_shell.rs` explicitly when UI edits are
  anticipated) — but the plan should acknowledge the row-level mismatch
  rather than leave it silent.

- [MINOR] Step 3's parenthetical "(spec 06 explicitly bounds UI edits to
  the mechanical call-site adjustments)" misattributes spec 06: the only
  "call-site adjustments" phrase in spec 06 belongs to 0005-H5 (terminal
  emulator, spec line 595), not to Slice B. No such Slice B bound exists —
  and the plan's own new `AppShell` field would exceed it if it did.

- [MINOR] Step 5's doc-comment edit claims `set_token_expiry` "is now
  called from the 0002-H2 refresh path" — but no such call exists after
  this slice (`create_eks_client` remains uncalled, so nothing populates
  `ClusterConnection.token_expiry`, exactly the state research 0002
  flagged). The hedge "(once a caller of `create_eks_client` exists)"
  concedes this, but a present-tense doc claim of a nonexistent call
  relationship should not be written into `cluster.rs`; spec 06 H2's
  Affected-files expectation "`token_expiry` field now populated" stays
  unmet and should ride along with the finalize spec clarification the
  plan already queues.

- [MINOR] Step 4's "fail-safe fallback" rationale is inaccurate: in the
  custom-service assembly (`ServiceBuilder` + `base_uri_layer` +
  `AuthRefreshLayer` over a hand-built `hyper_util` client) no kube
  `auth_layer` is applied, so kube-rs never sends the static kubeconfig
  token at all — if the middleware's header write were removed, requests
  would carry *no* `Authorization`, not the initial token. Harmless to the
  design (the static token is simply inert), but the stated reasoning is
  wrong and should not survive into code comments.

- [MINOR] `EksTokenRefresher`'s design note promises "Poisoned-lock cases
  surface as `EksTokenRefreshError::LockPoisoned` rather than
  panic-in-request-path", but the sketched `should_refresh()` /
  `current_token()` — the two methods the middleware calls on every
  request — use `.expect("lock poisoned")`, i.e. panic-in-request-path.
  Only `refresh()` maps poisoning to the typed error. Pick one story.

- [MINOR] "stop_event_watcher ... is already called by every
  `on_cluster_connection_lost` and cluster-disconnect path today" —
  it has exactly one caller (`app_shell.rs:1927`, the disconnect handler).
  Substance is fine (that is the disconnect path); the quantifier is not
  verified.

## What verified cleanly this round (for the record)

- kube 0.98 client-construction surface: `Client::new(service,
  default_namespace)` and the `ServiceBuilder::new().layer(config.
  base_uri_layer()) ... .service(...)` pattern are documented in the
  vendored `kube-client-0.98.0/src/client/mod.rs:110-118`;
  `ConfigExt::{base_uri_layer, rustls_https_connector}` confirmed in
  `config_ext.rs:23,:47`. The step-4 assembly matches the documented
  pattern.
- `generate_eks_token(cluster_name: &str, credentials: &Credentials,
  region: &str)` (`aws_eks.rs:688`) and `create_eks_client(cluster:
  &EksCluster, credentials: &Credentials)` (`:779`) match the step-4
  refresh-closure sketch (captures `Credentials: Clone` + name + region).
- Real `InformerManager::{register, unregister(:68), set_state,
  informers_for_cluster(:113), stop_for_cluster(:155),
  clear_cache_for_cluster(:196)}` and real `ResourceWatchBridge`
  (`watcher_ids: HashMap<(Uuid, String), Uuid>` at `watch.rs:14`,
  `register_watcher:25`, `stop_watching:62`) match the step-2 sketches'
  bases; the `WatcherEntry`-valued map and both `stop_for_cluster`
  variants are coherent extensions. Spec 06 H1's conditional allowance for
  `informer.rs` ("only if `InformerEntry` needs to hold the token for
  stop-by-key routing") is met — the live UI disconnect path routes
  through the manager.
- Spec 06 Invariants: no GPUI main-thread I/O added (refresh runs in the
  async request path on the Tokio runtime); `TokenState` carries
  `SecretString` with `expose_secret` only at the header-write boundary;
  no token logging introduced. TDD ordering (red tests first in every
  code-bearing step) is preserved; the +11 test count is internally
  consistent (2 + 2 + 3 + 3 + 1) and satisfies the no-decrease invariant.
- Scope discipline: non-goals correctly defer 0002 §5–§10 mediums, H3/H4
  to Slice C, and now also the live-path migration / `create_eks_client`
  deletion, with rationale.

## Required changes (for FAIL)

1. Fix step 6's integration-test recipe so its parameters and assertions
   are mutually satisfiable, and align bullet 2, bullets 4–7, and the
   Verification section's 2a restatement to one working variant: initial
   `expires_at` outside the 10-second leeway (e.g. `Utc::now() + 12s`),
   first request asserted to carry the initial token (fast path, also
   proving acceptance 2c), sleep chosen to cross `now + 10s >= expires_at`
   (3 s suffices for +12 s), second request asserted to carry the refreshed
   token, and the refresh counter asserted to equal exactly one — or, if a
   `>= 2` count is wanted, state that the canned closure returns
   within-leeway expiries so each request refreshes. Delete the
   self-contradictory parenthetical in bullet 2.
2. State the analogous precondition in step 4's
   `refresher_produces_fresh_token_on_call` (canned closure must return
   within-leeway `expires_at`, or prime the state to force both refreshes),
   so the two-refresh assertion can hold.
3. Reconcile step 5's doc-comment claim with reality (drop the
   present-tense "is now called" wording; fold the unpopulated
   `token_expiry` gap into the queued spec 06 clarification), correct the
   "fail-safe fallback" rationale and the poisoned-lock note in step 4,
   fix the spec-06 misattribution in step 3, and acknowledge the Slice B
   row's file list not naming `app_shell.rs`.

## Notes

The revision is a substantial, good-faith correction: both round-1
BLOCKERs and all four MAJORs are genuinely resolved, and every load-bearing
structural claim I re-verified against the tree this round (EKS call
graph, app_shell wiring points, informer/bridge shapes, kube 0.98
extension surface, dependency manifests and lock versions, spec text)
checked out. The corrected mental model of the EKS paths is not merely
plausible — it is right, and the plan's handling of the spec-06
Affected-files inaccuracy (fix the surface the frozen acceptance criterion
names; flag the discrepancy; queue a clarification rather than pausing) is
the best available reading of the authorities. What remains is narrower
than round 1 but the same species: the one test the whole revision rides
on — the wiremock integration test substituted for spec 06 H2's named
harness — cannot go green as specified, and the plan says so itself three
different ways. A slice-plan whose central acceptance evidence is
arithmetically self-defeating is not executable as written; per the
severity rule (an unaddressed MAJOR fails), this cannot pass. The fix is
an hour of editing, not a redesign.


---

# Evaluation: slice-b-watch-cancellation-eks-refresh.md

Verdict: PASS
Round: 2
Reviewed against: the round-2 evaluation
(`.docs/evaluations/slice-b-watch-cancellation-eks-refresh-eval.md`, last
section), the diff `274f407..3134215` of the artifact,
`.docs/spec/06-remediation-highs.md` (0002-H1, 0002-H2, Invariants, Slice B
row), `.docs/research/0002-core-client-aws-review.md` (finding 2), and the
real tree: `crates/baeus-core/src/{client.rs, aws_eks.rs, cluster.rs,
informer.rs, watch.rs}`, `crates/baeus-ui/src/layout/app_shell.rs`.

## Round-2 findings — resolution status

- **MAJOR (step-6 wiremock test arithmetically unsatisfiable) — RESOLVED.**
  The revision replaces the contradictory `now+2s` / "initially-not-
  should-refresh" / `counter >= 2` recipe with a single coherent
  parameterisation, and I walked the timeline mechanically:
  - t0: initial `TokenState { token = "token-0", expires_at = t0 + 12s }`.
    Request 1: `should_refresh()` ⇔ `t0+10 >= t0+12` ⇔ false → fast path.
    Header `Bearer token-0`, counter 0. Bullet 4's assertions hold.
  - Sleep 3 s → t0+3: remaining lifetime 9 s; `t0+13 >= t0+12` ⇔ true →
    the second request refreshes exactly once: counter 0→1, closure
    returns `token = "token-1"` (`n` read post-increment — bullet 2 now
    states this), `expires_at = t0+3+60s` (outside the leeway, so no
    further refresh can fire). Bullet 6's `Bearer token-1` assertion
    holds; bullet 7's `counter == 1` holds.
  - The self-contradictory bullet-2 parenthetical is deleted; bullets
  4–7, and the Verification section's 2a and 2c restatements, all name
  the same +12 s / 3 s / token-0 → token-1 / counter == 1 variant.
  A workspace grep of the artifact confirms no surviving `seconds(2)`,
  "two-second", `>= 2`, or "higher counter" text.
  Robustness note (not a finding): the only timing failure mode is the
  front-side margin — refresher construction to request-1 dispatch must
  stay under ~2 s of wall clock or the fast-path assertion flakes. That
  gap is wiremock-server-already-running plus a local `kube::Client`
  build, i.e. milliseconds in practice; 2 s of slack is acceptable for
  an integration test of this class. The back side is safe by
  construction (`tokio::time::sleep` never undershoots, so remaining
  lifetime is ≤ 9 s < 10 s leeway with ≥ 1 s slack).

- **MAJOR-adjacent flaw in step 4 (`refresher_produces_fresh_token_on_call`
  two-refresh precondition unstated) — RESOLVED.** The step now states the
  precondition explicitly: the canned closure returns `expires_at =
  Utc::now() + 5s` (inside the leeway) and the initial `TokenState` is
  also +5 s. Walked: refresh 1 fires (5 < 10), new expiry `now+5s`;
  refresh 2 re-checks `now+10 >= now+5` ⇔ true → fires again → two
  distinct tokens. Satisfiable and non-trivial. The companion
  `should_refresh` unit test's three cases (+9 s → true, +11 s → false,
  −1 s → true) match the spec-06 formula `Utc::now() + 10s >= expires_at`
  exactly.

- **MINOR (Slice B row omits `app_shell.rs`) — RESOLVED.** New dated Notes
  item acknowledges the row-level mismatch, cites the D/F/G-row precedent,
  and routes the correction as a dated additive amendment to spec 06 in
  the same finalize pass as the 0002-H2 Affected-files clarification.

- **MINOR (`set_token_expiry` false doc claim) — RESOLVED.** Step 5 now
  mandates future-tense wording ("will be populated by callers that hold
  an `EksTokenRefresher`"), states that `token_expiry` stays unpopulated
  post-slice-B, and the Notes item folds that gap into the queued spec 06
  clarification with the (a)/(b) migration resolution path.

- **MINOR (inert "fail-safe fallback" rationale) — RESOLVED.** Step 4 now
  states the static kubeconfig token is inert under the custom-service
  assembly (no `auth_layer` applied), explicitly forbids the "fail-safe
  fallback" description, and keeps the `insert`-not-`append` point.

- **MINOR (poisoned-lock contradiction) — RESOLVED.** The design note now
  documents the split semantics explicitly: request-path getters
  `.expect("lock poisoned")` (poisoning = unrecoverable invariant break),
  the writer maps its own `PoisonError` to
  `EksTokenRefreshError::LockPoisoned`. The sketch and the rationale are
  now consistent.

## Findings (round 3 — fresh)

- [MINOR] The round-2 spec-06 misattribution was corrected by addition,
  not by removal. Step 3 line 581 still reads "(spec 06 explicitly bounds
  UI edits to the mechanical call-site adjustments)", while eight lines
  later (:589-590) the same step correctly states "(the 'call-site
  adjustments' phrase in spec 06 line 595 belongs to 0005-H5, not to
  Slice B)". The false sentence directly contradicts the true one inside
  a single step. The prescribed action is identical either way, so
  executability is not compromised — but the stale parenthetical should
  be deleted, not merely answered. (Round-2 required change 3 asked to
  "fix the spec-06 misattribution in step 3"; this is half-done.)

- [MINOR] Standing from round 2 (was not in its required-changes list,
  still unaddressed): step 3's claim that `stop_event_watcher` "is
  already called by every `on_cluster_connection_lost` and
  cluster-disconnect path today" — it has exactly one caller
  (app_shell.rs:1927, the disconnect handler). Substance is fine; the
  quantifier is unverified. No text change in the revision.

## What verified cleanly this round (for the record)

- Verification-section 2a/2c restatements are now identical to step 6's
  parameterisation (12 s seed, 3 s sleep, token-0 → token-1, counter ==
  1) — the internal-consistency defect from round 2 is gone.
- Test-count accounting unchanged and still internally consistent:
  2 (step 1) + 2 (step 2 informer) + 3 (step 2 bridge) + 3 (step 4) +
  1 (step 6 integration) = +11.
- The revision touched only steps 3–6, Verification, and Notes; the
  round-1-resolved surfaces (dependency manifest, optional-token
  signature, `std::sync::RwLock` lock model, UI wiring through
  `informer_manager` / `resource_watch_cancels`, dead-`create_eks_client`
  mental model) were not regressed — re-spot-checked against the tree.
- Spec 06 conformance intact: 0002-H1 optional-token fix approach still
  verbatim; 0002-H2 acceptance 2a/2b/2c mapped to genuine, satisfiable
  tests; the wiremock-as-K8s-API substitution justification (accepted in
  kind at round 2) is untouched; the finalize amendment/clarification
  path respects spec 06's frozen status.

## Required changes (for FAIL)

None. Two MINORs above are quality nits for the finalize pass; neither
blocks development.

## Notes

The round-2 FAIL rode on one thing: the slice's flagship acceptance
evidence was arithmetically self-defeating. The revision fixes it with
exactly the parameterisation the round-2 eval prescribed (initial expiry
outside the leeway, fast-path first request, single refresh on the
second, counter == 1), extends the same rigour to step 4's two-refresh
test, and cleans up four of the five round-2 MINORs in substance. The
one remaining misattribution leftover (:581) is a stale-sentence
deletion, not a design question. No unaddressed MAJOR and no BLOCKER
remains; per the severity rule this passes, and per the round-counting
rule a PASS resolving the round-2 FAIL carries Round: 2.
