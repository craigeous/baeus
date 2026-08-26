# Review findings — slice-b-watch-eks

Slice diff reviewed: `bf074dd..HEAD` — baeus-core watch-cancellation surface,
`EksTokenRefresher` + tower `AuthRefreshLayer`, `create_eks_client` refactor,
AppShell wiring, new deps (tokio-util/tower/hyper/hyper-util/wiremock).

## /code-review

Status: skipped: command-unavailable

The built-in `code-review` skill is configured `disable-model-invocation` in
this environment — owner-invocable only. Recorded as a non-run.

## /security-review

Status: ran-clean

Finder pass over the full slice diff verified: token material is
`secrecy::SecretString` (zeroizes on drop, no `Debug` derive); the single
production `expose_secret` (aws_eks.rs:940) formats the token into the
`Authorization` header — its purpose; error paths cannot carry token material
into watcher warn-logs; the refresh layer is module-private, one instance per
`create_eks_client` call, pinned to its cluster via `base_uri_layer()` (no
cross-cluster leak); refresh failure is fail-closed (request aborted, no
stale token served); presign `X-Amz-Expires: 60` matches `expires_at` at both
sites with 10s leeway; TLS unchanged (cluster CA, no invalid-cert bypass);
no new process invocation (in-process HMAC presign); `tokio::Mutex` re-check
prevents double-presign. Noted (out of security scope, for evaluator
awareness): `create_eks_client` has no production callers yet (wiring
deferred per cluster.rs:114-118 comment); an app_shell cleanup race can leave
a new watcher uncancellable (reliability, no credential exposure). No
candidate findings; finder executed normally.
