# Research: Quality & delivery infrastructure review

**Status**: Research Review
**Date**: 2026-08-25
**Subsystem**: workspace config, CI/release, test estate

## Summary

The project has functional multi-platform release automation, a workspace-wide
clippy gate with `-D warnings`, and a documented deny.toml with reasoned advisory
suppressions — these are genuine assets. However, cargo-deny never actually runs in
CI despite the policy file existing, `rustfmt --check` is absent from the PR gate
entirely, and the CI check job is macOS-only meaning Linux/Windows regressions are
invisible until after merge. The release versioning is hardcoded (`v0.1.0-dev`) in
three independent tag-generation steps, creating a split-brain risk when jobs race.
Test coverage in `baeus-core` is wider than expected for a young project, but the
AWS SSO device-flow, credential-refresh, and live kube-watcher paths have no
integration-level harnesses.

## Strengths

- Clippy runs with `-D warnings` on all three platforms in the release workflow;
  PRs get macOS clippy gating.
- `deny.toml` exists with a maintained license allowlist and three RUSTSEC advisory
  suppressions, each with a documented rationale
  (`deny.toml:5-8`).
- Cargo.lock is committed — correct for a binary project; cache keys hash it
  (`ci.yml:36`, `release.yml:33`).
- `baeus-core` has inline unit tests across all 16 source files including aws_eks,
  exec, logs (78 tests), resource (129 tests), and kubeconfig (36 tests).
- cargo-nextest is used throughout for faster, more reliable test execution
  (`ci.yml:47`, `release.yml:124`).
- `RUST_MIN_STACK` is consistently set on every test/clippy step to work around the
  GPUI proc-macro stack issue.

## Findings

