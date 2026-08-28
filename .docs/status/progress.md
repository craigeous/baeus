# Progress

Status source of truth and decision index.

- **Phase**: remediation in flight — slices A, A2, B landed; C landing (PR open); slices D–G queued
- **Last action**: slice C code-eval PASS (Ready to Publish, 2026-08-28);
  plan archived to `.docs/slice-plans/archive/slice-c-injection-errors-async-tests.md`.
  PR landing on branch `slice/c-injection-errors` (AWS credential-injection typed errors +
  async wizard tests).
- **Next**: slice-plan D — plugin loader UB, helm CLI ops, diff-matrix cap (spec 06
  remaining highs). Also pending: Plan Review of back-filled specs 01–05 (roadmap milestone 2).

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
