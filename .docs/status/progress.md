# Progress

Status source of truth and decision index.

- **Phase**: remediation in flight — slice A LANDED (2026-08-26), slices B–G queued
- **Last action**: slice A landed on `main` via PR #5 (merge `5e90381`),
  receipt recorded in `.docs/slice-plans/archive/slice-a-ci-hardening.md`.
  Post-publish triage merged with it: RUSTSEC-2026-0190 (anyhow 1.0.104) +
  CI toolchain-parity clippy fixes (1.94 → 1.98).
- **Next**: slice-plan B — watch cancellation + EKS token refresh (spec 06).
  Queued for the same planner pass (owner-directed): CI toolchain pin as a
  spec-06 amendment candidate. Also pending: Plan Review of back-filled
  specs 01–05 (roadmap milestone 2).

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
