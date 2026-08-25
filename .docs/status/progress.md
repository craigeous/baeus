# Progress

Status source of truth and decision index.

- **Phase**: research / full project review complete — at owner gate
- **Last action**: `/loom:run` full project review (2026-08-25) — five
  research notes (`.docs/research/0002`–`0006`) authored across core/AWS, UI
  layout, UI components, supporting crates, and quality infra; all five
  passed blind plan evaluation on round 0 and are **Approved**. Verdicts in
  `.docs/evaluations/`. ~62 findings total (17 high / 26 medium / 19 low).
- **Next**: **owner gate** — owner reviews findings and picks which
  improvements go to planning (ADRs → specs → slice-plans). Also pending:
  Plan Review of back-filled specs 01–05 (roadmap milestone 2).

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
