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

CI (`.github/workflows/ci.yml`, macos-14, PRs to main touching crates/ or
Cargo manifests) runs the same lint and tests via `cargo nextest run
--workspace`. `-D warnings` is load-bearing — warnings are CI errors.

Current recorded state at alignment time: 3,641 tests passing, clippy clean.

## Other commands

- `cargo check --workspace` — fast compile check.
- `./macos/build-app.sh` — produces the signed-less macOS `.app` bundle
  (`macos/Info.plist`, `macos/AppIcon.icns`).
- Release pipeline: `.github/workflows/release.yml`.
- Dependency policy: `deny.toml` (cargo-deny); constitution requires
  vulnerability auditing of dependencies.
