# Progress

Status source of truth and decision index.

- **Phase**: init / Unaligned alignment complete
- **Last action**: `/loom:init` alignment pass (2026-08-25) — detected
  Unaligned-bare (no `.docs/`, no in-repo docs spine; speckit tree lives in
  the separate `~/git/baeus-spec` repo). Scaffolded `.docs/`, adopted the
  verified Rust gate (workspace-adapted), back-filled five descriptive Draft
  specs, imported six Accepted ADRs and the Phase-0 research note with
  provenance, seeded status. No new decisions were made.
- **Next**: owner declares scope and runs `/loom:run`. Declared scope on
  file: **full project review** (see `roadmap.md` milestone 1).

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
