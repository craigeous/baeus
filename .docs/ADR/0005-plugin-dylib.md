# ADR 0005: Plugin System — Dynamic Library Loading (.dylib)

**Status**: Accepted
**Date**: 2026-02-24
**Provenance**: Imported verbatim (renumbered) from the speckit decision record
`baeus-spec:.specify/decisions/ADR-005-plugin-dylib.md` during loom
alignment. The decision predates loom; content is unchanged.

## Context

Baeus needs an extensible plugin system allowing third-party code to add views, actions, and integrations without modifying the core application.

## Decision

Use **libloading** for dynamic Rust library loading (.dylib on macOS) with a versioned `BaeusPlugin` trait as the plugin contract.

## Rationale

- Plugins as Rust .dylib files implementing a versioned `BaeusPlugin` trait
- Plugin API exposes: view registration, action registration, resource access, event hooks
- Plugins are sandboxed — interact only through defined API, no arbitrary app state access
- Hot-reloading: plugins can be loaded/unloaded without app restart
- Plugin discovery: local directory scanning + optional remote catalog
- Permission model: plugins declare required permissions, users confirm before install

## Alternatives Considered

| Option | Why Rejected |
|--------|-------------|
| WASM plugins | Higher isolation but significant performance overhead, limited GPUI access. |
| Lua scripting | Limited type safety, another language for plugin authors. |
| gRPC out-of-process plugins | Complex IPC, latency overhead, harder UI view integration. |

## Consequences

- Plugins must be compiled for the same platform and ABI
- Version compatibility must be enforced (breaking API changes = major version bump)
- Security requires sandboxed directory loading and permission enforcement
- Plugin authors must use Rust (higher barrier than scripting languages)
