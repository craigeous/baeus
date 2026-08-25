# ADR/

Architecture Decision Records — one decision per file, named
`NNNN-short-slug.md`.

- **Authored by:** the planner role (via `/loom:run` or `/loom:plan`), when a
  decision with lasting consequences is made.
- **Reviewed by:** the plan evaluator (blind) before acceptance.
- **Lifecycle:** Draft → Plan Review → **Accepted**. Accepted ADRs are
  **immutable** — superseding a decision means writing a new ADR that
  references the old one, never editing the accepted record.

## Index

| ADR | Decision | Status |
|-----|----------|--------|
| [0001](0001-gpui-framework.md) | GPU-rendered UI framework — GPUI | Accepted (imported from speckit) |
| [0002](0002-kube-rs-client.md) | Kubernetes client library — kube-rs | Accepted (imported from speckit) |
| [0003](0003-alacritty-terminal.md) | Terminal emulation — alacritty_terminal + portable-pty | Accepted (imported from speckit) |
| [0004](0004-helm-cli-hybrid.md) | Helm integration — hybrid CLI + kube-rs | Accepted (imported from speckit) |
| [0005](0005-plugin-dylib.md) | Plugin system — dynamic library loading (.dylib) | Accepted (imported from speckit) |
| [0006](0006-tokio-runtime.md) | Async runtime — Tokio | Accepted (imported from speckit) |
