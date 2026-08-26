# Progress

Status source of truth and decision index.

- **Phase**: remediation in flight — slices A and A2 LANDED (2026-08-26), slices B–G queued
- **Last action**: slice A2 code-eval PASS (Ready to Publish → Archived); PR landing on
  branch `slice/a2-ci-toolchain-pin`. Pins Rust toolchain to `1.88` in CI to eliminate
  toolchain-drift failures.
- **Next**: slice B development on branch `slice/b-watch-eks` — watch cancellation + EKS
  token refresh (spec 06 findings 0002-H1, 0002-H2). Also pending: Plan Review of
  back-filled specs 01–05 (roadmap milestone 2).

## Decision index

| Decision | Where | Status |
|----------|-------|--------|
| GPUI + gpui-component UI framework | [ADR 0001](../ADR/0001-gpui-framework.md) | Accepted (imported) |
| kube-rs client | [ADR 0002](../ADR/0002-kube-rs-client.md) | Accepted (imported) |
| alacritty_terminal + portable-pty | [ADR 0003](../ADR/0003-alacritty-terminal.md) | Accepted (imported) |
| Helm hybrid (Secrets read + CLI ops) | [ADR 0004](../ADR/0004-helm-cli-hybrid.md) | Accepted (imported) |
| dylib plugin system | [ADR 0005](../ADR/0005-plugin-dylib.md) | Accepted (imported) |
| Tokio runtime | [ADR 0006](../ADR/0006-tokio-runtime.md) | Accepted (imported) |
| Constitution (principles + security reqs) | [spec/README non-negotiables](../spec/README.md) | Ratified 2026-02-24 (imported) |

Back-filled specs 01–05 are **Draft**, pending Plan Review — no decisions are
recorded from the alignment pass itself.
