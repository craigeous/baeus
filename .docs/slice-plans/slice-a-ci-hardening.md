# Slice A — CI & Release Hygiene

Status: Draft
Target specs: `.docs/spec/06-remediation-highs.md` (findings 0006-H1, 0006-H2, 0006-H3, 0006-H4); gate defined by `.docs/spec/03-toolchain-and-gate.md`.

## Context

The PR gate at `.github/workflows/ci.yml` is macOS-only, runs no `rustfmt --check`,
and never invokes `cargo deny` despite `deny.toml` (Approved) declaring the audit
policy. `.github/workflows/release.yml` generates its release tag in three
independent per-runner steps that hardcode `v0.1.0-dev` and can drift by date or
by workspace version bump. Spec 06 groups these four workflow-hygiene highs into
**Slice A** because it is small, isolated to two YAML files, and unblocks every
subsequent slice by making fmt/clippy/deny load-bearing before Slices B–G land.

**In scope (exactly the four Slice A findings):**

- **0006-H1** — Add `cargo deny check` to `ci.yml`, and extend
  `on.pull_request.paths:` to include `deny.toml` and `.github/workflows/**`
  (the minimum needed for the new gate to fire; spec 06 authorises exactly this
  filter extension and no more).
- **0006-H2** — Add `rustfmt` to the toolchain components and run
  `cargo fmt --all -- --check` as the first gate step, matching spec 03's
  `format → lint → test` order.
- **0006-H3** — Convert the PR `check` job into a matrix over
  `{ macos-14, ubuntu-latest, windows-latest }` and gate the PR on all three.
- **0006-H4** — Replace the three per-runner `Generate release tag` steps in
  `release.yml` with a single `compute-tag` job whose outputs (`tag`, `date`,
  `short_sha`, `version`) the three downstream jobs consume; drive `version`
  from workspace `Cargo.toml` (not the literal `0.1.0`).

