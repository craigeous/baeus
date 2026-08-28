# Slice A — CI & Release Hygiene

Status: Landed
Target specs: `.docs/spec/06-remediation-highs.md` (findings 0006-H1, 0006-H2, 0006-H3, 0006-H4); gate defined by `.docs/spec/03-toolchain-and-gate.md`.

## Context

The PR gate at `.github/workflows/ci.yml` is macOS-only, runs no `rustfmt --check`,
and never invokes `cargo deny` despite `deny.toml` (Approved) declaring the audit
policy. `.github/workflows/release.yml` generates its release tag in three
independent per-runner steps that hardcode `v0.1.0-dev` and can drift by date or
by workspace version bump. Spec 06 groups these four workflow-hygiene highs into
**Slice A** because it is small, isolated to two YAML files, and unblocks every
subsequent slice by making fmt/clippy/deny load-bearing before Slices B–G land.

Mechanically verified today (see Verification § Local, and eval round 1):
`cargo deny check` fails against the current `Cargo.lock` with ten distinct
`error[vulnerability]` advisories (aws-lc-sys, rustls-webpki, h2, quick-xml,
crossbeam-epoch), six `error[unmaintained]` advisories (async-std, core2,
paste, proc-macro-error2, rustybuzz, ttf-parser), and four
`error[rejected]` license entries (`0BSD`, `NCSA`,
`Apache-2.0 WITH LLVM-exception`). The 0006-H1 acceptance criterion "the new
`cargo-deny` check … passes" is therefore unachievable without concrete
triage of each of those findings, so triage is a first-class in-scope
deliverable of this slice (see step 3 below and Deny-gate triage policy). This
mirrors the spec-06 rationale that pulled the `paths:` filter additions into
Slice A: making the new deny gate *fire* required the filter extension, and
making it *pass* requires the triage. Both are intrinsic to 0006-H1.

