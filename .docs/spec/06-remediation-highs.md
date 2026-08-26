# 06 — Remediation of High-Severity Findings

Status: Approved

This spec is the authoritative plan for remediating the seventeen (17)
high-severity findings surfaced by the deep review captured in research notes
`0002`, `0003`, `0004`, `0005`, and `0006`. It prescribes the fix approach,
affected files, acceptance criteria, and test expectations for each finding, and
groups those fixes into a slice breakdown for sequential development. It does
**not** touch medium- or low-severity findings — those await a later planning
cycle.

## Authority

- ADR 0002 (`kube-rs` client) — bounds the watch/informer surface being repaired.
- ADR 0003 (`alacritty_terminal` + `portable-pty`) — mandates the terminal
  integration that finding 0005-H5 remediates.
- ADR 0004 (Helm hybrid CLI + kube-rs) — mandates the subprocess that finding
  0005-H3 remediates.
- ADR 0005 (Plugin dylib loading) — mandates the loader wiring and null-safety
  that findings 0005-H1 and 0005-H2 remediate.
- ADR 0006 (Tokio runtime) — bounds the async cancellation shape used by
  findings 0002-H1 and 0002-H2.
- Constitution non-negotiables in `.docs/spec/README.md`: **test-first**
  (0002-H4, 0006 test additions), **performance & responsiveness / virtual
  scrolling for large lists** (0003-H2, 0004-H1), **dependencies audited on
  every CI run** (0006-H1).
- Research authority: `.docs/research/0002-core-client-aws-review.md`,
  `.docs/research/0003-ui-layout-shell-review.md`,
  `.docs/research/0004-ui-components-review.md`,
  `.docs/research/0005-supporting-crates-review.md`,
  `.docs/research/0006-quality-infra-review.md` (all Approved 2026-08-25).

## Decisions

**No new ADR is required.** Every high-severity finding is a conformance fix,
a bug fix, or a hygiene/CI change against an already-accepted decision or the
constitution. Specifically:

- 0002-H1 (watch cancellation) and 0002-H2 (EKS token refresh) are mechanics
  for delivering behaviour ADR 0002 already implies. The chosen shapes
  (`CancellationToken` for watch loops, refresh-before-expiry callback for
  EKS clients) are constrained by kube-rs's existing tower/auth surface — they
  are pattern selection, not novel design with meaningful alternatives.
- 0002-H3 and 0002-H4 are a bug fix (silent no-op) and a test-first gap under
  the constitution.
- 0003-H1 (app_shell decomposition) uses the existing sub-struct pattern
  already established by `SidebarState`, `DockState`, `WorkspaceState` — a
  refactor, not a new decision. 0003-H2 (table virtualization) mirrors the
  `uniform_list` pattern already used by the navigator sidebar.
- 0004-H1 (log virtualization) mirrors the same navigator pattern. 0004-H2 is
  a bug-guard promotion.
