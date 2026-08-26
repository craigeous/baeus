# Slice A2 — CI Toolchain Pin

Status: Plan Review
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

**In scope (exactly the four deliverables spec 06 A2 lists):**

- **Pin workflow toolchain refs.** Replace every
  `uses: dtolnay/rust-toolchain@stable` in `ci.yml` and `release.yml` with
  an explicit rustc version pin (`dtolnay/rust-toolchain@1.98.0`, see step 1
  for version selection).
- **Add repo-root `rust-toolchain.toml`.** Slice A2 includes this file (spec
  06 A2 fix approach item 2 is described as "optional, recommended by the
  slice-plan"; this plan takes the recommendation). It locks local
  developers to the same rustc, closing the exact local/CI asymmetry that
  slice A exposed (spec 06 A2 Problem paragraph).
- **Document upgrade cadence.** Spec 06 A2 defers the choice between
  monthly and quarterly to the slice-plan. This plan picks **monthly**
  (owner default) and records the policy in two places: an inline comment
  at the pin sites, and a short note in this slice-plan's Verification
  section. Editing `.docs/spec/03-toolchain-and-gate.md` requires a fresh
  planning cycle (spec 03 is Draft-status but frozen once Approved; ADR
  0005 rule: an approved spec is frozen — even for Draft specs the
  planner-role rule applies once a spec has been evaluated). This
  slice-plan queues a spec 03 revision as a **follow-up** in the finalize
  pass, not an in-slice edit.
- **Optional scheduled bump-PR workflow.** Since owner direction is
  monthly, this slice adds a minimal
  `.github/workflows/toolchain-bump.yml` scheduled workflow that opens a
  bump PR on the 1st of each month. This is a mechanical extension of the
  cadence policy: if the policy is unenforced, drift returns in a slower
  form (nobody bumps and the pin ages out of security support). See step 4.

**Explicit non-goals:**

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
- Any change to the `dtolnay/rust-toolchain` action reference itself (e.g.
  pinning it to a SHA digest — medium finding 0006 §7, explicitly deferred
  per spec 06 Out of scope). This slice pins the *toolchain version input*
  the action consumes, not the action's own version ref.
- Spec 03 edits (queued as a finalize follow-up, not in-slice).

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order. Steps 1–3 are the pin. Step 4 is the optional monthly bump-PR
workflow (owner-directed cadence enforcement). Step 5 is the documentation
cross-check.

### 1. Select the pin version

Read the rustc version resolved by the *current* CI run for slice A's merge
commit (`5e90381`) from the "Install Rust" step's output — it prints the
resolved toolchain via `rustc --version`. That is the version slice A landed
against and is therefore the last known-green stable. If unavailable from
CI logs, run `rustup run stable rustc --version` on a matching runner OS
before the pin edit — but prefer the CI-log path because it eliminates
local/CI drift as a data source.

At time of writing, the last CI-observed rustc is `1.98.0` (per slice A's
in-slice fix commit `3b518de` message: "1.94 → 1.98"). The slice-plan
evaluator should verify by re-reading the CI log linked from PR #5 before
approving. If the current live stable has drifted to `1.99.x` or higher, the
slice-plan may either:

- Pin to `1.98.0` (last CI-observed green) and let the first monthly bump
  PR move us forward. **This is the default.**
- Pin to whatever `rustup update stable && rustc --version` reports on
  local at planning time. Requires a fresh `cargo clippy --workspace
  --all-targets -- -D warnings` pass locally before the pin.

**Decision:** default to `1.98.0` unless the evaluator's re-check shows CI
has already moved past it during the planning window; if so, use the newest
stable that a matching local `cargo clippy --workspace --all-targets --
-D warnings` passes on.

The rest of this slice-plan uses `<VERSION>` as a stand-in for the concrete
minor version the developer commits. Every occurrence of `<VERSION>` in the
edits below must be the same string.

### 2. `.github/workflows/ci.yml` — pin the toolchain ref

The file has one `Install Rust` step (line 32–36 in the current tree):

```yaml
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
          components: rustfmt, clippy
```

Change `@stable` to `@<VERSION>`. Do **not** change `components:` or
`targets:`. Add an inline comment above the `uses:` line stating the pin
rationale, the cadence policy, and the bump-PR trigger (per spec 06 A2
"Upgrade cadence policy — record in a location the code evaluator can
verify"):

```yaml
      # Pinned to Rust <VERSION> per spec-06 Slice A2 (2026-08-26). Do not
      # bump to @stable — the monthly `toolchain-bump` workflow (step 4)
      # opens a PR bumping this pin on the 1st of each month. Hand-editing
      # this line outside that PR flow breaks the drift invariant.
      - name: Install Rust
        uses: dtolnay/rust-toolchain@<VERSION>
        with:
          targets: ${{ matrix.target }}
          components: rustfmt, clippy
```

Result: one edit in `ci.yml`.

### 3. `.github/workflows/release.yml` — pin every toolchain ref

`release.yml` uses `dtolnay/rust-toolchain@stable` in five jobs. Grep before
editing to make sure the count is correct:

```
grep -n 'dtolnay/rust-toolchain@' .github/workflows/release.yml
```

Expected: five occurrences (per the current file: line 50 in `build-macos`,
127 in `check-linux`, 173 in `build-linux`, 223 in `check-windows`, 258 in
`build-windows`). If the count differs, stop and reconcile before editing —
divergence means the file changed since planning and the plan needs a
refresh.

Apply the same `@stable → @<VERSION>` swap to each. Add the same inline
comment (verbatim) above **only the first** occurrence (`build-macos`);
subsequent occurrences may omit the comment to avoid noise, but each must
match the version pinned on line 1. This is a mechanical five-way edit; a
`sed -i '' 's|dtolnay/rust-toolchain@stable|dtolnay/rust-toolchain@<VERSION>|g'
.github/workflows/release.yml` is acceptable so long as the grep after the
edit shows zero remaining `@stable` refs anywhere in `.github/workflows/`.

### 4. `rust-toolchain.toml` at repo root

Create a new file `rust-toolchain.toml` at the repo root with exactly this
content:

```toml
# Pinned to Rust <VERSION> per spec-06 Slice A2 (2026-08-26). This file locks
# local developer builds to the same compiler CI runs against, so no slice
# hits the "green local, red CI" class slice A did. Bumping is done via
# the monthly toolchain-bump PR — do not hand-edit outside that flow.
[toolchain]
channel = "<VERSION>"
components = ["rustfmt", "clippy"]
targets = ["aarch64-apple-darwin"]
```

Notes constraining this file:

- `channel = "<VERSION>"` **must** match the workflow pin exactly (spec 06 A2
  acceptance criterion: "The workflow-pin version matches the
  `rust-toolchain.toml` channel").
- `components` matches the two components CI installs; local runs will pick
  up the same set on first `cargo` invocation via `rustup`.
- `targets` includes only `aarch64-apple-darwin` — the local development
  target. CI's Linux and Windows targets are installed by the workflow's
  `targets: ${{ matrix.target }}` and do not need to be in this file
  (adding them would trigger unnecessary target downloads for every local
  developer). Add ubuntu / windows targets here only if a future planning
  cycle decides to support cross-compilation locally.
- The file is TOML, not JSON or YAML. Rustup honours both `rust-toolchain`
  (legacy, string content) and `rust-toolchain.toml` (structured). Use the
  `.toml` form because it lets us pin components/targets in one place.

### 5. `.github/workflows/toolchain-bump.yml` — scheduled monthly bump PR (owner cadence enforcement)

Create a new workflow file that opens a monthly PR bumping the pin. This
enforces the "monthly" cadence spec 06 A2 defers to the slice-plan — without
enforcement, "monthly" is aspiration, not policy.

```yaml
name: Toolchain Bump

on:
  schedule:
    # 09:00 UTC on the 1st of every month.
    - cron: '0 9 1 * *'
  workflow_dispatch: {}

permissions:
  contents: write
  pull-requests: write

jobs:
  open-bump-pr:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Resolve latest stable rustc version
        id: latest
        run: |
          curl -sSf https://static.rust-lang.org/dist/channel-rust-stable.toml \
            | grep -m1 '^version = ' \
            | sed -E 's/^version = "([0-9.]+).*"/\1/' \
            | { read v; echo "version=${v}" >> "$GITHUB_OUTPUT"; }

      - name: Compute current pin
        id: current
        run: |
          CURRENT=$(grep -m1 'dtolnay/rust-toolchain@' .github/workflows/ci.yml \
                     | sed -E 's|.*rust-toolchain@([0-9.]+).*|\1|')
          echo "version=${CURRENT}" >> "$GITHUB_OUTPUT"

      - name: Short-circuit if pin is already latest
        if: steps.latest.outputs.version == steps.current.outputs.version
        run: |
          echo "Pin already at ${{ steps.latest.outputs.version }} — nothing to do."
          echo "SKIP=1" >> "$GITHUB_ENV"

      - name: Rewrite pin in workflow files + rust-toolchain.toml
        if: env.SKIP != '1'
        run: |
          OLD="${{ steps.current.outputs.version }}"
          NEW="${{ steps.latest.outputs.version }}"
          sed -i "s|dtolnay/rust-toolchain@${OLD}|dtolnay/rust-toolchain@${NEW}|g" \
            .github/workflows/ci.yml .github/workflows/release.yml
          sed -i "s|channel = \"${OLD}\"|channel = \"${NEW}\"|" rust-toolchain.toml

      - name: Open bump PR
        if: env.SKIP != '1'
        uses: peter-evans/create-pull-request@v6
        with:
          branch: toolchain/bump-${{ steps.latest.outputs.version }}
          title: "chore: bump Rust toolchain pin to ${{ steps.latest.outputs.version }}"
          body: |
            Monthly toolchain pin refresh per spec-06 Slice A2 policy.

            Old: ${{ steps.current.outputs.version }}
            New: ${{ steps.latest.outputs.version }}

            The full CI matrix must pass on this PR before landing. If new
            clippy/rustc lints appear, fix them in this PR — do not defer.
          commit-message: "chore: bump Rust toolchain pin to ${{ steps.latest.outputs.version }}"
          delete-branch: true
```

Notes constraining this step:

- `peter-evans/create-pull-request` is not currently in the workflow tree;
  spec 06 Out of scope defers SHA-pinning of Actions (medium 0006 §7). Use
  the `@v6` pin shape consistent with peer Actions in the file (`@v4`,
  `@v6`); this matches slice A's precedent for `@vN` pinning.
- The `curl` to `static.rust-lang.org/dist/channel-rust-stable.toml`
  fetches the current stable version from the Rust project's own
  distribution manifest, which is the authoritative source. No third-party
  dependency, no rate-limited API.
- `sed -i` (GNU sed) syntax works on `ubuntu-latest`; do not port this
  workflow to macOS runners (BSD sed differs on `-i ''` empty backup arg).
- The `Short-circuit` step avoids opening an empty PR when the pin is
  already current — the workflow may fire during a lull between stable
  releases.
- `workflow_dispatch: {}` lets a human trigger the bump manually if the
  monthly schedule slips.

If the evaluator decides the bump-PR workflow is out of scope for this
slice — e.g. because it introduces a new Action dependency that spec 06's
Out-of-scope clause on SHA pinning tangentially touches — step 5 can be
deferred to a follow-up slice A3 and the "cadence enforcement" reduced to
the inline comments in steps 2 and 4. The pin itself (steps 1–4) is the
mandatory floor; step 5 is the recommended-but-severable enforcement.

### 6. Cross-check documentation references

Spec 03 (`.docs/spec/03-toolchain-and-gate.md`) currently states "Rust
stable, edition 2024, MSRV 1.85". After this slice lands, "Rust stable" is
no longer accurate — it is now "Rust `<VERSION>`, bumped monthly under the
Slice A2 policy". Spec 03 is Draft-status but frozen against unplanned
edits under the planner-role contract. This slice therefore does **not**
edit spec 03. Instead, the slice's finalize pass — the same finalize step
the planner role runs on approval — must:

- Update `.docs/status/handoff.md` / `.docs/status/roadmap.md` to record
  that spec 03's toolchain description is now stale relative to the merged
  workflow and queues a spec 03 revision cycle.
- Not attempt to edit spec 03 from this slice-plan; if the finalize pass
  discovers the stale prose materially misleads a subsequent slice's
  planner, escalate as a Needs Clarification cycle rather than in-slice.

`CLAUDE.md` describes MSRV 1.85 as the compilation floor, not the current
compiler. **No change needed** — pinning to `<VERSION>` does not change the
MSRV floor and CLAUDE.md's statement remains accurate.

`README.md` (if present at repo root) — no toolchain-shape claims to
update; verify with a `grep -n 'rust-toolchain\|rustc' README.md` and if
matches are found, revisit.

## Verification

The verification for a workflow slice is the same shape as slice A: local
pre-push checks plus the CI run on the PR itself.

### Local (pre-push)

1. **YAML lint** — `actionlint .github/workflows/ci.yml
   .github/workflows/release.yml .github/workflows/toolchain-bump.yml`
   (add `toolchain-bump.yml` if step 5 is included). Fallback is a
   `python -c 'import yaml; …'` syntactic check as in slice A step §
   Verification § Local. Verifies steps 2–5 for syntax.
2. **`rust-toolchain.toml` parses** — `cargo --version` in the repo root
   after adding the file. `cargo` reads `rust-toolchain.toml` and either
   installs or reports the pinned toolchain; if the file is malformed,
   `cargo` errors out with a parse diagnostic.
3. **Existing spec 03 gate** — `cargo fmt --check`,
   `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
   -D warnings`, `RUST_MIN_STACK=268435456 cargo test --workspace`.
   These are the invariant floor spec 06 restates ("CI gate stays green").
   They must pass unchanged on the pinned toolchain. If any lint newly
   fires on `<VERSION>` compared to whatever local rustc the developer had
   installed before, the lint is fixed *in this slice* — the fix commit
   is the whole point of picking the pin.
4. **Grep for stragglers** — `grep -Rn 'rust-toolchain@stable'
   .github/workflows/` must return zero lines. `grep -Rn '@<VERSION>'
   .github/workflows/` must return six lines (one in `ci.yml`, five in
   `release.yml`) or seven if step 5 references it as well (the bump
   workflow does not itself pin toolchain — it edits the pins in the two
   consumer files — so it should not add a seventh occurrence in
   toolchain-bump.yml).

### Remote (on the PR that lands this slice)

- **A2-a (workflow pin acceptance)** — CI matrix (`macos-14`,
  `ubuntu-latest`, `windows-latest`) all three legs pass. Each leg's
  "Install Rust" step logs the version, and the logged version equals
  `<VERSION>` on every leg. If any leg reports a different resolved
  version, the pin is malformed and the slice is not mergeable as-is.
- **A2-b (rust-toolchain.toml acceptance)** — a fresh clone + `cargo
  --version` on a Rust developer's machine reports `<VERSION>`, not the
  developer's rustup default. Verified locally by the slice's author
  before push (`rustup default 1.85.0 && cd <repo> && cargo --version` —
  should report `<VERSION>` regardless of the rustup default).
- **A2-c (monthly bump-PR wiring, only if step 5 is included)** — a
  manual `workflow_dispatch` invocation of `toolchain-bump.yml` on the
  slice branch opens a PR whose diff modifies exactly the three files
  listed in step 5. If the current pin already matches upstream stable,
  the workflow short-circuits and no PR is opened; that is also a valid
  observation of A2-c (the short-circuit is part of the design).

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
   `.github/workflows/` uses `@<VERSION>`; `rust-toolchain.toml`'s
   `channel` equals `<VERSION>`. `sed -n
   's|.*rust-toolchain@\([0-9.]*\).*|\1|p' .github/workflows/*.yml | sort
   -u | wc -l` returns 1.
3. **CI green on the pin.** All three matrix legs pass with `-D warnings`
   enforced on rustc `<VERSION>`.
4. **Cadence discoverable.** The inline comment at the pin site in
   `ci.yml` (step 2) names the cadence ("monthly bump PR via
   `toolchain-bump.yml`") and points at spec 06 A2 as the authority.
5. **Follow-up queued.** The finalize pass records a spec 03 revision as
   pending, per step 6.

Slice A2 is Complete when 1–5 are all observably true in CI runs against
`main` at the tip of the merged slice.

## Notes

_None._ If a role has a clarifying question, add a dated entry here and
set the artifact status to `Needs Clarification` per the loom role
contract.