**Explicit non-goals (deferred per spec 06's Out-of-scope section):**

- `.cargo/config.toml` `[env]` block for `RUST_MIN_STACK` (medium 0006 §8).
- Pinning Actions to full SHA digests (medium 0006 §7); the new
  `cargo-deny` install action follows the existing `@vN` pattern used by
  peer Actions in the file — this is spec 06's explicit instruction.
- `deny.toml` bans-policy tightening (`multiple-versions`, `wildcards` — medium
  0006 §6). This slice adds no edits to `deny.toml`; it is only a trigger path.
- macOS `.app` signing / notarization (medium 0006 §5).
- `macos/Info.plist` dynamic version (low 0006 §11); `compute-tag` derives the
  workspace version for the release tag only, not for the `.app` bundle
  metadata.
- Broader trigger-path redesign beyond the two additions above (partial
  remediation of medium 0006 §9 — the rest is deferred).
- All non-Slice-A highs (0002-H1..H4, 0003-H1..H2, 0004-H1..H2, 0005-H1..H5) —
  those are Slices B–G.
- New Rust code, new tests, or any change under `crates/`.

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order within the slice; steps 1–4 are the ci.yml pass, step 5 is release.yml,
step 6 is documentation-cross-check.

### 1. `.github/workflows/ci.yml` — extend PR trigger `paths:` (0006-H1 filter extension)

Edit the `on.pull_request.paths:` block at lines 6–9 to add two entries.
Result:

```yaml
on:
  pull_request:
    branches: [main]
    paths:
      - 'crates/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'deny.toml'
      - '.github/workflows/**'
```

Rationale: without this the deny step introduced in step 3 will not run on
PRs that only touch `deny.toml` or the workflow itself, and the new matrix
introduced in step 4 will not self-validate on workflow edits.

### 2. `.github/workflows/ci.yml` — add `rustfmt` component + fmt gate step (0006-H2)

In the `Install Rust` step (currently at line 20–24) change the
`components:` list from `clippy` to `rustfmt, clippy`. Insert a new step
**before** the existing `Clippy` step, matching spec 03's `format → lint →
test` ordering:

```yaml
      - name: Format check
        run: cargo fmt --all -- --check
```

`cargo fmt --check` does not need `RUST_MIN_STACK` (rustfmt is not a GPUI
consumer) so the step omits the env block; keep `RUST_MIN_STACK: "268435456"`
on the Clippy and Test steps unchanged.

Pre-landing check: run `cargo fmt --all -- --check` locally on `main` at the
tip of this slice's branch. If it reports diffs, add a `cargo fmt --all`
commit to the same slice before the CI edit lands (so the first CI run under
the new gate is green). The check runs cheaply and does not require the GPUI
build cache.

### 3. `.github/workflows/ci.yml` — add `cargo deny check` step (0006-H1)

Insert two steps after the toolchain install and before or in parallel with
Clippy (order not critical once fmt runs first). Between `Install Rust` and
`Install cargo-nextest`:

```yaml
      - name: Install cargo-deny
        uses: taiki-e/install-action@cargo-deny
```

After the `Format check` step and before `Clippy`:

```yaml
      - name: cargo-deny
        run: cargo deny check
```

`cargo deny check` covers advisories, licenses, bans, and sources per the
existing `deny.toml`. It runs against the checked-in `Cargo.lock` and does
not require a build; it is fast and can run before the heavy clippy/test
steps. Follow the `@vN`-style pin used by the existing
`taiki-e/install-action@nextest` (SHA pinning is medium 0006 §7 and out of
scope).

### 4. `.github/workflows/ci.yml` — three-OS matrix (0006-H3)

Convert the `check` job's runner from a hardcoded `macos-14` (line 16) to a
matrix strategy. Concrete shape:

```yaml
jobs:
  check:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
          components: rustfmt, clippy

      - name: Install Linux dependencies
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            gcc g++ make cmake clang mold \
            libasound2-dev libfontconfig-dev libgit2-dev \
            libglib2.0-dev libssl-dev libva-dev \
            libvulkan1 libwayland-dev libx11-xcb-dev \
            libxkbcommon-x11-dev libzstd-dev libstdc++-14-dev \
            libsqlite3-dev

      - name: Install cargo-deny
        uses: taiki-e/install-action@cargo-deny

      - name: Install cargo-nextest
        uses: taiki-e/install-action@nextest

      - name: Cache cargo registry & build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-ci-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-ci-

      - name: Format check
        run: cargo fmt --all -- --check

      - name: cargo-deny
        run: cargo deny check

      - name: Clippy
        env:
          RUST_MIN_STACK: "268435456"
        run: cargo clippy --workspace -- -D warnings

      - name: Test
        env:
          RUST_MIN_STACK: "268435456"
        run: cargo nextest run --workspace
```

Notes constraining this step:

- `fail-fast: false` ensures a Linux/Windows-only regression is fully
  reported instead of masked by an early macOS pass.
- Linux `apt-get install` list is copied verbatim from
  `release.yml:112-121` — the same runtime deps required by GPUI/kube/git2
  on Ubuntu today; no discovery.
- Windows needs no OS-specific install step beyond the toolchain; matches
  `release.yml:204-237`.
- Cache key gains `${{ runner.os }}` so the three OSes do not collide on a
  single macOS-only key (the current key `macos-arm64-ci-...` is retired
  in favour of the per-OS pattern).
- `RUST_MIN_STACK` is retained on Clippy and Test on every OS (Spec 03
  requires it for GPUI proc-macro compilation regardless of platform).
- `cargo fmt --all -- --check`, `cargo deny check`, `cargo clippy`, and
  `cargo nextest run --workspace` all run per-OS. On Windows and Linux this
  is the first time the workspace faces those gates; per spec 06 acceptance
  for 0006-H3, "Windows-specific tolerances… should surface as build
  failures the first time this lands — that is the intended point of adding
  the gate." Any per-OS test adjustment discovered on the first run is
  additive to this slice's acceptance (spec 06, 0006-H3 test expectations).

### 5. `.github/workflows/release.yml` — single `compute-tag` job (0006-H4)

Introduce a lightweight job at the top of the workflow that resolves the
workspace version once and pins `DATE`/`SHORT_SHA` once, exposing them as
outputs. The three build jobs (`build-macos`, `build-linux`, `build-windows`)
gain a `needs: [compute-tag]` dependency and reference the shared outputs.

New job (inserted before `build-macos`):

```yaml
jobs:
  compute-tag:
    runs-on: ubuntu-latest
    outputs:
      tag: ${{ steps.tag.outputs.tag }}
      version: ${{ steps.tag.outputs.version }}
      date: ${{ steps.tag.outputs.date }}
      short_sha: ${{ steps.tag.outputs.short_sha }}
    steps:
      - uses: actions/checkout@v4

      - name: Compute release tag
        id: tag
        run: |
          SHORT_SHA="${GITHUB_SHA::7}"
          DATE=$(date -u +%Y%m%d)
          VERSION=$(cargo metadata --no-deps --format-version 1 \
                      | jq -r '.packages
                                | map(select(.name == "baeus-app"))
                                | .[0].version')
          TAG="v${VERSION}-dev.${DATE}.${SHORT_SHA}"
          {
            echo "short_sha=${SHORT_SHA}"
            echo "date=${DATE}"
            echo "version=${VERSION}"
            echo "tag=${TAG}"
          } >> "$GITHUB_OUTPUT"
```

Notes constraining this step:

- The `jq` filter selects the `baeus-app` package explicitly rather than
  `.packages[0]` (which is order-dependent in a multi-package workspace).
  `baeus-app` is the application entry point (workspace `Cargo.toml`); its
  version is the release version.
- The `ubuntu-latest` runner is chosen because it has `jq` and `cargo`
  pre-installed and is the cheapest of the three; no build occurs here.
- Rust toolchain install is not required — `cargo metadata` on a stock
  runner works with the pre-installed stable toolchain that GitHub provides.
  If a first-run failure shows otherwise, add `dtolnay/rust-toolchain@stable`
  as an additive fix within the slice.

Downstream jobs — three edits, each replacing the local `Generate release
tag` step (`release.yml:73-79`, `191-196`, `272-279`) with a `needs:` clause
plus references to the shared outputs:

For `build-macos`:

```yaml
  build-macos:
    needs: [compute-tag]
    runs-on: macos-14
    steps:
      # ... (existing checkout, toolchain, build, bundle, dmg steps unchanged)
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.compute-tag.outputs.tag }}
          name: Baeus ${{ needs.compute-tag.outputs.tag }}
          body: |
            Automated build from `main` (${{ github.sha }}).
            # ... unchanged body ...
          files: target/release/Baeus-macos-arm64.dmg
          prerelease: true
          make_latest: true
