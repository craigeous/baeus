# Evaluation: slice-b-watch-cancellation-eks-refresh (code review)

Verdict: PASS
Round: 0
Reviewed against: slice-plan `.docs/slice-plans/slice-b-watch-cancellation-eks-refresh.md`
(Approved), spec `.docs/spec/06-remediation-highs.md` §0002-H1/§0002-H2, research
`.docs/research/0002-core-client-aws-review.md`, review-findings artifact
`.docs/evaluations/slice-b-watch-eks-review-findings.md`; diff `bf074dd..HEAD`
(branch `slice/b-watch-eks`, commits `b3f7dc3..400e4dd`).

## Gate (re-run by evaluator, not trusted)

| Step | Command | Result |
|------|---------|--------|
| format | `cargo fmt --check` | exit 0 |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | exit 0 (only pre-existing future-incompat note on third-party `block`/`proc-macro-error2`) |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` | exit 0 — **3,691 passed, 0 failed**; all 11 new slice tests confirmed ran and passed |
| deny | `cargo deny check` | exit 0 (advisories/bans/licenses/sources ok) |

Working tree clean (only untracked `.sdlc/`); commits author-neutral.

## Fidelity to plan & specs (step by step)

- **Step 0 (deps):** `tokio-util`/`tower`/`hyper`/`hyper-util` added to baeus-core,
  `wiremock` dev-dep; versions and features as planned, with one deviation (see
  [MINOR] #1). `tokio-util` also added to `baeus-ui` — required by step 3's
  `CancellationToken` use in `app_shell.rs`; spec 06 explicitly permits another
  crate needing it in the same slice.
- **Step 1 (H1 cancellation surface):** `watch_events_inner`/`watch_resources_inner`
  extracted taking the stream by value (`'static` for `tokio::spawn`), `tokio::select!
  { biased; _ = token.cancelled() => return Ok(()), ... }` — cancel wins within one
  poll cycle. Public signatures take `cancel: Option<CancellationToken>` with
  `unwrap_or_default()` — verbatim spec 06 "optional-not-required (default `None`
  -> new token constructed internally)". Existing match arms preserved unchanged
  (regression-proofed by the untouched pre-existing tests passing).
- **Step 2 (token storage):** `InformerEntry.cancel`, `set_cancel_token`,
  `cancel_token`, `stop_for_cluster` cancels attached tokens, `unregister` cancels
  before removal; bridge `WatcherEntry { informer_id, cancel }`, tuple-returning
  `register_watcher`, `stop_watching`/`stop_for_cluster` cancel. All 39 pre-existing
  bridge-test call sites migrated mechanically (tuple destructure). 5 new tests
  present and passing.
- **Step 3 (AppShell wiring):** one shared event token attached to all four standard
  informer ids (correct: a single `watch_events` loop backs them; `cancel()` is
  idempotent) and passed as `Some(token)` at the `watch_events` call site;
  `resource_watch_cancels: HashMap<ResourceListKey, CancellationToken>` added,
  populated in `start_resource_watcher` after the duplicate guard, threaded into
  `watch_resources` as `Some(token)`; cleanup closure removes the key from both
  maps in one update; `stop_event_watcher` retain-cancels all tokens matching the
  disconnected context. Matches the plan exactly.
- **Step 4 (EksTokenRefresher):** `std::sync::RwLock<TokenState>` + `tokio::sync::Mutex<()>`
  in-flight guard with post-acquire `should_refresh()` re-check (no double-presign);
  `SecretString` token state; `REFRESH_LEEWAY_SECS = 10` shared by tests and
  production; typed `EksTokenRefreshError::{PresignFailed, LockPoisoned}`; poison
  split as documented (readers `.expect`, writer maps to typed error);
  `clone_handle()` shares Arcs. `AuthRefreshLayer`/`AuthRefreshService` module-private,
  `Clone`-inner call pattern, `HeaderMap::insert` for `AUTHORIZATION`, single
  `expose_secret` into the header value. `create_eks_client` returns
  `(kube::Client, EksTokenRefresher)`, builds `rustls_https_connector()` +
  `base_uri_layer()` via `ConfigExt`, layers `AuthRefreshLayer` over the
  `hyper_util` legacy client. Kubeconfig static token retained and correctly
  documented as inert (auth_layer not applied).
- **Step 5 (cluster.rs):** only a doc comment on `set_token_expiry`, written in
  the future tense as the plan required. No behaviour change; no duplicate test.
- **Step 6 (wiremock acceptance test):** placed inline in `aws_eks.rs` rather than
  `tests/aws_eks_refresh_integration.rs` — justified in a comment (`AuthRefreshLayer`
  is module-private); the plan's test-count accounting (+11) is unaffected. The test
  implements the adjudicated timeline exactly: seed `expires_at = now+12s` → first
  real `kube::Client` request through the real middleware asserts
  `Authorization: Bearer token-0` and `counter == 0` (fast path) → 3 s wall-clock
  sleep (lands remaining lifetime at 9 s, inside the leeway) → second request
  asserts `Bearer token-1` → `counter == 1` (exactly one presign) →
  `Mock::expect(2)` bounds total requests.
- **Step 7 (docs cross-check):** no spec/ADR edits in the diff — correct; the
  queued spec-06 clarifications remain in the slice-plan Notes for the finalize
  pass, which owns them. Diff `.docs/` changes are limited to the slice-plan
  Status line (Approved → Implemented), the handoff update, and the orchestrator's
  review-findings artifact.
- **Scope:** 12 files, all inside the slice boundary; no drive-by edits; no
  spec/ADR mutation.

## Security invariants (verified mechanically in the real code)

- `create_eks_client` has zero production callers — `grep -Rn 'create_eks_client'
  crates/` returns the definition plus doc references only. Confirmed.
- Token material is `secrecy::SecretString` (zeroizing); `TokenState`/
  `EksTokenRefresher`/`AuthRefreshLayer` have no `Debug` derive; the single
  production `expose_secret` (aws_eks.rs) formats into the `Authorization`
  header only; test-side `expose_secret` calls are assertion-only.
  `EksTokenRefreshError` carries message strings, no token material.
- Fail-closed: `refresh()` error aborts the request via `?` before any send;
  no stale-token fallback path exists.
- TLS unchanged: production client uses kube `ConfigExt::rustls_https_connector()`
  with the cluster CA; the plain-HTTP connector appears only in the wiremock
  test against localhost. No cert-validation bypass anywhere.
- No new process invocation; presign remains in-process HMAC.

## Review-findings adjudication (per rubric)

- `/security-review` — status `ran-clean`. **Confirmed** by independent inspection
  (invariants above). No candidate findings to map.
- Security finder's noted cleanup race ("an ending watcher's cleanup removing a
  NEW watcher's token") — **rejected as a false positive.** The only removal
  path for `active_resource_watchers[K]` is the same serialized main-thread
  update closure that removes `resource_watch_cancels[K]` (app_shell.rs:2752-2753),
  and the duplicate-watch guard (app_shell.rs:2654) blocks any replacement
  watcher for K until that cleanup has run. No interleaving exists in which a
  stale cleanup can remove a new watcher's token; the two maps cannot diverge
  on that path.
- `/code-review` — status `skipped: command-unavailable`. Informational non-run;
  not read as a clean review and not treated as a finding.

## Findings

- [MINOR] `tokio-util` declared as `features = ["rt"]` in both Cargo.tomls,
  whereas slice-plan step 0 explicitly required `["rt", "sync"]` "so a future
  defaults change does not silently break the build." `sync` is a default
  feature today so the build is green, and spec 06's own dependency line
  prescribes `["rt"]` — hygiene-level deviation from the plan's stated
  robustness constraint.
- [MINOR] After `stop_event_watcher`, `active_resource_watchers` retains its
  keys until each cancelled watcher's own cleanup update runs, so an immediate
  same-key `start_resource_watcher` is briefly rejected by the duplicate guard.
  Self-heals on the next poll cycle; consistent with the plan's deferral of
  informer state-machine hardening to a later cycle.
- Planning flag (not a code finding): spec 06 0002-H2's Affected-files model
  (`create_client_from_path_with_aws_creds` "wires the refresher";
  "`token_expiry` field now populated") is inaccurate against the tree, as the
  approved slice-plan already documented. The delivered work satisfies 0002-H2's
  acceptance criteria 2a/2b/2c at `create_eks_client`'s stated boundary; the
  spec reconciliation belongs to the finalize clarification queue, not to this
  slice's code.

## Required changes (for FAIL)

None.

## Notes

Spec-boundary adjudication (requested item 4): spec 06 0002-H2 acceptance
criterion 2a is written against `create_eks_client`; the diff makes that exact
function correct at its own boundary and proves the criterion's observable
behavior through the real middleware stack (refresh inside the 10 s leeway,
exactly one presign, `Bearer token-0 → token-1`). The absence of production
callers was disclosed and adjudicated at plan-approval time with the finalize
clarification queued; it does not fail the criterion as written. Gate totals:
3,691 passed / 0 failed (baseline 3,680 + the slice's 11 new tests).
