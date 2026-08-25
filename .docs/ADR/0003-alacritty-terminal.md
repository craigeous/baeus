# ADR 0003: Terminal Emulation — alacritty_terminal + portable-pty

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-003-alacritty-terminal.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus needs embedded terminal emulation for pod exec sessions and local shell access, rendered within the GPUI framework at native performance.

## Decision

Use **alacritty_terminal** (v0.25.x) for terminal emulation and **portable-pty** (v0.9.x) for cross-platform PTY handling. This is the same stack used by the Zed editor.

## Rationale

- `alacritty_terminal` is battle-tested, used by Alacritty and Zed
- `portable-pty` provides native macOS PTY support with 900k+ monthly downloads
- Zed provides a reference architecture for embedding alacritty_terminal within GPUI
- For pod exec: kube-rs WebSocket exec pipes through the terminal emulator instead of a local process
- Architecture: Terminal core wraps `Term` with custom event listener, PTY managed via portable-pty, grid rendered to GPUI with GPU-accelerated text

## Alternatives Considered

| Option | Why Rejected |
|--------|-------------|
| Custom VTE implementation | Reimplements complex terminal escape sequence handling. |
| xterm.js via WebView | Introduces web runtime dependency, breaks pure-Rust GPU rendering. |

## Consequences

- Proven architecture pattern from Zed reduces implementation risk
- Terminal rendering shares GPUI's Metal pipeline for consistent performance
- PTY management for pod exec requires bridging kube-rs WebSocket streams