```

Delete the local `Generate release tag` step (lines 73–79) entirely.

For `build-linux` (currently `needs: [check-linux]`) — retain the existing
`check-linux` dependency and add `compute-tag` (`needs: [check-linux, compute-tag]`).
Delete the local `Generate release tag` step (lines 190–196). Change the
upload step's `tag_name: ${{ steps.tag.outputs.tag }}` to
`tag_name: ${{ needs.compute-tag.outputs.tag }}`.

For `build-windows` — mirror the `build-linux` edits
(`needs: [check-windows, compute-tag]`; delete lines 272–279; upload
`tag_name` references `needs.compute-tag.outputs.tag`).

After these edits, the string `v0.1.0-dev` should no longer appear anywhere
in `release.yml`; grep confirms.

### 6. Cross-check documentation references

Verify that no doc file describes CI in a way this slice contradicts:

- Spec 03 (`.docs/spec/03-toolchain-and-gate.md:26-28`) already names the
  correct three-step gate. **No change needed** — this slice is what makes
  reality match the spec's description.
- `CLAUDE.md` describes `cargo clippy --workspace -- -D warnings` and
  `cargo test --workspace` as the local gate. **No change needed** — the
  slice adds no new required local step (`cargo deny check` is a
  recommended local check but not the required local gate).
- `README.md` (if present at repo root) — no CI-shape claims to update.

If step 6 discovers that spec 03 or another approved artifact must change
to reflect the new CI shape, that is a spec revision — stop and raise
`Needs Clarification` rather than editing spec 03 from a slice-plan.

## Verification

The verification for a CI slice is unusual: the artifact under test **is**
CI itself. Verification splits into local (pre-push) and remote (the CI run).

### Local (pre-push)

Run each command from repo root on the slice branch. All four must pass
before the branch is pushed for review:

1. **YAML lint** — `actionlint .github/workflows/ci.yml
   .github/workflows/release.yml`. Any developer using nix, Homebrew, or the
   Docker image can run this; if `actionlint` is not installed locally the
   fallback is a syntactic YAML check via `python -c 'import yaml, sys;
   yaml.safe_load(open(f)) for f in sys.argv[1:]' .github/workflows/*.yml`
   plus a manual re-read. Verifies steps 1–5 for syntax and expression
   correctness.
