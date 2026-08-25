# Handoff

**Start here each session.**

1. Read [`.docs/spec/README.md`](../spec/README.md) — reading order and the
   non-negotiable decisions (constitution).
2. Read [`.docs/status/progress.md`](progress.md) — current phase, last
   action, decision index.
3. Check [`.docs/status/roadmap.md`](roadmap.md) — what the owner wants next.

## Current context (2026-08-25)

- The repo is loom-shaped as of today's `/loom:init` alignment pass
  (Unaligned-bare → aligned). Future `/loom:*` runs detect **Initialized**.
- Five descriptive specs (`.docs/spec/01`–`05`) are Draft and need Plan
  Review; six ADRs and one research note were imported from the speckit repo
  with status preserved.
- Gate: `cargo fmt --check` → `RUST_MIN_STACK=268435456 cargo clippy
  --workspace --all-targets -- -D warnings` → `RUST_MIN_STACK=268435456
  cargo test --workspace` (see `.docs/spec/03-toolchain-and-gate.md`).

## Immediate next step

**Slice A Ready to Publish — PR open (2026-08-25):** branch
`slice/a-ci-hardening` awaits owner merge. After merge: author slice-plan B
(watch cancellation + EKS token refresh, per spec 06). Remaining slices:
B → C → D → E → F → G.