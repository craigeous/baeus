# 01 — Overview

**Status**: Draft (descriptive back-fill, 2026-08-25 — pending Plan Review)

Baeus is a Kubernetes cluster management desktop UI for macOS, written in Rust
and rendered with GPUI (Zed's GPU-accelerated UI framework) via Metal. It aims
at feature parity with Lens / OpenLens / FreeLens / Aptakube / Headlamp:
multi-cluster connectivity, resource browsing and management, log streaming,
embedded terminal exec, Helm release management, metrics, CRD browsing, a
resource relationship map, and a dylib plugin system.

## What exists today

The full originally-specified feature set (speckit feature `001-k8s-cluster-ui`,
phases 1–25) is implemented, plus a subsequent UX overhaul and an enhanced pod
detail view. As of the alignment survey the workspace reports **3,641 passing
tests** and a clean clippy run. The app builds as a distributable macOS
`.app` bundle (`macos/build-app.sh`).

Major implemented surfaces (descriptive, not exhaustive):

- **Navigator sidebar** — Lens-style multi-cluster tree: all discovered
  kubeconfig contexts shown simultaneously with status dots, deterministic
  2-letter cluster icons, independently expandable resource categories with
  count badges, indent guides, `uniform_list` rendering.
- **Workspace tabs** — preview/fixed tab modes (VS Code-style), pinned
  dashboard tab, cluster-prefixed tab titles, keyboard navigation.
- **Resource tables** — per-kind columns, sorting, fuzzy filtering, namespace
  scoping, CSV export, row click → detail.
- **Resource detail views** — rich pod detail (collapsible sections, container
  cards, custom Lucide SVG section icons via `BaeusAssets`), node detail,
  event detail, properties/labels/annotations/conditions sections.
- **Dock panel** — terminal sessions (alacritty_terminal + portable-pty), log
  viewer (streaming, search highlight/filter modes, multi-container,
  previous-container), port-forward manager.
- **YAML editor** — ropey buffer + tree-sitter highlighting, validation, diff.
- **Helm** — release listing decoded from cluster Secrets; operations via
  helm CLI hybrid (ADR 0004).
- **AWS/EKS integration** (added after the original spec) — AWS SSO login
  flow, EKS cluster discovery/connection wizard (`baeus-core::aws_eks`,
  `aws_sso`; `eks_wizard*` components), in-memory credential injection for
  initial connection, auto-connect of restored EKS clusters on startup,
  re-authentication and updater preferences.
- **Kubeconfig discovery** — recursive `~/.kube/` directory scan (depth 3),
  YAML heuristic detection, aggregated contexts.
- **Real cluster I/O** — kube-rs client, informers, metrics-server
  integration, RBAC awareness, graceful degradation on errors.

## Distribution

- CI: `.github/workflows/ci.yml` (clippy + nextest on macos-14, PRs to main).
- Release: `.github/workflows/release.yml`; local bundle via
  `macos/build-app.sh` (icon `macos/AppIcon.icns`, `Info.plist`).

## Historical source documents

The original product requirements and design live in the separate speckit
repository `~/git/baeus-spec` (`specs/001-k8s-cluster-ui/`: `spec.md`,
`plan.md`, `data-model.md`, `research.md`, `tasks.md`, `lens-reference.md`).
[05 — Feature Scope](05-feature-scope.md) summarizes that requirement
inventory; the speckit repo remains the historical record, while `.docs/` is
authoritative going forward.