**1. cargo-deny never runs in CI**
Severity: high
Evidence: `deny.toml` exists with advisory, license, bans, and sources sections, but
neither `ci.yml` (48 lines total) nor `release.yml` contain any `cargo deny` step.
The three suppressed RUSTSEC advisories are maintained manually but are never
machine-verified. Any new advisory from a transitive dep will silently pass.
Why it matters: the project constitution (per the owner's framing) requires
dependency auditing every CI run; the gap is total, not partial.

**2. No `rustfmt --check` in the PR gate**
Severity: high
Evidence: `ci.yml:24` installs only `components: clippy` — `rustfmt` is not in the
component list and there is no `cargo fmt --check -- --check` step anywhere in
`ci.yml`. `rustfmt.toml` defines policy (edition=2024, max_width=100) but it is
never enforced in CI. Format drift will accumulate silently across contributors.

**3. CI PR gate is macOS-only; Linux/Windows invisible until post-merge**
Severity: high
Evidence: `ci.yml:16` specifies `runs-on: macos-14` for the single `check` job.
Linux and Windows clippy/test jobs exist only in `release.yml`, which triggers on
push to `main` (`release.yml:3-9`). A PR that compiles cleanly on macOS but fails
on `ubuntu-latest` or `windows-latest` will be merged before the failure is visible.

**4. Release version hardcoded; three independent tag generators risk split-brain**
Severity: high
Evidence: The release tag is generated as `v0.1.0-dev.${DATE}.${SHORT_SHA}` in three
separate `Generate release tag` steps (`release.yml:74-79`, `release.yml:191-196`,
`release.yml:274-279`). The string `0.1.0` is literal, not read from `Cargo.toml`.
Consequences: (a) if jobs execute on different runners at different wall-clock times
(DATE differs) they produce different tags, creating multiple GitHub Releases or
failing the `softprops/action-gh-release` upload; (b) bumping the workspace version
in `Cargo.toml` has no effect on release tags.

**5. macOS app unsigned and unnotarized**
Severity: medium
Evidence: `release.yml:93` documents the workaround as "Run `xattr -cr
/Applications/Baeus.app` (unsigned)". `build-app.sh` has no `codesign` or
`xcrun notarytool` step. `macos/` contains no `.entitlements` file. The app
communicates with AWS APIs and local kube API servers, so macOS Gatekeeper will
quarantine it on first launch for every user.

**6. `deny.toml` bans policy is warn-only; wildcards allowed**
Severity: medium
Evidence: `deny.toml:30` sets `multiple-versions = "warn"` and `deny.toml:31` sets
`wildcards = "allow"`. Even if cargo-deny ran in CI (see finding 1), duplicate
dependency versions and wildcard version constraints would not block a build. Given
that kube-rs and the AWS SDK together pull in many transitive deps, duplicate
versions accumulate unnoticed.

**7. GitHub Actions pinned to floating semver tags, not SHAs**
Severity: medium
Evidence: `ci.yml:18` uses `actions/checkout@v4`; `release.yml:82` uses
`softprops/action-gh-release@v2`; `release.yml:124` uses `taiki-e/install-action@nextest`
(no SHA). A malicious or accidental tag mutation at the upstream owner is
transparently adopted on the next CI run. Standard supply-chain hardening practice
is to pin to `@<full-commit-sha>`.

**8. `.cargo/config.toml` is comment-only — `RUST_MIN_STACK` not actually configured**
Severity: medium
Evidence: `.cargo/config.toml:1-7` is entirely comments explaining the issue. The
env var is set inline in every CI step and in `build-app.sh:25` but is NOT declared
under `[env]` in the config file. Any developer who runs `cargo test` locally
without reading the README or CLAUDE.md will hit the SIGBUS stack overflow.
A `[env]` section in `.cargo/config.toml` would eliminate this class of failure
for all contributors automatically.

**9. CI `paths:` filter excludes `.github/workflows/` changes**
Severity: medium
Evidence: `ci.yml:6-9` filters to `crates/**`, `Cargo.toml`, `Cargo.lock`. A PR
that changes only `.github/workflows/ci.yml` or `deny.toml` does not trigger CI.
Workflow regressions are never caught by CI before merge.

**10. AWS SSO device-flow and credential-refresh have no integration harness**
Severity: low
Evidence: `aws_sso.rs` (356 lines, 6 tests) tests only string-matching helper
`is_aws_sso_auth_error` and kubeconfig injection; the actual SSO device-flow
(`start_device_authorization`, `create_token` polling, `get_role_credentials`) has
no tests. `aws_eks.rs` (1,116 lines, 15 tests) covers serialization; the
`connect_eks_cluster`, `refresh_eks_credentials`, and `auto_reconnect` code paths
have no unit or integration tests.

**11. `macos/Info.plist` version is static**
Severity: low
Evidence: `macos/Info.plist:12-16` hardcodes both `CFBundleVersion` and
`CFBundleShortVersionString` as `0.1.0`. Neither the build script (`build-app.sh`)
nor the CI workflow reads from `Cargo.toml` to populate these at bundle time. The
installed app will always report 0.1.0 regardless of workspace version.

**12. `.sdlc/` is untracked; `CLAUDE.md` is gitignored**
Severity: low
Evidence: `git status` shows `?? .sdlc/` — the directory contains `adrs/`, `design/`,
`explorations/`, `features/`, `INDEX.md` but is not committed to the repo.
`.gitignore:35` intentionally excludes `CLAUDE.md`. Project instructions and design
decisions do not travel with the repository clone, creating an onboarding gap for
new contributors.

## Candidate opportunities

- Add a `cargo deny check` step to `ci.yml` (before or after clippy) to enforce the
  existing `deny.toml` policy on every PR — this closes the largest single gap with
  the least effort.
- Add `cargo fmt --check` to `ci.yml` alongside clippy; add `rustfmt` to the
  `components:` list on that step.
- Add Linux and Windows `cargo check`/`cargo clippy` jobs to `ci.yml` so all three
  platforms are gated on PRs, not just post-merge.
- Drive the release tag from `Cargo.toml` workspace version (e.g.
  `cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version'`) and
  share it as a single job output to eliminate three independent tag generators.
- Add a `[env]` section to `.cargo/config.toml` for `RUST_MIN_STACK` so all
  contributors get correct behavior without manual env var setup.
- Add `.github/workflows/` to the `paths:` filter in `ci.yml` so workflow changes
  are themselves validated.
- Pin all GitHub Actions to SHA digests and adopt Dependabot for Actions version
  management.
- Populate `CFBundleVersion` dynamically at bundle time from the Cargo.toml version
  (via sed or a build script argument) in both `build-app.sh` and the release
  workflow.
- Consider whether `.sdlc/` content should be committed (possibly in a
  `docs/` or `design/` tree) or explicitly documented as intentionally local-only.
- Evaluate integration-level harnesses for the AWS SSO credential chain using
  mocked HTTP responses (wiremock or httpmock) targeting the critical
  `start_device_authorization` and `create_token` poll loops.

## Citations

- `/Users/craig.pfeiffer/git/baeus/.github/workflows/ci.yml` — full file (48 lines)
- `/Users/craig.pfeiffer/git/baeus/.github/workflows/release.yml` — full file (286 lines)
- `/Users/craig.pfeiffer/git/baeus/deny.toml` — full file
- `/Users/craig.pfeiffer/git/baeus/rustfmt.toml` — full file
- `/Users/craig.pfeiffer/git/baeus/clippy.toml` — full file
- `/Users/craig.pfeiffer/git/baeus/.cargo/config.toml` — full file
- `/Users/craig.pfeiffer/git/baeus/Cargo.toml` — workspace manifest
- `/Users/craig.pfeiffer/git/baeus/macos/build-app.sh` — full file
- `/Users/craig.pfeiffer/git/baeus/macos/Info.plist` — full file
- `/Users/craig.pfeiffer/git/baeus/.gitignore` — full file
- `/Users/craig.pfeiffer/git/baeus/crates/baeus-core/src/aws_sso.rs` — line counts and test inspection
- `/Users/craig.pfeiffer/git/baeus/crates/baeus-core/src/aws_eks.rs` — line counts and test counts
- `/Users/craig.pfeiffer/git/baeus/crates/baeus-core/src/client.rs` — test count
- `/Users/craig.pfeiffer/git/baeus/crates/baeus-core/src/runtime.rs` — test module inspection
- `grep -c "#\[test\]" crates/baeus-core/src/*.rs` — test count survey
- `ls crates/baeus-ui/tests/` — 61-file integration test estate enumeration
- `git status` — untracked `.sdlc/` detection
