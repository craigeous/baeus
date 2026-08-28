# Handoff

**Start here each session.**

1. Read [`.docs/spec/README.md`](../spec/README.md) — reading order and the
   non-negotiable decisions (constitution).
2. Read [`.docs/status/progress.md`](progress.md) — current phase, last
   action, decision index.
3. Check [`.docs/status/roadmap.md`](roadmap.md) — what the owner wants next.

## Current context (2026-08-28)

- Slices A, A2, and B have landed on `main`. Slice C (AWS credential-injection
  typed errors + async wizard tests) passed code evaluation (Ready to Publish,
  2026-08-28); plan archived to `.docs/slice-plans/archive/`.
- Five descriptive specs (`.docs/spec/01`–`05`) are Draft and need Plan
  Review; six ADRs and one research note were imported from the speckit repo
  with status preserved.
- Gate: `cargo fmt --check` → `RUST_MIN_STACK=268435456 cargo clippy
  --workspace --all-targets -- -D warnings` → `RUST_MIN_STACK=268435456
  cargo test --workspace` (see `.docs/spec/03-toolchain-and-gate.md`).

## Immediate next step

**Open C PR and merge on green** (`slice/c-injection-errors` → `main`; CI must pass).
Then: slice-plan D — plugin loader UB, helm CLI ops, diff-matrix cap
(spec 06 remaining highs). Remaining after D: E → F → G.
