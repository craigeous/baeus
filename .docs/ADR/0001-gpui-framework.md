# ADR 0001: GPU-Rendered UI Framework — GPUI

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-001-gpui-framework.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus requires a high-performance, native desktop UI framework for macOS that can render complex data-heavy views (resource tables with 5,000+ rows, charts, terminal emulators, YAML editors) at 120+ FPS while never blocking on I/O. The spec mandates GPU-accelerated rendering comparable to Warp's approach.

## Decision

Use **GPUI** (Zed's GPU-accelerated UI framework) with the **gpui-component** library (by Longbridge) for production-ready components.

## Rationale

- Direct lineage from Warp's approach — Nathan Sobo built Warp's custom UI framework, which evolved into GPUI for Zed
- Production-proven: Zed editor ships with GPUI rendering complex UIs at 120+ FPS on Metal
- Native Metal support on macOS with flexbox layout (Taffy engine) and grid support
- gpui-component provides 40+ components including data tables with virtual scrolling, charts, forms, dialogs
- Excellent text rendering quality — Zed is a code editor built on GPUI
- Hybrid immediate/retained mode with reactive state management

## Alternatives Considered

| Framework | Why Rejected |
|-----------|-------------|
| Makepad | Font rendering and internationalization lacking. No data table or chart components. |
| Xilem/Masonry/Vello | Alpha state, not recommended for production. Text input described as "janky." |
| Custom framework | Massive engineering effort. GPUI provides equivalent capability. |

## Consequences

- macOS-only initially (Metal dependency)
- Tight coupling to GPUI's layout model and component patterns
- Benefits from Zed ecosystem improvements over time