- 0005-H1, H2, H3 are pure conformance to ADRs 0005 and 0004 respectively.
  0005-H4 is a memory-safety guard. 0005-H5 is conformance to ADR 0003, whose
  Rationale/Consequences already specify the integration architecture
  ("wrap `Term` with a custom event listener, PTY via portable-pty, grid to
  GPUI; kube-rs WebSocket pipes through the emulator for pod exec"). No open
  integration question remains.
- 0006-H1 is direct constitution compliance ("Dependencies audited for known
  vulnerabilities on every CI run"). 0006-H2, H3, H4 are CI hygiene against
  existing tooling policy.

If, during slice-planning, any of these fixes surfaces a genuine open decision
(e.g., an unforeseen alacritty integration branch, or a token-refresh model
whose consequences cross ADR 0002's boundary), the planner must pause and
raise a new ADR before the slice-plan proceeds.

## Design

### Descriptive-prescriptive scope

This is a planning artifact; the sections below prescribe fixes. Each section
cites the originating research finding by note ID and finding number (e.g.
`0002-H1`) and states the fix approach, the crates/files affected, acceptance
criteria, and the tests expected. No section requires more than the changes
listed — the descriptive-prescriptive boundary applies to the *shape* of the
fix, not to unrelated cleanups a developer might notice in the same file.

### 0002-H1 — Cancellation surface on `watch_events` / `watch_resources`

**Finding:** `crates/baeus-core/src/client.rs:1181,1246` — infinite `while let
Some(...)` watch loops backed by `default_backoff()` have no cancellation
input; disconnecting a cluster leaves the loops running until the outer task
is aborted with no deterministic teardown.

**Fix approach:** Extend both `watch_events` and `watch_resources` with an
optional `tokio_util::sync::CancellationToken` parameter. `tokio-util` is not
currently a direct dependency of `baeus-core` (verified against
`crates/baeus-core/Cargo.toml`: only `tokio.workspace = true` is declared;
`tokio-util` appears in `Cargo.lock` transitively but that does not make it
importable). Add `tokio-util = { version = "*", features = ["rt"] }` to
`crates/baeus-core/Cargo.toml` under `[dependencies]` (or promote to the
workspace `[workspace.dependencies]` if another crate needs it in the same
slice). Inside each loop, replace
the bare `while let Some(...)` with a `tokio::select!` that races the stream
next-item against `token.cancelled()`, breaking cleanly on cancellation.
`ResourceWatchBridge::register_watcher`
(`crates/baeus-core/src/watch.rs`) stores the token alongside each watcher
entry; `stop_watching` and `stop_for_cluster` call `token.cancel()` on the
matching entry. Existing callers that pass no token get a
`CancellationToken::new()` created at the call site so the public surface is
optional-not-required (default `None` -> new token constructed internally, or
overload the entry points).

**Affected files:**
- `crates/baeus-core/src/client.rs` — `watch_events`, `watch_resources`.
- `crates/baeus-core/src/watch.rs` — `ResourceWatchBridge::register_watcher`,
  `stop_watching`, `stop_for_cluster`, and `WatcherEntry` (add a
  `CancellationToken` field).
- `crates/baeus-core/src/informer.rs` — only if `InformerEntry` needs to hold
  the token for stop-by-key routing (see 0002-H1 scope note below).
- `crates/baeus-core/Cargo.toml` — dependency add if missing.

**Scope note:** Associating an `AbortHandle` with each `InformerEntry`
(candidate opportunity F7 in research 0002) is a *medium*-severity item and
therefore **out of scope for this spec**. This slice adds the cancellation
surface only; the state-machine hardening is a later cycle.

**Acceptance criteria:**
- Cancelling the token stops the watch loop within one poll cycle (test
  asserts observable via a spawned task's `JoinHandle` completing).
- `ResourceWatchBridge::stop_watching` cancels the associated loop.
- Public API of `watch_events` / `watch_resources` remains backward compatible
  for callers that do not care about cancellation.

**Test expectations:**
- New `#[tokio::test]` in `crates/baeus-core/src/client.rs` (or a
  `client_watch.rs` sibling test module) that spawns `watch_events` with a
  mock kube client feeding no items, cancels the token, and asserts the loop
  returns within a bounded timeout.
- Corresponding test for `watch_resources`.
- No decrease in existing test count.

### 0002-H2 — EKS bearer token 60-second TTL with no refresh path

**Finding:** `crates/baeus-core/src/aws_eks.rs:774,832-873` — the presigned
STS token embedded in the `Kubeconfig` for the built kube client expires 60
seconds after `create_eks_client` returns, and no refresh mechanism exists.
`ClusterConnection.token_expiry` (`cluster.rs:39`) is present but nothing
watches it.

**Fix approach:** Change `create_eks_client` to return not only the
`kube::Client` but also a `TokenExpiry { expires_at: DateTime<Utc>, refresh:
Arc<dyn Fn() -> Future<Output=Result<SecretString>> + Send + Sync> }` handle
(or an equivalent typed struct). Wire the refresh into the kube client via
kube-rs's `AuthLayer`/tower middleware so that when a request is about to be
sent within 10 seconds of `expires_at`, the middleware calls `refresh` to
regenerate the presigned token before the send. The refresh closure captures
the AWS `SdkConfig` and the cluster ARN so it can call
`build_eks_presigned_token` again.

**Affected files:**
- `crates/baeus-core/src/aws_eks.rs` — `create_eks_client`,
  `build_eks_presigned_token`, plus a new `EksTokenRefresher` struct and
  `AuthLayer` wiring.
- `crates/baeus-core/src/cluster.rs` — `ClusterConnection` records the
  expiry/refresher for observability; `token_expiry` field now populated.
- `crates/baeus-core/src/client.rs` — `create_client_from_path_with_aws_creds`
  wires the refresher through when constructing EKS clients.

**Acceptance criteria:**
- A `kube::Client` produced by `create_eks_client` succeeds on API calls made
  more than 60 seconds after construction (test uses a clock override or
  mock/wiremock STS backend to advance simulated time and observe a second
  presign).
- If `refresh` fails, the error surfaces as a typed
  `EksTokenRefreshError` rather than as an opaque 401.
- No behavioural change for callers who use the client immediately (fast path
  bypasses the refresher check when `expires_at` is comfortably in the future).

**Test expectations:**
- New `#[tokio::test]` under `crates/baeus-core/src/aws_eks.rs` (or
  `tests/aws_eks_refresh.rs` in `baeus-core`) using the AWS SDK's smithy mock
  transport or a wiremock STS to prove a second `presigned_token` request is
  issued when the client makes a call after expiry.
- Unit test: `EksTokenRefresher::should_refresh` returns true when
  `Utc::now() + 10s >= expires_at`, else false.
- No decrease in existing test count.

### 0002-H3 — Silent no-op when exec block is absent in credential injection

**Finding:** `crates/baeus-core/src/aws_sso.rs:80-81,139-140` —
`inject_aws_profile_into_kubeconfig` and
`inject_aws_credentials_into_kubeconfig` return `Ok(())` when
`auth_info.exec` is `None`, hiding a misconfiguration until it later surfaces
as a confusing 401 or TLS error from the API server.

**Fix approach:** Introduce a `KubeconfigInjectionError` variant
`ExecBlockMissing { context: String }` in the crate's error enum (find the
existing error type in `aws_sso.rs` or `error.rs`). Change both inject
functions to return `Err(ExecBlockMissing { context })` when the exec block
is absent. `create_client_from_path_with_aws_creds`
(`crates/baeus-core/src/client.rs:199`) propagates the error and, at the UI
boundary, surfaces it as a diagnostic message that names the missing exec
block so the user can fix the kubeconfig.

**Affected files:**
- `crates/baeus-core/src/aws_sso.rs` — both inject functions and the
  error enum they return.
- `crates/baeus-core/src/client.rs` — error propagation in
  `create_client_from_path_with_aws_creds`.

**Acceptance criteria:**
- Calling either inject function with a context lacking an exec block returns
  an explicit typed error, not `Ok(())`.
- Existing callers that expect `Ok(())` on the happy path are unaffected.
- Error message identifies the offending context name.

**Test expectations:**
- New unit tests in `aws_sso.rs` cover both inject functions with (a) a valid
  exec-block kubeconfig, (b) a kubeconfig where the target context has no
  exec block. Each asserts the correct `Result` variant.

### 0002-H4 — Zero async tests for AWS SDK paths

**Finding:** No `#[tokio::test]` coverage of `sso_register_client`,
`sso_start_device_auth`, `sso_poll_for_token`, `sso_list_accounts`,
`sso_get_role_credentials`, `authenticate_with_access_key`, `assume_role`,
`create_eks_client`, or `generate_eks_token` — the most fragile
user-facing flow is untested (`0002` note §4).

**Fix approach:** Add an async test harness in `crates/baeus-core/tests/` (a
new `aws_wizard_smoke.rs` integration test file, plus per-function inline
tests where the function is easily isolated) using the AWS SDK's smithy mock
transport (or wiremock as fallback). Cover, at minimum, the flows listed in
research 0002's candidate opportunity for §4: device-auth happy path,
`AuthorizationPendingException` retry, token exchange, cluster discovery, and
`create_eks_client` round-trip (which now includes the refresher from
0002-H2).

**Affected files:**
- `crates/baeus-core/tests/aws_wizard_smoke.rs` — new integration test file.
- `crates/baeus-core/src/aws_sso.rs`,
  `crates/baeus-core/src/aws_eks.rs` — minimal API surface tweaks
  needed only to make the SDK client configurable/injectable for the mock
  transport (constructor-injection or a `SdkConfig` parameter).
- `crates/baeus-core/Cargo.toml` — `[dev-dependencies]` addition for the
  chosen mock (prefer AWS smithy mock; fall back to `wiremock` only if the
  smithy mock cannot cover a given call).

**Acceptance criteria:**
- At least one `#[tokio::test]` exists for every function listed above.
- The device-auth polling test asserts the retry-on-pending behaviour.
- Tests run under the existing `cargo nextest` gate without requiring live
  AWS credentials or network access.

**Test expectations:** New tests contribute to the workspace test count and
run within CI's existing runtime budget.

### 0003-H1 — `app_shell.rs` god-object decomposition

**Finding:** `crates/baeus-ui/src/layout/app_shell.rs` is 11,692 lines with
60+ fields and 8+ `impl AppShell` blocks covering drag state, YAML editor
state, AWS SSO state, PTY handles, and more (0003 note §1).

**Fix approach:** Extract the following sub-structs, each defined in its own
module under `crates/baeus-ui/src/layout/` (mirroring the existing
`SidebarState`, `DockState`, `WorkspaceState` pattern):

- `DragState` — dock/column/sidebar/topology drag coordinates (`is_dragging_*`
  fields at `app_shell.rs:511-611`).
- `EksConnectionState` — `eks_wizard`, `eks_cluster_data`, `pending_sso_login`
  and their helpers (`app_shell.rs:617-635`).
- `ClusterAppearanceStore` — cluster appearance persistence
  (`persist_cluster_appearances`, `load_cluster_appearances`, palette
  constants).
- `PtyState` — `pty_processes`, `pty_output_buffers` (`app_shell.rs:508-510`).
- `YamlEditorState` — `yaml_editors`, `yaml_editor_focus_handles`
  (`app_shell.rs:557-559`).

`AppShell` owns one instance of each. `impl AppShell` methods that only touch
one of these areas move to `impl <SubStruct>` methods; methods that need the
whole `AppShell` remain on `AppShell` but delegate reads/writes into the
sub-structs. The `impl AppShell` in `pod_detail_render.rs` pattern remains the
model for auxiliary rendering modules — do not force it into the sub-struct
split.

**Scope boundaries:**
- The extraction is a refactor: no observable behaviour changes; no new
  features; no API changes visible outside the `baeus-ui` crate.
- Line count is not itself an acceptance criterion, but the `AppShell` struct
  must have **at most 25 direct fields** post-extraction (down from 60+).
- Existing tests continue to pass without modification; the file split may
  require re-homing tests that today live inline in `app_shell.rs`.

**Acceptance criteria:**
- Each new sub-struct compiles independently of `app_shell.rs` (no cyclic
  imports).
- `AppShell` field count ≤ 25.
- `cargo clippy --workspace -- -D warnings` clean.
- `RUST_MIN_STACK=268435456 cargo test --workspace` count unchanged or
  higher.

**Test expectations:**
- Existing UI integration tests in `crates/baeus-ui/tests/*.rs` (55+ files)
  continue to pass.
- Each new sub-struct has at least one focused unit test (constructor,
  primary state transition), added under
  `crates/baeus-ui/src/layout/<sub>.rs` inline or a matching integration test
  file if inline hits the GPUI proc-macro stack limit.

### 0003-H2 — Resource table body has no virtualization

**Finding:** `render_resource_table_body_filtered`
(`crates/baeus-ui/src/layout/app_shell.rs:7992-8042`) renders every filtered
row via a `.child()` loop capped at 200 — no virtualization despite the
navigator sidebar demonstrating `uniform_list` at `app_shell.rs:5029`.

**Fix approach:** Replace the eager for-loop with a `uniform_list` fed by a
cached `Vec<TableRow>` on the `AppShell` (or a new
`ResourceTableViewState` sub-struct — coordinate with 0003-H1 if that slice
lands first). Remove the hard cap of 200 rows. Header row remains a fixed
element above the `uniform_list`. Follow the navigator pattern for
`cx.entity().downgrade()` closures.

**Affected files:**
- `crates/baeus-ui/src/layout/app_shell.rs` —
  `render_resource_table_body_filtered` and any callers that assumed the
  200-row ceiling.

**Acceptance criteria:**
- Rendering a resource list with 5,000 filtered rows does not build 5,000
  element subtrees per frame (verified by inspecting the `uniform_list`
  callback invocation count in a test).
- The 200-row hard cap is removed.
- Scrolling behaves visually identically to the navigator sidebar (scrollbar,
  overscroll handling).

**Test expectations:**
- New UI-level test in `crates/baeus-ui/tests/` that constructs a table
  state with a large row count and asserts the `uniform_list` item-count
  matches the row-count without eager materialisation (probe via a callback
  counter, since GPUI headless rendering is not available per note in
  research 0005-F16).

### 0004-H1 — Log viewer renders all lines eagerly

**Finding:** `render_log_body` (`crates/baeus-ui/src/components/log_viewer.rs:918-958`)
materialises every one of up to 10,000 lines per frame with a `for` loop; no
virtualization.

**Fix approach:** Replace the eager loop with `uniform_list`, feeding it the
`state.visible_lines()` slice via a downgraded entity handle. Preserve
line-number gutter alignment and level-color logic per line. Auto-scroll
behaviour when a new line arrives at the tail must continue to work (scroll
`uniform_list` to the last item on push when the user has not scrolled up).

**Affected files:**
- `crates/baeus-ui/src/components/log_viewer.rs` — `render_log_body` and any
  helpers it inlines.

**Acceptance criteria:**
- Streaming 10,000 lines does not create 10,000 element subtrees per frame.
- Auto-scroll-on-tail continues to work.
- ANSI stripping (`strip_ansi_escapes`) still applies before render.
- Search-match highlighting still applies to the visible slice.

**Test expectations:**
- New test in `crates/baeus-ui/tests/log_viewer_virtualization.rs` (or
  inline if under the 300-line proc-macro budget) covering: push 10,000
  lines, assert `uniform_list` reports the expected item count, assert
  auto-scroll flag flips on tail push.

### 0004-H2 — Column/cell count mismatch silently passes in release builds

**Finding:** `json_to_table_row`
(`crates/baeus-ui/src/components/json_extract.rs:187-193`) uses
`debug_assert_eq!` to guard the extractor-vs-columns count match — compiled
out in release, silently rendering broken rows if the two files drift.

**Fix approach:** Promote the guard from `debug_assert_eq!` to an
`assert_eq!` in release builds **and** add a compile-time enumeration test
that exercises `resource_table::columns_for_kind`
(`crates/baeus-ui/src/components/resource_table.rs:674` — the definition
imported by `json_extract.rs:9`, not the duplicate at
`crates/baeus-ui/src/views/resource_list.rs:230`) and each extractor for
all 34 resource kinds, asserting the vecs are the same length. The runtime
`assert_eq!` is the belt-and-braces defence for kinds discovered at runtime
(CRDs); the test is the primary gate for the fixed set.

**Drift-hazard note:** The duplicate `columns_for_kind` in
`views/resource_list.rs` is itself exactly the class of drift the
finding guards against; it is a **medium**-severity item (dedup /
consolidation) and remains out of scope for this spec. The new enumeration
test intentionally binds to `resource_table::columns_for_kind` by explicit
path so a later dedup slice does not have to touch this test.

**Affected files:**
- `crates/baeus-ui/src/components/json_extract.rs` — replace
  `debug_assert_eq!` with `assert_eq!` including a clear panic message
  (kind name, column count, cell count).
- `crates/baeus-ui/tests/json_extract_columns_match.rs` (new) — iterate over
  every resource kind covered by `resource_table::columns_for_kind` and
  assert the extractor cell count matches.

**Acceptance criteria:**
- The new test fails if any kind's extractor and columns disagree.
- A release build panics (not silently truncates) on mismatch for
  runtime-discovered kinds.
- Existing tests still pass.

**Test expectations:** New integration test file listed above adds 34
individual assertions (one per kind), contributing to the workspace test
count.

### 0005-H1 — Plugin loader not connected to the app

**Finding:** `PluginLoader`, `PluginRegistry`, `SandboxedLoader` are never
called from `baeus-ui` or `baeus-app`; only the `Plugin` struct is consumed
by the plugin manager UI (0005 note §1). ADR 0005 requires plugin discovery
and loading at runtime.

**Fix approach:** Wire `PluginLoader::scan_directory` + `PluginLoader::load`
into startup in `baeus-app` (or the appropriate `AppShell::new` path in
`baeus-ui`). The plugin manager UI already renders `Plugin` values; feed
those values from the loader instead of an empty vec. Permission confirmation
UI is a separate feature (ADR 0005 mentions it) — for this slice, wire only
the discovery + load with permissions defaulting to their manifest-declared
values; the confirmation dialog is out of scope.

**Affected files:**
- `crates/baeus-app/src/` — startup wiring (identify actual entry file
  during slice-plan; likely `main.rs` or a startup helper).
- `crates/baeus-ui/src/views/plugin_manager.rs` — read from the loader
  registry instead of a placeholder.
- `crates/baeus-ui/src/layout/app_shell.rs` — inject the registry handle
  into `AppShell` (may coordinate with 0003-H1 sub-structs).

**Acceptance criteria:**
- Placing a compatible `.dylib` in the plugin directory results in the
  plugin appearing in the plugin manager UI after startup.
- No plugin file / empty directory: startup completes cleanly, no crash.
- Failure to load a plugin (bad ABI, wrong version) is surfaced as a
  `PluginError` in the UI, not a panic.

**Test expectations:**
- New integration test in `crates/baeus-plugins/tests/` that constructs a
  temporary directory, drops a fixture manifest into it, calls
  `PluginLoader::scan_directory`, and asserts the returned plugins.
- Any manual/end-to-end verification steps are documented in the slice-plan
  test plan but are not required for the automated gate.

### 0005-H2 — `Box::from_raw(create_fn())` without null check

**Finding:** `crates/baeus-plugins/src/loader.rs:178` — no null-pointer
check before `Box::from_raw`, producing UB if a plugin's
`_baeus_plugin_create` returns null (0005 note §2).

**Fix approach:** Before `Box::from_raw(ptr)`, guard with `if ptr.is_null()
{ return Err(PluginError::AbiViolation { reason: "create_fn returned null"
}); }`. Document the ABI contract on the exported symbol in `loader.rs`.

**Affected files:**
- `crates/baeus-plugins/src/loader.rs` — the `unsafe` block and its
  surrounding function; add the null check and error variant.
- Add a `PluginError::AbiViolation { reason: String }` variant if not
  present.

**Acceptance criteria:**
- A test fixture that returns `std::ptr::null_mut()` from a stub
  `_baeus_plugin_create` triggers `PluginError::AbiViolation`, not UB.
- Clippy remains clean.

**Test expectations:**
- Unit test in `crates/baeus-plugins/src/loader.rs` (or a `#[cfg(test)]`
  helper crate under `crates/baeus-plugins/tests/`) that dynamically loads a
  fixture dylib returning null and asserts the typed error. If dylib
  fixture is impractical in the workspace, a white-box helper that takes a
  `*mut c_void` and runs the same guard is acceptable, provided the guard
  itself is exercised by a real code path.

### 0005-H3 — Helm CLI subprocess not implemented

**Finding:** `HelmOperation::to_args()`
(`crates/baeus-helm/src/operations.rs:33-115`) constructs argv but nothing
calls `std::process::Command` to execute — `HelmCommandResult` is defined
but never populated (0005 note §3). ADR 0004 mandates shelling out to
`helm` for mutating operations.

**Fix approach:** Implement
`execute_helm_operation(op: HelmOperation) -> Result<HelmCommandResult,
HelmError>` in `crates/baeus-helm/src/operations.rs`, spawning
`std::process::Command::new("helm")` (or `tokio::process::Command` for the
async variant — pick one at slice-plan time; async is preferred to keep the
GPUI main thread unblocked). On successful spawn, capture stdout/stderr and
populate `HelmCommandResult`. On `Command::spawn` failure (helm not
installed), return a typed `HelmError::CliNotFound` with guidance for the UI
layer (ADR 0004 Consequences: "handle CLI absence gracefully with user
guidance").

**Affected files:**
- `crates/baeus-helm/src/operations.rs` — new `execute_helm_operation` and
  `HelmError` variants.
- `crates/baeus-helm/Cargo.toml` — add `tokio` (features `process`) if async
  form is chosen and not already present via workspace inheritance.
- Callers in `crates/baeus-ui/src/` that today produce a `HelmOperation`
  route it through the new executor.

**Acceptance criteria:**
- `execute_helm_operation` returns `Ok(HelmCommandResult)` when the mocked
  `helm` binary succeeds.
- Returns `Err(HelmError::CliNotFound)` when `helm` is not on `PATH`.
- Returns `Err(HelmError::NonZeroExit { code, stderr })` on non-zero exit.
- Executor never runs on the GPUI main thread (callers use `cx.spawn` /
  `tokio_handle.spawn`).

**Test expectations:**
- Unit test using a shell-script stub as `helm` on a temp `PATH` (or
  `tempfile`-based binary) that exits 0 with a known payload — assert the
  parsed result.
- Unit test asserting `HelmError::CliNotFound` when `PATH` is empty.

### 0005-H4 — LCS diff allocates O(m×n) memory without guard

**Finding:** `crates/baeus-editor/src/diff.rs:100-103` — the O(m×n)
allocation `vec![vec![0usize; n + 1]; m + 1]` is inside
`longest_common_subsequence` (called by `compute_diff`) with no line-count
check. Two 10,000-line YAML manifests hit ~800 MB.

**Fix approach:** Add a `MAX_LCS_LINES` constant (5,000 per candidate
opportunity in 0005 note) and a guard at the top of `compute_diff` (the
public entry) so the check runs before the LCS helper is called. When
either side exceeds the limit, return a `DiffMode::Truncated` variant (or a
`Result::Err(DiffError::TooLarge { .. })`, decision at slice-plan) so
callers can render a "diff too large — showing hunks only" view rather than
allocating gigabytes. A hunk-only fallback is a bigger project — for this
slice, the guard returning an explicit error is sufficient.

**Affected files:**
- `crates/baeus-editor/src/diff.rs` — the guard at `compute_diff`, the
  constant, and the new error variant. (`longest_common_subsequence` itself
  is not modified.)
- Callers in `crates/baeus-ui/src/` that today assume `compute_diff` never
  errs — propagate the new error and render a placeholder message.

**Acceptance criteria:**
- `compute_diff` with 5,001+ lines returns the guarded error without
  allocating the DP table.
- Below-limit inputs behave identically to today.
- Clippy clean.

**Test expectations:**
- Unit test asserting the error variant fires at 5,001 lines on either side.
- Unit test asserting normal behaviour at 4,999 lines.
- No decrease in existing test count.

### 0005-H5 — alacritty_terminal not integrated; custom parser used

**Finding:** `crates/baeus-terminal/src/emulator.rs:1-7,292-298,501-513` —
custom partial ANSI parser with UTF-8 gaps, simplified IL/DL, no sixel/iTerm2
image support. ADR 0003 mandates `alacritty_terminal + portable-pty`.

**Fix approach:** Replace the custom `TerminalEmulator` with an
alacritty_terminal-backed implementation per ADR 0003's architecture note:
wrap `alacritty_terminal::Term<L>` behind a custom `EventListener`, drive
input via `portable-pty` (already partially in `pty_process.rs`), and expose
a grid snapshot for GPUI rendering. Existing `PtyProcess` and `PtySession`
integration is preserved; the custom parser and grid types in `emulator.rs`
are removed.

The kube-rs WebSocket exec bridge (ADR 0003 Rationale, and 0005 note §7 —
medium severity) is **out of scope for this spec** — this slice replaces the
emulator core only. Local shell PTY continues to work through the existing
`spawn_shell` path.

**Affected files:**
- `crates/baeus-terminal/src/emulator.rs` — full replacement.
- `crates/baeus-terminal/src/lib.rs` — re-exports.
- `crates/baeus-terminal/Cargo.toml` — add `alacritty_terminal` if not
  present; retain `portable-pty`.
- `crates/baeus-ui/src/components/` — call-site adjustments where the UI
  reads the emulator grid.

**Acceptance criteria:**
- `alacritty_terminal::Term` is the driving state machine (custom parser
  removed).
- Multi-byte UTF-8 characters render correctly (test with a Japanese and
  emoji input line).
- Existing local-shell PTY session tests pass without modification (adjust
  test API only if signatures shift).

**Test expectations:**
- New unit test feeding a UTF-8 byte stream through the emulator and reading
  the resulting grid.
- Existing tests in `crates/baeus-terminal/src/*.rs` continue to pass.

### 0006-H1 — `cargo-deny` never runs in CI

**Finding:** `deny.toml` exists but no `cargo deny check` step exists in
`ci.yml` or `release.yml` (0006 note §1). Constitution requires dependencies
audited on every CI run.

**Fix approach:** Add a `cargo deny check` step to `.github/workflows/ci.yml`
(after `cargo fmt --check` from 0006-H2, before or after clippy — order is
not critical as long as it runs). Install cargo-deny via
`taiki-e/install-action@cargo-deny` (pinned per 0006 note §7 — but SHA
pinning is a medium finding, not required by this slice — use the same
`@vN` pattern as other Actions in the file for now to keep the change
focused).

**In-scope `paths:` filter extension.** The current `on.pull_request.paths:`
filter at `.github/workflows/ci.yml:6-9` is `crates/**`, `Cargo.toml`,
`Cargo.lock` only. For the new deny gate to actually fire on the changes
that can invalidate it, the filter must additionally trigger on `deny.toml`
and on `.github/workflows/**`. This spec pulls **exactly that minimal
extension** into slice A as an intrinsic part of 0006-H1 — without it the
acceptance criterion below is unachievable. Any broader trigger-path
redesign (e.g., adding docs paths, tests-only paths, or restructuring the
trigger graph) is the substance of medium finding 0006 §9 and remains
deferred; 0006 §9 is therefore **partially remediated by slice A** and
listed as such in Out of scope.

**Affected files:**
- `.github/workflows/ci.yml` — deny step, install action, and the two
  `paths:` filter additions (`deny.toml`, `.github/workflows/**`).

**Acceptance criteria:**
- The `on.pull_request.paths:` filter includes `crates/**`, `Cargo.toml`,
  `Cargo.lock`, `deny.toml`, and `.github/workflows/**`, and every PR that
  touches any of those paths triggers a `cargo deny check` run.
- A new RUSTSEC advisory that hits the workspace's transitive deps causes CI
  to fail (verified by running `cargo deny check` locally against the
  current `Cargo.lock`).

**Test expectations:** No new unit tests; the verification is CI itself.

### 0006-H2 — No `rustfmt --check` in the PR gate

**Finding:** `ci.yml:24` installs only `components: clippy`; no `cargo fmt
--check` step (0006 note §2). `rustfmt.toml` is authoritative but not
enforced.

**Fix approach:** Add `rustfmt` to the `components:` list in the toolchain
setup step in `.github/workflows/ci.yml`, then add `cargo fmt --check`
(equivalent: `cargo fmt --all -- --check`) as the first gate step, before
clippy and tests. Order matches the spec 03 gate.

**Affected files:**
- `.github/workflows/ci.yml`.

**Acceptance criteria:**
- Any PR whose files violate `rustfmt.toml` fails CI.
- Existing tree passes `cargo fmt --check` today (verify before merging the
  slice; if it does not, add a fmt commit within the same slice).

**Test expectations:** Verification is CI itself.

### 0006-H3 — CI PR gate is macOS-only

**Finding:** `ci.yml:16` — `runs-on: macos-14`. Linux and Windows only
appear in `release.yml`, which runs post-merge (0006 note §3).

**Fix approach:** Convert the `check` job to a matrix over
`{ macos-14, ubuntu-latest, windows-latest }` and gate the PR on all three.
Reuse the same install and cache steps; keep `RUST_MIN_STACK` on every step.
Windows-specific tolerances (line-endings, path separators) should surface
as build failures the first time this lands — that is the intended point of
adding the gate.

**Affected files:**
- `.github/workflows/ci.yml`.

**Acceptance criteria:**
- A PR fails when Linux or Windows fails, even if macOS passes.
- Cache keys per-OS remain effective (`Cargo.lock` hash + `runner.os`).

**Test expectations:** Verification is CI itself. Any per-OS test
adjustments discovered on the first run are additive to this slice's
acceptance.

### 0006-H4 — Release version hardcoded; three tag generators risk split-brain

**Finding:** `release.yml:74-79,191-196,274-279` each independently generate
`v0.1.0-dev.${DATE}.${SHORT_SHA}` — `0.1.0` is literal, `DATE` differs
across runners, `Cargo.toml` version has no effect on tags (0006 note §4).

**Fix approach:** Add a single `compute-tag` job at the top of
`release.yml` that reads the workspace version via `cargo metadata
--no-deps --format-version 1 | jq -r '.packages[0].version'` (or `cargo
pkgid`), fixes `DATE` and `SHORT_SHA` once, and emits them as job outputs.
The three downstream jobs consume the outputs instead of computing their
own. This eliminates both the split-brain risk and the hardcoded `0.1.0`.

**Affected files:**
- `.github/workflows/release.yml`.

**Acceptance criteria:**
- All release artifacts share the same tag.
- Bumping the workspace version in `Cargo.toml` results in the release tag
  reflecting the new version.
- The three per-runner "Generate release tag" steps are removed.

**Test expectations:** Verification is CI itself; a dry-run push to a
throwaway branch validates the new job wiring before landing.

## Slice Breakdown

Slices are **sequentially ordered so file overlaps do not cause conflicts**;
they are near-disjoint on primary files but not mechanically disjoint. The
known overlaps (all acceptable under the stated ordering) are:

- **B and C** share `crates/baeus-core/src/aws_eks.rs`,
  `crates/baeus-core/src/client.rs`, and `crates/baeus-core/Cargo.toml`.
  Order **B → C** is required: C's async wizard tests exercise the
  `create_eks_client` shape and error-propagation surface that B
  introduces.
- **D, F, and G** all touch `crates/baeus-ui/src/layout/app_shell.rs`. D
  injects a registry handle (0005-H1); F edits a single method,
  `render_resource_table_body_filtered`; G is the wider decomposition.
  Order **D → F → G** is required: G's decomposition considers the
  registry-handle injection and the virtualized table body as part of its
  input state.

Order chosen so that shared-dependency slices land before their consumers.

| # | Slice | Highs remediated | Primary crates / files | Rough size |
|---|-------|-------------------|-------------------------|------------|
| A | CI & release hygiene | 0006-H1, H2, H3, H4 | `.github/workflows/ci.yml`, `.github/workflows/release.yml` | S |
| B | Core watch cancellation + EKS token refresh | 0002-H1, H2 | `crates/baeus-core/src/{client.rs, aws_eks.rs, cluster.rs, watch.rs}`, `crates/baeus-core/Cargo.toml` | M |
| C | AWS credential injection error + async wizard tests | 0002-H3, H4 | `crates/baeus-core/src/{aws_sso.rs, client.rs, aws_eks.rs}` (error propagation + mock-injection tweaks), `crates/baeus-core/tests/aws_wizard_smoke.rs` (new), `crates/baeus-core/Cargo.toml` (dev-deps) | M |
| D | Editor / Plugin / Helm safety & wiring | 0005-H1, H2, H3, H4 | `crates/baeus-plugins/src/loader.rs`, `crates/baeus-helm/src/operations.rs`, `crates/baeus-editor/src/diff.rs`, `crates/baeus-app/src/main.rs` (or equivalent startup), `crates/baeus-ui/src/views/plugin_manager.rs`, `crates/baeus-ui/src/layout/app_shell.rs` (registry-handle injection only) | M |
| E | Terminal emulator: alacritty_terminal integration | 0005-H5 | `crates/baeus-terminal/src/emulator.rs`, `crates/baeus-terminal/Cargo.toml` | M-L |
| F | UI virtualization & table integrity | 0003-H2, 0004-H1, 0004-H2 | `crates/baeus-ui/src/layout/app_shell.rs` (table body only), `crates/baeus-ui/src/components/log_viewer.rs`, `crates/baeus-ui/src/components/json_extract.rs`, `crates/baeus-ui/tests/json_extract_columns_match.rs` (new) | M |
| G | `app_shell.rs` decomposition | 0003-H1 | `crates/baeus-ui/src/layout/app_shell.rs`, new `crates/baeus-ui/src/layout/{drag_state.rs, eks_connection_state.rs, cluster_appearance_store.rs, pty_state.rs, yaml_editor_state.rs}` | L |

Ordering rationale:
- **A first.** CI hygiene benefits every subsequent slice (rustfmt / clippy /
  deny gate). The change is small and isolated to workflow YAML — merging it
  early exposes latent format or dependency issues so B–G can fix them
  incrementally.
- **B before C.** C's async tests target the API surface that B changes
  (`create_eks_client` shape, error propagation from injections). C also
  edits the same `client.rs`, `aws_eks.rs`, and `baeus-core/Cargo.toml` B
  touches, so sequential ordering avoids merge conflicts.
- **D before E.** D touches five unrelated files across four crates with
  small edits; E is a larger emulator rewrite. Landing D first keeps early
  slices small and reviewable.
- **D before F, F before G** (the `app_shell.rs` chain). D adds a
  registry-handle injection at `AppShell::new`. F edits only
  `render_resource_table_body_filtered`. G restructures the file
  wholesale. Doing them in this order means each successor consumes the
  prior slice's state (registry handle, then virtualized table body) as
  input to its decomposition.

Each slice will be planned separately via `/loom:plan` — this spec is the
input to those slice-plans, not a substitute for them.

## Invariants / non-negotiables

- **No decrease in test count** across any slice (constitution: TDD, no
  merges that decrease coverage). Every finding's fix carries at least one
  new automated test unless the fix is a pure workflow / documentation
  change (0006 slice).
- **Existing ADRs are authoritative.** No slice may change or contradict an
  Accepted ADR; if a slice-plan surfaces a genuine open decision, the
  planner authors a new ADR before the slice-plan proceeds (see Decisions
  section above).
- **No GPUI main-thread I/O added.** New file writes, subprocess spawns
  (0005-H3), and refresh loops (0002-H2) run under `cx.spawn` /
  `tokio_handle.spawn`.
- **Credentials remain in `zeroize`/`secrecy` types** (constitution security
  requirement). Any refresh callback for 0002-H2 handles `SecretString`, not
  `String`.
- **CI gate stays green.** Every slice must pass `cargo fmt --check`,
  `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, and
  (after slice A) `cargo deny check`.

## Out of scope

- Medium- and low-severity findings from all five research notes — a later
  planning cycle, **except** for the minimal `paths:` filter additions
  named in 0006-H1's fix approach (`deny.toml`,
  `.github/workflows/**`), which are intrinsic to making the new deny gate
  fire. This means medium finding 0006 §9 ("CI `paths:` filter excludes
  workflow/config changes") is **partially remediated by slice A**; any
  broader trigger-path redesign remains deferred.
- The `InformerManager`/`AbortHandle` state-machine hardening (medium
  finding 0002 §7).
- Permission-confirmation UI for plugin install (mentioned in ADR 0005 but
  not required by 0005-H1's wiring fix).
- kube-rs WebSocket exec bridge to `alacritty_terminal` (medium finding
  0005 §7 — post-emulator-integration follow-up).
- Hunk-only fallback for oversized diffs (0005-H4's guard returns an error;
  a proper fallback view is a future feature).
- Signing / notarising the macOS `.app` (medium finding 0006 §5).
- Pinning GitHub Actions to SHA digests (medium finding 0006 §7).
- `.sdlc/` visibility and `CLAUDE.md` gitignore policy (low finding
  0006 §12).
- Any refactor beyond what a listed high requires — the god-object
  decomposition (0003-H1) is bounded to the five sub-structs listed and the
  ≤25-field target; wider layout reshuffles are deliberately deferred.
