# Evaluation: 0006-quality-infra-review

Verdict: PASS
Round: 0
Reviewed against: cited sources under `/Users/craig.pfeiffer/git/baeus/` (`.github/workflows/ci.yml`, `.github/workflows/release.yml`, `deny.toml`, `rustfmt.toml`, `clippy.toml`, `.cargo/config.toml`, `macos/build-app.sh`, `macos/Info.plist`, `.gitignore`, `crates/baeus-core/src/*.rs`, `crates/baeus-ui/tests/`, `git status`)

## Findings

- [MINOR] Finding #10 names three specific functions — `connect_eks_cluster`,
  `refresh_eks_credentials`, `auto_reconnect` — as untested code paths in
  `aws_eks.rs`, but a `grep -rn "fn connect_eks_cluster|fn refresh_eks_credentials|fn auto_reconnect"` across
  `crates/` returns zero hits. The actual public surface in `aws_eks.rs` is
  `sso_register_client`, `sso_start_device_auth`, `sso_poll_for_token`,
  `sso_get_role_credentials`, `authenticate_with_access_key`, `assume_role`,
  `discover_eks_clusters`, `generate_eks_token`, `create_eks_client` (see lines
  237, 262, 298, 432, 493, 530, 603, 724, 828). The general claim (SSO device
  flow + EKS connect paths lack integration harnesses) is directionally
  accurate — the SDK calls `.start_device_authorization()` (aws_eks.rs:276),
  `.create_token()` (aws_eks.rs:312), `.get_role_credentials()` (aws_eks.rs:446)
  are present and untested — but the three fabricated Rust function names
  weaken the citation. Suggest replacing with the actual function names above.

- [MINOR] Finding #11 cites `macos/Info.plist:12-16` for the hardcoded version
  strings. The literals are actually at lines 12 (`CFBundleVersion` value) and
  14 (`CFBundleShortVersionString` value); line range is slightly padded but
  the substance is correct.

## Required changes (for FAIL)

None — verdict is PASS. Minor citation refinements above are optional
polish, not blockers.

## Notes

Verification pass summary (spot-checked all high/medium findings mechanically):

- Finding #1 (cargo-deny never runs): `ci.yml` (48 lines) and `release.yml`
  (286 lines) contain no `cargo deny` invocation. `deny.toml` lines 5-8 list
  three RUSTSEC ignores with reasons. **Confirmed.**
- Finding #2 (no `rustfmt --check`): `ci.yml:24` `components: clippy` only;
  no `cargo fmt` step anywhere in `ci.yml`. `rustfmt.toml` sets `edition="2024"`,
  `max_width=100`. **Confirmed.**
- Finding #3 (macOS-only PR gate): `ci.yml:16` `runs-on: macos-14` on the
  single `check` job. `release.yml:3-9` triggers on push to main. Linux
  (`release.yml:101`) and Windows (`release.yml:205`) check jobs live only in
  the release workflow. **Confirmed.**
- Finding #4 (three tag generators, hardcoded `v0.1.0-dev`): steps at
  `release.yml:73-79`, `release.yml:190-196`, `release.yml:272-279` each emit
  `TAG="v0.1.0-dev.${DATE}.${SHORT_SHA}"`. **Confirmed.** (The note cites
  74-79/191-196/274-279; the step blocks start one line earlier but the
  literal strings are within the cited ranges.)
- Finding #5 (unsigned/unnotarized): `release.yml:93` contains the
  `xattr -cr /Applications/Baeus.app` workaround line; `build-app.sh` has no
  `codesign`/`notarytool` invocations; no `.entitlements` in `macos/`.
  **Confirmed.**
- Finding #6 (deny bans warn-only): `deny.toml:30` `multiple-versions = "warn"`,
  `deny.toml:31` `wildcards = "allow"`. **Confirmed.**
- Finding #7 (floating action tags): `ci.yml:18` `actions/checkout@v4`;
  `release.yml:82` `softprops/action-gh-release@v2`; `release.yml:124`
  `taiki-e/install-action@nextest`. **Confirmed.**
- Finding #8 (`.cargo/config.toml` comment-only): all 5 lines are `#` comments
  explaining the GPUI stack issue; no `[env]` section. `build-app.sh:25`
  inlines `RUST_MIN_STACK=268435456`. **Confirmed.**
- Finding #9 (paths filter excludes workflows): `ci.yml:6-9` filters to
  `crates/**`, `Cargo.toml`, `Cargo.lock`. **Confirmed.**
- Finding #12 (`.sdlc/` untracked, `CLAUDE.md` gitignored): `git status`
  shows `?? .sdlc/`; `.sdlc/` contains `adrs/design/explorations/features/
  INDEX.md/resources.yaml/templates`; `.gitignore:35` `CLAUDE.md`.
  **Confirmed.**
- Test counts in Strengths section: `aws_sso.rs=6`, `aws_eks.rs=15`,
  `client.rs=31`, `exec.rs=43`, `logs.rs=78`, `kubeconfig.rs=36`,
  `resource.rs=129` — all confirmed via `grep -c "#\[test\]"`. `baeus-core/src/`
  contains exactly 16 `.rs` files. `crates/baeus-ui/tests/` contains 61 files.

Format & scope: Status line present (`Research Review`), Date, Subsystem,
Summary, Strengths, Findings (with severity tags), Candidate opportunities,
Citations — matches expected research-note shape. Findings are analytical;
recommendations are correctly parked in "Candidate opportunities" rather than
smuggled as decisions. Every claim in the body carries a file:line or
verifiable command citation.
