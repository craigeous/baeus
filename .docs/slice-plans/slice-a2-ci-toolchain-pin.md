# Slice A2 — CI Toolchain Pin

Status: Implemented
Target specs: `.docs/spec/06-remediation-highs.md` (§ "Slice A2 — CI toolchain
pin (owner-directed, 2026-08-26)", authorised by the Amendments section, same
spec); gate defined by `.docs/spec/03-toolchain-and-gate.md`.

## Context

`.github/workflows/ci.yml` and `.github/workflows/release.yml` both resolve
the Rust toolchain via `dtolnay/rust-toolchain@stable`. `stable` is a moving
ref: GitHub-hosted runners upgrade to whatever rustc is current at CI-run
time, and there is no correspondence between the toolchain that produced the
commit locally and the one that gates it in CI. Slice A hit this class
empirically — see the slice A archived receipt
(`.docs/slice-plans/archive/slice-a-ci-hardening.md#landed-receipt`):

> **Post-publish triage** (same branch, merged in the same PR): […] 7 clippy
> errors from CI's newer stable toolchain (1.94 → 1.98) fixed for CI parity.

That fix (commit `3b518de`) closed the specific lints, but did not close the
class. Every subsequent slice B..G risks the same asymmetry: a green local
gate, red CI, and a same-slice fix pass that adds noise to the diff a plan
and code evaluator must review. Owner direction (2026-08-26, queued at slice
A close) is to pin CI's toolchain to a specific version so that stable
upgrades happen only when we intentionally raise them.

Spec 06's Slice A2 section (added by the 2026-08-26 amendment) is the
authority for this slice. This plan operationalises that section.

**In scope (the three mandatory deliverables spec 06 A2 lists, plus one
recommended addition, less the enforcement automation deferred to A3):**

- **Pin workflow toolchain refs.** Replace every
  `uses: dtolnay/rust-toolchain@stable` in `ci.yml` and `release.yml` with
  `dtolnay/rust-toolchain@1.98.0`. Version rationale in step 1.
- **Add repo-root `rust-toolchain.toml`.** Spec 06 A2 lists this file as
  "optional, recommended by the slice-plan"; this plan takes the
  recommendation to close the local/CI asymmetry that slice A exposed.
- **Document upgrade cadence at the pin site.** Spec 06 A2 accepts either
  an inline workflow comment or a spec 03 pointer as the discoverable
  location. This plan uses the inline-comment form: an above-the-`uses:`
  block at the pin sites naming the monthly cadence and citing spec 06 A2
  as authority. Spec 03 editing requires a fresh planning cycle (ADR 0005
  frozen-spec rule) — this slice queues a spec 03 revision as a finalize
  follow-up, not an in-slice edit.

**Explicit non-goals:**

- **Scheduled monthly bump-PR automation.** The prior draft of this plan
  proposed a `.github/workflows/toolchain-bump.yml` scheduled workflow.
  Round-1 plan evaluation surfaced two mechanical defects that made the
  automation self-defeating: (a) `peter-evans/create-pull-request` with
  the default `GITHUB_TOKEN` opens PRs whose `pull_request` event does
  **not** trigger workflow runs (documented GitHub Actions behaviour —
  events caused by `GITHUB_TOKEN` do not create new runs, only
  `workflow_dispatch`/`repository_dispatch` do), so the promised
  "monthly CI-gated bump" would in fact merge unverified or be
  branch-protection-blocked; and (b) `workflow_dispatch` only registers
  for workflow files present on the default branch, so the plan's
  own pre-merge dispatch verification (A2-c) was unexecutable. Spec 06
  A2 leaves cadence enforcement location open — inline comment is
  admissible — so this slice adopts the inline-comment form and defers
  the automation to a **follow-up slice A3** where the design can be
  redone honestly (dedicated PAT/machine account or an explicit
  post-creation `gh workflow run` step; branch-protection reconciliation;
  post-merge dispatch verification path). A3 is queued in this slice's
  finalize pass, not in scope here.
- Any edit to `crates/**`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, or any
  other file outside `.github/workflows/**` and `rust-toolchain.toml` (and
  optionally this slice-plan document itself).
