# Research: Core client & AWS integration review

**Status**: Research Review
**Date**: 2026-08-25
**Subsystem**: crates/baeus-core

## Summary

The core client layer has a solid foundation: path-traversal guards on URL construction,
`Debug` redaction for sensitive fields, and zeroize-on-drop for credential types.
The AWS EKS wizard is the most complex and risky area. Four high-severity gaps were
found: watch streams have no cancellation surface, EKS bearer tokens are single-use with
a 60-second TTL and no refresh path, credential injection functions silently succeed when
the exec block is absent, and all AWS SDK async paths are completely untested.
The subsystem would benefit most from cancellation tokens for watch loops, a token-refresh
callback for EKS clients, and at least smoke-level async tests for the SSO/EKS wizard flow.

## Strengths

- Path-traversal protection on every URL-interpolated segment (`client.rs:754-770`),
  enforced before both reads and writes.
- `resolve_known_api_resource` guard on all mutating operations (`client.rs:774-780`)
  prevents writes to fallback-heuristic API paths.
- `Debug` impls redact cert/key/token data throughout `auth.rs` and `aws_eks.rs`
  (`auth.rs:70-108`, `aws_eks.rs:48-64`, `aws_eks.rs:95-106`, `aws_eks.rs:118-133`,
  `aws_eks.rs:162-180`).
- Zeroize on drop for `AccessKeyConfig` (`aws_eks.rs:66-72`) and `AuthDetails`
  (`auth.rs:43-66`); `secrecy::SecretString` used correctly for the kube bearer token
  in `create_eks_client` (`aws_eks.rs:853`).
- Kubeconfig Unix permissions warning at `kubeconfig.rs:30-42`.
- Home-directory restriction on additional kubeconfig scan paths (`kubeconfig.rs:246`)
  prevents a tampered preferences file from reading arbitrary locations.
- `default_backoff()` on watcher streams (`client.rs:1191`, `1283`) handles transient
  API server errors without manual retry loops.
- `update_resource` validates body name/namespace against URL target and injects
  `resourceVersion` for optimistic concurrency (`client.rs:899-940`).

## Findings

### 1. No cancellation surface on `watch_events` / `watch_resources`
**Severity**: High

`watch_events` (`client.rs:1181`) and `watch_resources` (`client.rs:1246`) are infinite
`while let Some(...)` loops backed by `kube_runtime::watcher(...).default_backoff()`.
The `default_backoff` combinator reconnects on errors indefinitely and never yields
`None` under normal operation. Neither function accepts a `CancellationToken` or any
shutdown signal. Callers can abort the outer Tokio task, but that leaves no deterministic
teardown for the associated `kube::Client` and in-flight requests. When a cluster is
disconnected from the UI there is no structured way to stop the associated watch loops.

### 2. EKS bearer token expires in 60 s; `create_eks_client` has no refresh path
**Severity**: High

`build_eks_presigned_token` sets `X-Amz-Expires = "60"` (`aws_eks.rs:774`).
`create_eks_client` generates one token at construction time and embeds it in a static
`Kubeconfig` (`aws_eks.rs:832-873`). The resulting `kube::Client` receives 401 responses
from every API call after 60 seconds with no mechanism to regenerate the token.
`ClusterConnection.token_expiry` exists (`cluster.rs:39`) but nothing watches it for EKS
clients created through this path. There are no tests that observe what happens when
the token lapses.

### 3. Silent no-op when exec block is absent in credential injection
**Severity**: High

Both `inject_aws_profile_into_kubeconfig` (`aws_sso.rs:53-107`) and
`inject_aws_credentials_into_kubeconfig` (`aws_sso.rs:112-180`) return `Ok(())` when
the context's `auth_info.exec` block is `None` (double-nested `if let Some` at
`aws_sso.rs:80-81` and `aws_sso.rs:139-140`). If the kubeconfig context uses certificate
or token auth rather than an exec plugin, the caller receives success but no credentials
were injected. `create_client_from_path_with_aws_creds` (`client.rs:199`) relies on this
injection before building a `kube::Config`; the failure mode is a confusing 401/TLS
error from the API server rather than an explicit diagnostic at injection time.

### 4. Zero async tests for all critical AWS SDK paths
**Severity**: High

`aws_eks.rs` and `aws_sso.rs` contain no `#[tokio::test]` tests. All async functions —
`sso_register_client` (line 237), `sso_start_device_auth` (line 262),
`sso_poll_for_token` (line 298), `sso_list_accounts` (line 348),
`sso_get_role_credentials` (line 432), `authenticate_with_access_key` (line 493),
`assume_role` (line 530), `create_eks_client` (line 828), and `generate_eks_token`
(line 724) — are untested beyond serialization smoke tests on their config structs.
The AWS wizard is the most complex and fragile user-facing flow; regressions in any step
are invisible until a live cluster is available.

