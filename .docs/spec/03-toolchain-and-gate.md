# 03 — Toolchain & Gate

**Status**: Draft (descriptive back-fill, 2026-08-25 — pending Plan Review)

## Toolchain

- Rust stable, edition 2024, MSRV 1.85 (`Cargo.toml` workspace.package,
  `clippy.toml` msrv).
- Formatting: `rustfmt.toml` — `max_width=100`, `use_small_heuristics=Max`.
- Target: macOS (aarch64-apple-darwin), Metal rendering.
- `RUST_MIN_STACK=268435456` is required for compiling/testing `baeus-ui`
  (GPUI `syn` proc-macro recursion; also set in `.cargo/config.toml`).

## Gate (verified Rust gate, workspace-adapted)

Runs in `format → lint → test` order; all three must pass before any slice is
marked Implemented. The code evaluator re-runs this gate rather than trusting
recorded results.

| Step | Command |
|------|---------|
| format | `cargo fmt --check` |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` |

CI (`.github/workflows/ci.yml`, matrix over macos-14 / ubuntu-latest /
windows-latest, PRs to main touching `crates/**`, `Cargo.toml`, `Cargo.lock`,
`deny.toml`, or `.github/workflows/**`) runs the gate in `format → deny →
lint → test` order: `cargo fmt --all -- --check`, `cargo deny check`,
`cargo clippy --workspace -- -D warnings`, and `cargo nextest run --workspace`.
`-D warnings` is load-bearing — warnings are CI errors. (Updated to reflect
slice A: 3-OS matrix, fmt gate, deny gate, and extended trigger paths added by
findings 0006-H1/H2/H3.)

Current recorded state at alignment time: 3,641 tests passing, clippy clean.

## Other commands

- `cargo check --workspace` — fast compile check.
- `./macos/build-app.sh` — produces the signed-less macOS `.app` bundle
  (`macos/Info.plist`, `macos/AppIcon.icns`).
- Release pipeline: `.github/workflows/release.yml`.
- Dependency policy: `deny.toml` (cargo-deny); constitution requires
  vulnerability auditing of dependencies.