- Any change to MSRV. Spec 03 pins MSRV at 1.85 via `Cargo.toml`
  `workspace.package.rust-version` and `clippy.toml` msrv; this slice does
  not touch either. The workflow pin is a **compiler-version ceiling**
  driven by owner direction, not an MSRV move.
- Any change to the fmt / clippy / test / deny gate steps themselves —
  slice A owns those and A2 does not renegotiate them.
- Any change to release-artifact tagging, notarisation, or signing (all
  spec 06 Out of scope, medium findings 0006 §5).
- Any change to the `dtolnay/rust-toolchain` action reference itself
  (SHA-pinning, medium finding 0006 §7 — deferred). This slice pins the
  *toolchain version input* the action consumes, not the action's own
  version ref.
- Spec 03 edits (queued as a finalize follow-up, not in-slice).

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order.

### 1. Pin version

The pin is **`1.98.0`**, stated unconditionally. Round-1 evaluation
verified live stable is exactly `1.98.0` (channel manifest
`static.rust-lang.org/dist/channel-rust-stable.toml`, dated 2026-08-18):
`version = "1.98.0 (88d9e12ae 2026-08-18)"`. This is also the version slice
A's post-merge fix commit (`3b518de`) targeted, i.e. the last known-CI-green
stable. `dtolnay/rust-toolchain@1.98.0` is a real branch on the action
repo (verified via `git ls-remote`) and its `action.yml` hardcodes
`toolchain: 1.98.0` while still accepting the workflow's `targets:` and
`components:` inputs — so the `@stable → @1.98.0` swap installs rustc
1.98.0 on every matrix leg without breaking `targets: ${{ matrix.target }}`
or the `taiki-e/install-action@nextest` step.

If a re-check at code-eval time (>2 weeks after this plan) shows CI stable
has moved past 1.98.0, the code evaluator may request a bump to the
newest stable that a matching local `RUST_MIN_STACK=268435456 cargo clippy
--workspace --all-targets -- -D warnings` passes on. Otherwise, use
`1.98.0` verbatim.

### 2. `.github/workflows/ci.yml` — pin the toolchain ref

The file has one `Install Rust` step (line 33 in the current tree):

```yaml
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
          components: rustfmt, clippy
```