### 5. `get_caller_identity` shells out to the AWS CLI
**Severity**: Medium

`aws_sso::get_caller_identity` (`aws_sso.rs:24`) spawns
`tokio::process::Command::new("aws")` to run `sts get-caller-identity`. All other AWS
operations in the crate use `aws-sdk-sts` / `aws-sdk-sso` / `aws-sdk-eks` directly via
`aws-config`. This function fails on machines where the AWS CLI is not installed, with an
error message that mentions a binary baeus does not otherwise require. The SDK already
provides the same call; `authenticate_with_access_key` (`client.rs:508-513`) demonstrates
a direct `aws_sdk_sts` call for identity verification.

### 6. AWS credential fields use manual zeroize but not `secrecy::SecretString`
**Severity**: Medium

`AccessKeyConfig.secret_access_key` (`aws_eks.rs:43`) and `.session_token` (`aws_eks.rs:44`)
are plain `String` with `zeroize` called in `Drop` (`aws_eks.rs:66-72`).
`AwsSession.sso_access_token` (`aws_eks.rs:159`) is also a plain `String`.
The `secrecy` crate is already a dependency and is used correctly for the kube bearer
token in `create_eks_client` (`aws_eks.rs:853`). Plain strings can be cloned and moved,
leaving copies that the `Drop` impl on the original cannot reach. `secrecy::SecretString`
would prevent accidental `Display` / `Debug` leakage and make the memory contract
explicit across clone sites.

### 7. `InformerManager` state tracking is decoupled from real Tokio tasks
**Severity**: Medium

`InformerManager` tracks `InformerState` (Idle/Running/Reconnecting/Stopped/Error)
(`informer.rs:22-29`) but no Tokio task handle is associated with each entry.
`ResourceWatchBridge::register_watcher` sets state to `Running` immediately at
`watch.rs:44` without spawning a background task. If a real background task for that
watcher crashes or is aborted, the stored state remains `Running` indefinitely.
The UI reads this state to decide whether a watcher is healthy; stale state leads to
misleading status indicators.

### 8. `ResourceWatchBridge::register_watcher` orphans old informer on duplicate key
**Severity**: Medium

At `watch.rs:45`, `self.watcher_ids.insert((cluster_id, kind.to_string()), id)` overwrites
the previous entry when the same `(cluster_id, kind)` pair is registered twice. The old
`InformerEntry` remains in `informer_manager.informers` indefinitely, contributing to
`active_count()` and `total_count()` with no key pointing to it. The test
`test_register_same_kind_overwrites_watcher_id` (`watch.rs:659`) documents this as
observed behavior but does not assert the orphan is cleaned up.

### 9. `fetch_dashboard_data` issues unbounded list requests
**Severity**: Medium

`ListParams::default()` is used for nodes, pods, and namespaces at `client.rs:326`.
On a large cluster (thousands of pods or nodes), this can return hundreds of megabytes
in a single response, blocking the Tokio executor during deserialization and holding the
full list on the heap. Events are correctly bounded to 100 (`client.rs:327`) but the
core resources are not.

### 10. `discover_clusters_in_region` describes clusters sequentially
**Severity**: Medium

Within a region, `discover_clusters_in_region` loops over cluster names with sequential
`eks.describe_cluster().name(name).send().await` calls (`aws_eks.rs:681`). Describing N
clusters serially multiplies round-trip latency by N. Cross-region discovery is
parallelized (`aws_eks.rs:622`) but per-region cluster describes are not.

### 11. `sso_get_role_credentials` constructs a fabricated ARN
**Severity**: Medium

At `aws_eks.rs:481`, `identity_arn` is set to
`format!("arn:aws:sso:::account/{account_id}/role/{role_name}")`, which is not a valid
AWS ARN. Consumers that validate or display ARNs will receive malformed data. The
`GetRoleCredentials` SDK response does not include the caller's ARN; a subsequent
`GetCallerIdentity` call or leaving the field empty would be more correct.

### 12. `LogStreamManager::remove_stream` by position invalidates caller-held indices
**Severity**: Low

`remove_stream(index)` calls `self.streams.remove(index)` (`logs.rs:167`), which shifts
all subsequent entries down by one. `add_stream` returns a `usize` index (`logs.rs:158`)
with no stable UUID handle, so any caller that caches an index before a removal
references the wrong stream afterward.

### 13. `KubeconfigWatcher` silently recovers from poisoned Mutex
**Severity**: Low