2. **Format gate is achievable** — `cargo fmt --all -- --check`. If this
   fails locally today, add `cargo fmt --all` as a same-slice commit
   (spec 06, 0006-H2 acceptance: "if it does not, add a fmt commit within
   the same slice").
3. **Deny gate is achievable** — `cargo deny check` locally with cargo-deny
   installed (`cargo install cargo-deny --locked` or `brew install
   cargo-deny`). Must exit zero against the current `Cargo.lock`. If it
   surfaces a new advisory or license issue, either update `deny.toml`
   suppressions with a documented reason (still within Slice A because
   `deny.toml` is a Slice A concern for making the gate green) or stop and
   flag it as a real dependency issue requiring a separate cycle.
4. **Existing spec 03 gate** — `cargo fmt --check`,
   `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
   -D warnings`, `RUST_MIN_STACK=268435456 cargo test --workspace`. These
   must pass unchanged on macOS; they are the invariant floor spec 06
   restates ("CI gate stays green").

### Remote (on the PR that lands this slice)

The PR itself is the acceptance test. The following must be observable in
the PR's Checks tab:

- **A1 (0006-H1 acceptance)** — A `cargo-deny` check runs on all three
  OSes and passes. Manual verification that the trigger fires on
  `deny.toml`- and `.github/workflows/**`-only edits: the slice's PR
  edits both categories (it touches `.github/workflows/ci.yml` and
  `.github/workflows/release.yml`, and no `crates/**`), so the PR
  itself proves the trigger fires. A workflow-only PR that did **not**
  trigger CI would demonstrate failure of this acceptance.
- **A2 (0006-H2 acceptance)** — A `Format check` step is visible in the
  logs before `Clippy`, on all three OSes, and passes.
- **A3 (0006-H3 acceptance)** — Three matrix legs appear in Checks
  (`check (macos-14, aarch64-apple-darwin)`, `check (ubuntu-latest,
  x86_64-unknown-linux-gnu)`, `check (windows-latest, x86_64-pc-windows-msvc)`).
  All three must pass for the PR to be mergeable. If any leg fails on
  first run due to a genuine Linux/Windows regression, spec 06 permits
  fixing that regression within Slice A ("Any per-OS test adjustments
  discovered on the first run are additive to this slice's acceptance").
- **A4 (0006-H4 acceptance)** — Post-merge verification. After the PR
  lands on `main`, the `release.yml` run triggered by that merge shows:
  (a) a `compute-tag` job whose outputs three downstream jobs consume,
  (b) a single GitHub Release created with all three artifacts under the
  same tag, (c) that tag begins with `v<workspace-version>-dev.` where
  `<workspace-version>` matches `baeus-app`'s version in `Cargo.toml`,
  (d) `grep -R 'v0.1.0-dev' .github/` returns no results.

  If a full push-to-main dry run is deemed too risky before landing, the
  cheaper pre-merge validation is to push the slice branch to a personal
  fork whose `release.yml` triggers on that branch (temporary edit reverted
  before opening the upstream PR), or use `act` locally to smoke-test the
  `compute-tag` job. Spec 06 (0006-H4 test expectations) explicitly calls
  for "a dry-run push to a throwaway branch" as the validation route.

### Gate (spec 03 + slice-specific)

Every slice must clear spec 03's format → lint → test gate. This slice adds
one slice-specific gate:

| Step | Command | Where |
|------|---------|-------|
| format | `cargo fmt --all -- --check` | local + CI (new in this slice) |
| lint | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | local + CI |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` (CI uses `cargo nextest run --workspace`) | local + CI |
| deny | `cargo deny check` | local + CI (new in this slice) |
| yaml | `actionlint .github/workflows/*.yml` | local (pre-push) |

No new automated Rust tests are added by this slice. Spec 06's invariant
"no decrease in test count across any slice" holds because the slice adds
no `#[test]` code and removes none; the workspace test count is unchanged.
Spec 06 explicitly permits this exception ("carries at least one new
automated test unless the fix is a pure workflow / documentation change
(0006 slice)").

## Acceptance criteria (consolidated from spec 06's Slice A findings)

Verbatim from spec 06 with rewording only for consolidation:

1. **0006-H1**
   a. The `on.pull_request.paths:` filter includes `crates/**`, `Cargo.toml`,
   `Cargo.lock`, `deny.toml`, and `.github/workflows/**`, and every PR that
   touches any of those paths triggers a `cargo deny check` run.
   b. A new RUSTSEC advisory that hits the workspace's transitive deps
   causes CI to fail (verified by running `cargo deny check` locally
   against the current `Cargo.lock`).
2. **0006-H2**
   a. Any PR whose files violate `rustfmt.toml` fails CI.
   b. Existing tree passes `cargo fmt --check` today (verified pre-merge;
   if not, an in-slice `cargo fmt --all` commit is added).
3. **0006-H3**
   a. A PR fails when Linux or Windows fails, even if macOS passes.
   b. Cache keys per-OS remain effective (`Cargo.lock` hash + `runner.os`).
4. **0006-H4**
   a. All release artifacts share the same tag.
   b. Bumping the workspace version in `Cargo.toml` results in the release
   tag reflecting the new version.
   c. The three per-runner "Generate release tag" steps are removed.

Slice A is Complete when 1a/b, 2a/b, 3a/b, 4a/b/c are all observably true
in CI/release runs against `main` at the tip of the merged slice.

## Notes

_None._ If a role has a clarifying question, add a dated entry here and
set the artifact status to `Needs Clarification` per the loom role
contract.