Change `@stable` to `@1.98.0`. Do **not** change `components:` or
`targets:`. Add an inline comment above the `uses:` line stating the pin
rationale and the cadence policy (per spec 06 A2 "Upgrade cadence policy —
record in a location the code evaluator can verify"):

```yaml
      # Pinned to Rust 1.98.0 per spec-06 Slice A2 (2026-08-26). Do not
      # bump to @stable — cadence policy is a monthly bump PR (owner
      # direction); the enforcing automation is deferred to slice A3.
      # Until A3 lands, bumps are manual: open a PR that edits this pin
      # and rust-toolchain.toml together and lets the full CI matrix
      # verify the new version before merge.
      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.98.0
        with:
          targets: ${{ matrix.target }}
          components: rustfmt, clippy
```

Result: one edit in `ci.yml`.

### 3. `.github/workflows/release.yml` — pin every toolchain ref

`release.yml` uses `dtolnay/rust-toolchain@stable` in five jobs. Grep
before editing to confirm the count:

```
grep -n 'dtolnay/rust-toolchain@' .github/workflows/release.yml
```

Expected: five occurrences (per the current file: line 50 in `build-macos`,
127 in `check-linux`, 173 in `build-linux`, 223 in `check-windows`, 258 in
`build-windows`). If the count differs, stop and reconcile — divergence
means the file changed since planning and the plan needs a refresh.

Apply the same `@stable → @1.98.0` swap to each. Add the same inline
comment (verbatim) above **only the first** occurrence (`build-macos`);
subsequent occurrences may omit the comment to avoid noise, but each must
match the version pinned on the first occurrence. A `sed -i ''
's|dtolnay/rust-toolchain@stable|dtolnay/rust-toolchain@1.98.0|g'
.github/workflows/release.yml` is acceptable so long as the grep after the
edit shows zero remaining `@stable` refs anywhere in `.github/workflows/`
and exactly one edited block carries the comment.

### 4. `rust-toolchain.toml` at repo root

Create a new file `rust-toolchain.toml` at the repo root with exactly this
content:

```toml
# Pinned to Rust 1.98.0 per spec-06 Slice A2 (2026-08-26). This file locks
# local developer builds to the same compiler CI runs against, so no slice
# hits the "green local, red CI" class slice A did. Cadence policy is a
# monthly bump PR (owner direction); the enforcing automation is deferred
# to slice A3. Bumping is done in the same PR that edits the pin in
# .github/workflows/ci.yml — keep this channel and that pin in lockstep.
[toolchain]
channel = "1.98.0"
components = ["rustfmt", "clippy"]
```

Notes constraining this file:

- `channel = "1.98.0"` **must** match the workflow pin exactly (spec 06 A2
  acceptance criterion: "The workflow-pin version matches the
  `rust-toolchain.toml` channel").
- `components` matches the two components CI installs; local runs will pick
  up the same set on first `cargo` invocation via `rustup`.
- **No `targets` field.** Round-1 evaluation flagged that
  `targets = ["aarch64-apple-darwin"]` would apply on every CI leg
  (Linux/Windows) and on any non-Apple dev machine, causing rustup to
  auto-download an unused Apple std. Harmless but wasteful, and
  contradicts the file's own rationale of a minimal local pin. Workflow
  `targets: ${{ matrix.target }}` already installs the correct target on
  each CI leg, and cross-compilation is not a supported local flow. If a
  future planning cycle decides to support cross-compilation locally, it
  may re-add `targets` at that point.
- The file is TOML, not JSON or YAML. Rustup honours both `rust-toolchain`
  (legacy, string content) and `rust-toolchain.toml` (structured). Use the
  `.toml` form because it lets us pin components in one place.

### 5. Cross-check documentation references

Spec 03 (`.docs/spec/03-toolchain-and-gate.md`) currently states "Rust
stable, edition 2024, MSRV 1.85". After this slice lands, "Rust stable" is
no longer accurate — it is now "Rust 1.98.0, bumped monthly under the
Slice A2 policy". Spec 03 is Draft-status but frozen against unplanned
edits under the planner-role contract. This slice therefore does **not**
edit spec 03. Instead, the finalize pass — the same finalize step the
planner role runs on approval — must:

- Update `.docs/status/handoff.md` / `.docs/status/roadmap.md` to record
  that spec 03's toolchain description is stale relative to the merged
  workflow, and queue a spec 03 revision cycle.
- **Queue slice A3** (the bump-PR automation deferred above) as a follow-up
  in the same handoff entry, naming the two mechanical defects a future
  planner must resolve (PAT vs `GITHUB_TOKEN` and post-merge dispatch
  verification).
- Not attempt to edit spec 03 from this slice-plan; if the finalize pass
  discovers the stale prose materially misleads a subsequent slice's
  planner, escalate as a Needs Clarification cycle rather than in-slice.

`CLAUDE.md` describes MSRV 1.85 as the compilation floor, not the current
compiler. **No change needed** — pinning to `1.98.0` does not change the
MSRV floor and CLAUDE.md's statement remains accurate.

`README.md` (if present at repo root) — no toolchain-shape claims to
update; verify with `grep -n 'rust-toolchain\|rustc' README.md` and if
matches are found, revisit.

## Verification

The verification for a workflow slice is the same shape as slice A: local
pre-push checks plus the CI run on the PR itself.

### Local (pre-push)

1. **YAML lint** — `actionlint .github/workflows/ci.yml
   .github/workflows/release.yml`. Fallback is a `python -c 'import yaml;
   …'` syntactic check as in slice A step §Verification §Local. Verifies
   steps 2–3 for syntax.
2. **`rust-toolchain.toml` parses** — `cargo --version` in the repo root
   after adding the file. `cargo` reads `rust-toolchain.toml` and either
   installs or reports the pinned toolchain; if the file is malformed,
   `cargo` errors out with a parse diagnostic.
3. **Existing spec 03 gate** — `cargo fmt --check`,
   `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
   -D warnings`, `RUST_MIN_STACK=268435456 cargo test --workspace`.
   These are the invariant floor spec 06 restates ("CI gate stays green").
   They must pass unchanged on the pinned toolchain. If any lint newly
   fires on `1.98.0` compared to whatever local rustc the developer had
   installed before, the lint is fixed *in this slice* — the fix commit
   is the whole point of picking the pin.
4. **Grep for stragglers** — `grep -Rn 'rust-toolchain@stable'
   .github/workflows/` must return zero lines. `grep -Rn
   'dtolnay/rust-toolchain@1.98.0' .github/workflows/` must return
   exactly six lines (one in `ci.yml`, five in `release.yml`).

### Remote (on the PR that lands this slice)

- **A2-a (workflow pin acceptance)** — CI matrix (`macos-14`,
  `ubuntu-latest`, `windows-latest`) all three legs pass. Each leg's
  "Install Rust" step logs the version (rustc's own `--version` output
  during the step), and the logged version equals `1.98.0` on every leg.
  If any leg reports a different resolved version, the pin is malformed
  and the slice is not mergeable as-is.
- **A2-b (rust-toolchain.toml acceptance)** — a fresh clone + `cargo
  --version` on a Rust developer's machine reports `1.98.0`, not the
  developer's rustup default. Verified locally by the slice's author
  before push (`rustup default 1.85.0 && cd <repo> && cargo --version` —
  should report `1.98.0` regardless of the rustup default).

### Gate (spec 03 + slice-specific)

| Step | Command | Where |
|------|---------|-------|
| format | `cargo fmt --all -- --check` | local + CI (unchanged from slice A) |
| lint (CI) | `RUST_MIN_STACK=268435456 cargo clippy --workspace -- -D warnings` | CI on the **pinned** toolchain |
| lint (local) | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | local on the **pinned** toolchain via `rust-toolchain.toml` |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` (CI uses `cargo nextest run --workspace`) | local + CI |
| deny | `cargo deny check` | local + CI (unchanged from slice A) |
| yaml | `actionlint .github/workflows/*.yml` | local (pre-push) |
| pin scan | `grep -R 'rust-toolchain@stable' .github/workflows/` returns nothing | local (pre-push) |

No new automated Rust tests are added by this slice. Spec 06's invariant
"no decrease in test count" holds because this is a pure workflow /
documentation change — the same explicit exception spec 06 grants for the
0006 slice ("carries at least one new automated test unless the fix is a
pure workflow / documentation change").

## Acceptance criteria (mirrors spec 06 A2 verbatim, plus concrete numbers)

1. **No `@stable` remaining.** `grep -R 'dtolnay/rust-toolchain@stable'
   .github/workflows/` returns zero lines at the tip of the slice branch.
2. **Pin consistency.** Every `dtolnay/rust-toolchain@` reference under
   `.github/workflows/` uses `@1.98.0`; `rust-toolchain.toml`'s
   `channel` equals `1.98.0`. Concrete check:
   `sed -n 's|.*dtolnay/rust-toolchain@\([0-9][0-9.]*\).*|\1|p'
   .github/workflows/*.yml | sort -u` returns exactly one line: `1.98.0`.
   The `[0-9][0-9.]*` pattern (non-empty, digit-anchored) is deliberately
   tighter than a `[0-9.]*` (which matches the empty string) so this
   check remains correct if `toolchain-bump.yml` — added later by slice
   A3 — introduces `dtolnay/rust-toolchain@${VAR}` shell-literal lines.
3. **CI green on the pin.** All three matrix legs pass with `-D warnings`
   enforced on rustc `1.98.0`.
4. **Cadence discoverable.** The inline comment at the pin site in
   `ci.yml` (step 2) names the cadence ("monthly bump PR (owner
   direction); enforcing automation deferred to slice A3") and points at
   spec 06 A2 as the authority. The same comment appears above the first
   pin site in `release.yml` and inside `rust-toolchain.toml`.
5. **Follow-ups queued.** The finalize pass records (a) a spec 03
   revision as pending and (b) slice A3 (bump-PR automation) as
   pending, per step 5.

Slice A2 is Complete when 1–5 are all observably true in CI runs against
`main` at the tip of the merged slice.

## Notes

_None._ If a role has a clarifying question, add a dated entry here and
set the artifact status to `Needs Clarification` per the loom role
contract.