At `kubeconfig.rs:521`, `prev_snapshot.lock().unwrap_or_else(|e| e.into_inner())`
continues processing after mutex poisoning. A panic in the diff callback would poison
the lock; subsequent invocations would silently process potentially inconsistent snapshot
data without surfacing the original panic.

### 14. `is_kubeconfig_file` uses string-contains heuristic
**Severity**: Low

`kubeconfig.rs:316-321` checks `contents.contains("kind: Config")` without full YAML
parse to identify kubeconfig files. A YAML file containing that string in a comment or
in a string value would be identified as a kubeconfig; a kubeconfig with atypical
formatting (`kind:Config` without a space) is handled but other edge cases are not.

## Candidate opportunities

- Add a `CancellationToken` parameter (or return a `tokio::task::AbortHandle`) for
  `watch_events` and `watch_resources` so callers can stop watch loops cleanly.
- Give `create_eks_client` or a wrapper a token-refresh callback, or return the expiry
  time alongside the client so the caller can re-generate before the 60-second TTL
  lapses.
- Return an explicit `Err` from the inject functions when the exec block is absent,
  so `create_client_from_path_with_aws_creds` receives a diagnostic rather than
  a downstream auth failure.
- Replace the `get_caller_identity` shell-out with a direct `aws_sdk_sts` call,
  eliminating the AWS CLI dependency for this function.
- Migrate `AccessKeyConfig.secret_access_key`, `.session_token`, and
  `AwsSession.sso_access_token` from plain `String` to `secrecy::SecretString`.
- Add `#[tokio::test]` async tests for the SSO and EKS wizard paths using the AWS SDK's
  smithy mock transport or a local HTTP mock, covering at minimum: device auth happy
  path, `AuthorizationPendingException` retry, token exchange, cluster discovery, and
  `create_eks_client` round-trip.
- Associate each `InformerEntry` with a `tokio::task::AbortHandle` so `stop_for_cluster`
  and `stop_watching` actually cancel background tasks, and task completion transitions
  state to `Stopped` automatically.
- Add a `ListParams::default().limit(N)` (e.g. 1000) to node and pod list calls in
  `fetch_dashboard_data` to bound memory use on large clusters.
- Parallelize `describe_cluster` calls within `discover_clusters_in_region` using
  `futures::stream::FuturesUnordered` or a bounded `tokio::spawn` batch.
- Replace positional indexing in `LogStreamManager` with a UUID-keyed `HashMap` to
  eliminate the index-shift hazard on removal.

## Citations

- `crates/baeus-core/src/aws_eks.rs` — EKS token generation, SSO device-code flow,
  cluster discovery, `create_eks_client`, `AccessKeyConfig`, `AwsSession`
- `crates/baeus-core/src/aws_sso.rs` — `get_caller_identity` CLI shell-out,
  `inject_aws_profile_into_kubeconfig`, `inject_aws_credentials_into_kubeconfig`,
  SSO expiry detection
- `crates/baeus-core/src/client.rs` — `watch_events`, `watch_resources`,
  `fetch_dashboard_data`, path-segment validation, URL building, resource CRUD,
  RBAC helpers
- `crates/baeus-core/src/informer.rs` — `InformerManager`, state machine, cache
- `crates/baeus-core/src/watch.rs` — `ResourceWatchBridge`, `register_watcher`,
  `stop_watching`
- `crates/baeus-core/src/kubeconfig.rs` — `KubeconfigLoader`, `KubeconfigDiscovery`,
  `KubeconfigWatcher`, `is_kubeconfig_file`, `scan_directory_for_kubeconfigs`
- `crates/baeus-core/src/auth.rs` — `AuthDetails`, `AuthConfig`, debug redaction,
  zeroize on drop
- `crates/baeus-core/src/cluster.rs` — `ClusterConnection`, `AuthMethod`, reconnect
  state machine
- `crates/baeus-core/src/logs.rs` — `LogStreamManager`, `LogBuffer`, `MultiPodLogState`
- `crates/baeus-core/src/exec.rs` — `ExecManager`, `PortForwardManager`
- `crates/baeus-core/src/runtime.rs` — `TokioHandle` global
- `crates/baeus-core/src/resource.rs` — `Resource`, `ResourceStore`
- `crates/baeus-core/src/metrics.rs` — `NodeMetrics`, `PodMetrics`
- `crates/baeus-core/src/rbac.rs` — RBAC verb types
- `crates/baeus-core/src/crd.rs` — `CrdSchema`
- `crates/baeus-core/src/lib.rs` — module exports, `Namespace`, `Event` types
- `crates/baeus-core/Cargo.toml` — dependency inventory (secrecy, zeroize, ring,
  aws-sdk-*)