**In scope (exactly the four Slice A findings, plus deny-gate triage
intrinsic to H1's acceptance):**

- **0006-H1** — Add `cargo deny check` to `ci.yml`; extend
  `on.pull_request.paths:` to include `deny.toml` and `.github/workflows/**`
  (the minimum needed for the new gate to fire); **and** perform per-finding
  triage of the current `cargo deny check` failures against the checked-in
  `Cargo.lock` so the gate passes on first CI run. Triage output is edits to
  `deny.toml` (license allowlist additions and per-advisory justified
  ignores) and, where a newer registry version resolves an advisory,
  `Cargo.lock` bumps produced by targeted `cargo update -p <crate>` runs.
  No workspace `Cargo.toml` edits: bumps must be reachable within the
  existing semver constraints. See "Deny-gate triage policy" below for the
  binding rules on suppressions.
- **0006-H2** — Add `rustfmt` to the toolchain components and run
  `cargo fmt --all -- --check` as the first gate step, matching spec 03's
  `format → lint → test` order.
- **0006-H3** — Convert the PR `check` job into a matrix over
  `{ macos-14, ubuntu-latest, windows-latest }` and gate the PR on all three.
- **0006-H4** — Replace the three per-runner `Generate release tag` steps in
  `release.yml` with a single `compute-tag` job whose outputs (`tag`, `date`,
  `short_sha`, `version`) the three downstream jobs consume; drive `version`
  from workspace `Cargo.toml` (not the literal `0.1.0`).

Affected files (superset of spec 06's per-finding lists, expanded only for
H1's triage as authorised by spec 06's "CI gate stays green" invariant and
the same rationale that pulled the `paths:` filter into Slice A):
`.github/workflows/ci.yml`, `.github/workflows/release.yml`, `deny.toml`,
`Cargo.lock`. **No** edits to workspace `Cargo.toml`, any `crates/**`
`Cargo.toml`, or any `crates/**` source.

### Deny-gate triage policy (H1 intrinsic scope)

These rules govern every edit to `deny.toml` and every `cargo update -p`
invocation in step 3. They are binding: the developer executing this slice
must follow them, and the eval will fail the slice if any triage entry
violates them.

1. **No blanket suppressions.** `cargo deny check`'s `--allow`/`--deny`
   flags are not used. Every advisory suppression is a per-ID entry in
   `[advisories].ignore` with an explicit `reason = ...` string identifying
   (a) why the crate is in the tree (transitive-through-what), (b) why no
   in-slice fix is available (e.g., "pinned by kube-rs 0.98.0"), and
   (c) the removal trigger ("remove when kube-rs bumps rustls-webpki past
   0.101.7").
2. **No suppression of live vulnerabilities where a bump resolves them.**
   Before ignoring any `error[vulnerability]`, the developer must run
   `cargo update -p <crate>` (and, where the crate is transitive, the
   direct parent chain) and re-run `cargo deny check` to confirm the
   advisory persists. Only advisories that survive a targeted update — i.e.
   whose ceiling is pinned by a transitive constraint the slice does not
   own — may be ignored, and the reason string must name the pin.
3. **Unmaintained-crate ignores follow the existing precedent.** The
   current `deny.toml` already ignores three unmaintained advisories
   (backoff/instant/rustls-pemfile) with reasons naming their kube-rs
   provenance. New unmaintained ignores added by this slice must follow
   the same shape (per-ID entry, one-line reason, provenance named).
4. **License allowlist additions are per-license with justification.** The
   three rejected licenses fall into two categories:
   - `0BSD` and `NCSA` are OSI-approved permissive licenses (cargo-deny's
     own report labels both "OSI approved", NCSA additionally "FSF
     Free/Libre"). Add them to `[licenses].allow` with a brief inline
     comment naming an example crate and the OSI-approved status.
   - `Apache-2.0 WITH LLVM-exception` is an SPDX exception expression, not
     a bare license, and is standard across the Rust/LLVM ecosystem
     (`ar_archive_writer`, `stacker`, `psm`). Use cargo-deny's
     `[[licenses.exceptions]]` clause scoped to the specific crate(s) that
     carry the expression, not a blanket allow.
5. **Escape hatch.** If, during step 3, the developer finds an advisory
   that neither a bump nor a spec-allowed justified ignore can address
   (e.g. a live vulnerability in a crate the workspace directly depends on
   with no upstream fix), stop and raise a `Needs Clarification` entry in
   this file's `## Notes` section rather than adding an unsafe suppression.
   That path is preferable to landing a knowingly-blind gate.

**Explicit non-goals (deferred per spec 06's Out-of-scope section):**

- `.cargo/config.toml` `[env]` block for `RUST_MIN_STACK` (medium 0006 §8).
- Pinning Actions to full SHA digests (medium 0006 §7); the new
  `cargo-deny` install action follows the existing `@vN` pattern used by
  peer Actions in the file — this is spec 06's explicit instruction.
- `deny.toml` **bans-policy** tightening (`multiple-versions`, `wildcards`
  — medium 0006 §6). This slice edits `deny.toml`'s `[advisories]` and
  `[licenses]` sections only, as required to make the H1 gate pass. The
  `[bans]` and `[sources]` sections remain unchanged.
- macOS `.app` signing / notarization (medium 0006 §5).
- `macos/Info.plist` dynamic version (low 0006 §11); `compute-tag` derives the
  workspace version for the release tag only, not for the `.app` bundle
  metadata.
- Broader trigger-path redesign beyond the two additions above (partial
  remediation of medium 0006 §9 — the rest is deferred).
- Dependency upgrades that require workspace `Cargo.toml` version-constraint
  edits (as opposed to `cargo update -p` bumps within existing constraints).
  Any advisory that requires the former is out of scope and must be handled
  by the escape hatch above.
- All non-Slice-A highs (0002-H1..H4, 0003-H1..H2, 0004-H1..H2, 0005-H1..H5) —
  those are Slices B–G.
- New Rust code, new tests, or any change under `crates/`.

## Steps

Each step is a concrete edit to a specific file. Step numbers are landing
order within the slice; step 3a is the mandatory triage that must precede
adding the deny gate to CI (step 3b), because a red gate cannot be added to
CI without first making it green. Steps 1–2, 3b, 4 are the ci.yml pass, step
5 is release.yml, step 6 is documentation cross-check.

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

Rationale: without this the deny step introduced in step 3b will not run on
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

### 3a. `deny.toml` + `Cargo.lock` — triage current deny failures (0006-H1 prerequisite)

Before the CI workflow gains a `cargo deny check` step (3b), the local
`cargo deny check` must pass on the checked-in `Cargo.lock`. It does not
today. This step performs the triage. Rules from the "Deny-gate triage
policy" section above apply verbatim; the enumeration below is the concrete
work.

**3a-i. Vulnerability bumps (`cargo update -p`).** For each vulnerable crate
in the current lockfile, run a targeted update and re-check. Order chosen so
that upstream fixes propagate before downstream:

- `cargo update -p aws-lc-sys` — resolves RUSTSEC-2026-0044 and
  RUSTSEC-2026-0048 if a fixed patch version is reachable within the
  existing rustls / aws-lc-rs constraints.
- `cargo update -p rustls-webpki` — resolves RUSTSEC-2026-0049,
  RUSTSEC-2026-0098, RUSTSEC-2026-0099, and RUSTSEC-2026-0104 for the
  `0.103.x` line if reachable. Attempt for both duplicate entries
  (`0.101.7` and `0.103.9`); `0.101.7` may be pinned by an older kube-rs
  transitive that owns the constraint — if the update leaves `0.101.7` in
  place, its remaining advisories move to the justified-ignore list
  (3a-iii) with the pin cited.
- `cargo update -p h2` — resolves RUSTSEC-2026-0258 for both duplicate
  entries (`0.3.27` and `0.4.13`) where the enclosing HTTP-client stack
  admits it; if a duplicate persists, ignore with reason naming the pinning
  parent.
- `cargo update -p quick-xml` — resolves RUSTSEC-2026-0194 and
  RUSTSEC-2026-0195.
- `cargo update -p crossbeam-epoch` — resolves RUSTSEC-2026-0204.

After each `cargo update -p`, run `cargo deny check advisories` and record
which advisories cleared and which persist. Any that persist go to 3a-iii.

**3a-ii. License allowlist / exceptions (`deny.toml` edits).** Edit
`deny.toml`'s `[licenses]` section to add:

```toml
[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
    "OpenSSL",
    "BSL-1.0",
    "CC0-1.0",
    "MPL-2.0",
    # OSI-approved permissive licenses encountered via transitive deps
    # (0BSD via enum-iterator/enum-iterator-derive → gpui-component;
    #  NCSA via libfuzzer-sys → rav1e → ravif → image → gpui).
    # Both are cargo-deny-reported "OSI approved"; NCSA is additionally
    # "FSF Free/Libre" per cargo-deny's own labelling.
    "0BSD",
    "NCSA",
]

# LLVM's standard Apache-2.0 exception. Scope to the specific crate carrying
# the SPDX-expression form so a future crate importing it under different
# terms would fail the license check.
[[licenses.exceptions]]
name = "ar_archive_writer"
allow = ["Apache-2.0 WITH LLVM-exception"]
```

The `Unicode-DFS-2016` entry in the existing allow list is retained even
though cargo-deny warns it is unmatched; a future dep may reintroduce it and
removing it would be a churn edit outside this slice's scope. The
`license-not-encountered` diagnostic remains a warning, not an error, and
does not block the gate.

**3a-iii. Justified `[advisories].ignore` additions.** For each advisory
that survives 3a-i because the vulnerability sits in a crate pinned by a
transitive constraint the slice does not own, and for each `[unmaintained]`
advisory (async-std, core2, paste, proc-macro-error2, rustybuzz, ttf-parser
— none of which have alternate-registry fixes in reach of a `cargo update
-p`), add a per-ID entry following the shape of the three existing entries
in `deny.toml`:

```toml
[advisories]
ignore = [
    # existing three entries retained verbatim:
    { id = "RUSTSEC-2025-0012", reason = "backoff is a transitive dep of kube-runtime, no direct usage" },
    { id = "RUSTSEC-2024-0384", reason = "instant is a transitive dep of backoff via kube-runtime" },
    { id = "RUSTSEC-2025-0134", reason = "rustls-pemfile is a transitive dep of kube-client" },

    # New entries — one per surviving advisory. Each MUST include:
    #   (a) provenance (what depends on it),
    #   (b) why no in-slice fix (which parent pins the version), and
    #   (c) the removal trigger (which upstream bump will retire it).
    # Example shape (developer fills in verified details from step 3a-i output):
    # { id = "RUSTSEC-2025-0052", reason = "async-std: transitive via zed-async-tar → gpui_http_client → gpui; awaits gpui removing zed-async-tar dep. Remove when gpui switches its http-client backend." },
    # ...one entry per advisory that 3a-i could not eliminate...
]
```

The developer executing this slice fills in the actual entries from the
output of the 3a-i re-check. Every entry must satisfy the triage policy's
rules 1–3. Live vulnerabilities not resolved by 3a-i **must not** be
ignored unless rule 2 is satisfied (bump attempted, upstream constraint
identified, expiry trigger named); if any advisory fails that test, invoke
the escape hatch (policy rule 5) and stop.

**3a-iv. Verify.** Run `cargo deny check` locally with no arguments. It must
exit zero. Retain the resulting `Cargo.lock` diff and `deny.toml` diff as
part of the slice's commits.

### 3b. `.github/workflows/ci.yml` — add `cargo deny check` step (0006-H1)

Only after 3a exits zero locally, insert two steps into `ci.yml`. Between
`Install Rust` and `Install cargo-nextest`:

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
existing `deny.toml` as updated by 3a. It runs against the checked-in
`Cargo.lock` and does not require a build; it is fast and can run before
the heavy clippy/test steps. Follow the `@vN`-style pin used by the
existing `taiki-e/install-action@nextest` (SHA pinning is medium 0006 §7
and out of scope).

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

Verify what this slice makes stale in the tree and flag the appropriate
follow-ups. Spec 03 (`.docs/spec/03-toolchain-and-gate.md:26-28`) currently
prose-describes CI as "`macos-14`, PRs to main touching crates/ or Cargo
manifests" running "lint and tests" — a description this slice makes stale
on three counts:

1. It now runs on a 3-OS matrix (macOS 14, Ubuntu latest, Windows latest).
2. The gate ordering is now `format → deny → lint → test`, not just
   `lint → test`.
3. The trigger paths now include `deny.toml` and `.github/workflows/**`.

Spec 03 is Approved. **A slice-plan cannot edit an Approved spec** (loom
planner-role rule: "An approved spec is frozen … changes only by a new
planning cycle"). This slice therefore does **not** edit spec 03. Instead,
the slice's finalize pass — the same finalize step the planner role runs on
approval, per the role contract — must:

- Update `.docs/status/handoff.md` / `.docs/status/roadmap.md` to record
  that spec 03:26-28's CI description is now stale relative to the merged
  workflow and queues a spec 03 revision cycle.
- Not attempt to edit spec 03 from this slice-plan; if the finalize pass
  discovers the stale prose materially misleads a subsequent slice's
  planner, escalate as a Needs Clarification cycle rather than in-slice.

`CLAUDE.md` describes `cargo clippy --workspace -- -D warnings` and
`cargo test --workspace` as the local gate. **No change needed** — the
slice adds no new required local step; `cargo deny check` is a
recommended local check but not the required local gate (the required
gate is what CI enforces; the CLAUDE.md prose is the developer-side
quickstart and does not need to enumerate every CI step).

`README.md` (if present at repo root) — no CI-shape claims to update.

If step 6 discovers that spec 03 or another approved artifact must be
edited to make an in-slice acceptance criterion checkable, that is a spec
revision — stop and raise `Needs Clarification` rather than editing spec 03
from a slice-plan.

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
2. **Format gate is green** — `cargo fmt --all -- --check`. If this
   fails locally today, add `cargo fmt --all` as a same-slice commit
   (spec 06, 0006-H2 acceptance: "if it does not, add a fmt commit within
   the same slice").
3. **Deny gate is green** — `cargo deny check` locally with cargo-deny
   installed (`cargo install cargo-deny --locked` or `brew install
   cargo-deny`). This is the acceptance test for step 3a: after 3a
   completes, `cargo deny check` **must** exit zero against the current
   `Cargo.lock` and the updated `deny.toml`. It is a slice-blocking
   failure — not a hypothetical — if it does not, because step 3b (adding
   the check to CI) is invalid without a green local gate. Recovery paths
   are enumerated in the "Deny-gate triage policy" section above; the
   escape hatch (Needs Clarification) applies when neither a bump nor a
   spec-conformant ignore can resolve a surviving advisory.
4. **Existing spec 03 gate** — `cargo fmt --check`,
   `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets --
   -D warnings`, `RUST_MIN_STACK=268435456 cargo test --workspace`. These
   must pass unchanged on macOS; they are the invariant floor spec 06
   restates ("CI gate stays green"). `--all-targets` is used locally to
   also cover test/bench/example targets that the current CI `clippy` step
   does not; the CI step (unchanged by this slice) omits `--all-targets`,
   and that omission is a known scope gap tracked outside Slice A.

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
- **A4 (0006-H4 acceptance)** — This slice's merge touches only
  `.github/workflows/**`, and `release.yml`'s `on.push.paths` filter is
  `crates/**`, `Cargo.toml`, `Cargo.lock` (broader trigger-path redesign is
  the deferred remainder of medium 0006 §9). The merge therefore does
  **not** trigger a `release.yml` run on its own; A4 is a two-part
  verification:

  a. **Primary (pre-merge dry run, per spec 06 H4 test expectations)** —
  spec 06 explicitly calls for "a dry-run push to a throwaway branch" as
  the validation route. Push the slice branch (or a temporary
  branch-with-release-trigger variant of it) to a personal fork whose
  `release.yml` triggers on that branch (temporary trigger edit reverted
  before opening the upstream PR), or use `act` locally to smoke-test the
  `compute-tag` job. The dry run must show: (i) a `compute-tag` job whose
  outputs three downstream jobs consume, (ii) a single GitHub Release
  created with all three artifacts under the same tag, (iii) that tag
  begins with `v<workspace-version>-dev.` where `<workspace-version>`
  matches `baeus-app`'s version in `Cargo.toml`, (iv) `grep -R
  'v0.1.0-dev' .github/` returns no results.

  b. **Secondary (post-merge, on the next code-touching push)** — after
  the slice lands on `main`, the *first subsequent* push touching
  `crates/**` or `Cargo.toml` or `Cargo.lock` will fire `release.yml`
  under the new shape; that run must again show properties (i)–(iv). This
  is confirmatory, not a blocker for slice merge.

  Note that the deny-triage step 3a may itself produce a `Cargo.lock`
  bump, which does fall within `release.yml`'s `on.push.paths`. If the
  slice lands with a lockfile change, the merge itself triggers
  `release.yml` and provides an immediate secondary verification without
  waiting for a subsequent push.

### Gate (spec 03 + slice-specific)

Every slice must clear spec 03's format → lint → test gate. This slice adds
one slice-specific gate:

| Step | Command | Where |
|------|---------|-------|
| format | `cargo fmt --all -- --check` | local + CI (new in this slice) |
| lint (CI) | `RUST_MIN_STACK=268435456 cargo clippy --workspace -- -D warnings` | CI (unchanged by this slice) |
| lint (local) | `RUST_MIN_STACK=268435456 cargo clippy --workspace --all-targets -- -D warnings` | local dev quickstart (`--all-targets` is local-only; the CI omission is a known scope gap tracked outside Slice A) |
| test | `RUST_MIN_STACK=268435456 cargo test --workspace` (CI uses `cargo nextest run --workspace`) | local + CI |
| deny | `cargo deny check` | local + CI (new in this slice; requires step 3a triage first) |
| yaml | `actionlint .github/workflows/*.yml` | local (pre-push) |

No new automated Rust tests are added by this slice. Spec 06's invariant
"no decrease in test count across any slice" holds because the slice adds
no `#[test]` code and removes none; the workspace test count is unchanged.
Spec 06 explicitly permits this exception ("carries at least one new
automated test unless the fix is a pure workflow / documentation change
(0006 slice)").

## Acceptance criteria (consolidated from spec 06's Slice A findings, plus the H1-intrinsic triage criterion)

Verbatim from spec 06 with rewording only for consolidation; criterion 1c
below is added to bind the triage step 3a to a checkable outcome (the
"CI gate stays green" invariant + H1's acceptance require it).

1. **0006-H1**
   a. The `on.pull_request.paths:` filter includes `crates/**`, `Cargo.toml`,
   `Cargo.lock`, `deny.toml`, and `.github/workflows/**`, and every PR that
   touches any of those paths triggers a `cargo deny check` run.
   b. A new RUSTSEC advisory that hits the workspace's transitive deps
   causes CI to fail (verified by running `cargo deny check` locally
   against the current `Cargo.lock`).
   c. **(Triage acceptance)** `cargo deny check` exits zero locally
   against the checked-in `Cargo.lock` and `deny.toml` at the tip of the
   slice branch. Every entry added to `[advisories].ignore` and every
   license allowlist / exception addition in `deny.toml` complies with
   the Deny-gate triage policy (rules 1–4). No live vulnerability
   suppression violates rule 2. If the escape hatch (rule 5) was
   invoked, this criterion is failed and the slice is not mergeable
   as-is.
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

Slice A is Complete when 1a/b/c, 2a/b, 3a/b, 4a/b/c are all observably true
in CI/release runs against `main` at the tip of the merged slice (with A4
verified per its two-part scheme in the Remote verification section).

## Notes

_None._ If a role has a clarifying question, add a dated entry here and
set the artifact status to `Needs Clarification` per the loom role
contract.

---

## Landed receipt

- **Landed:** 2026-08-26 via PR #5 (merge commit `5e903819986c7734a707e24eda916a3791248cea` on `main`, remote-verified).
- **Post-publish triage** (same branch, merged in the same PR): RUSTSEC-2026-0190 (anyhow) bumped to 1.0.104; 7 clippy errors from CI's newer stable toolchain (1.94 → 1.98) fixed for CI parity.
- **Follow-up queued by owner:** pin the CI toolchain version to kill the toolchain-drift class permanently — routed to the planner's next pass as a spec-06 amendment candidate.
