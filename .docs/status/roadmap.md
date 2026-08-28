# Roadmap

Milestone order — what's next and why. Owned by the project owner; loom
records, the owner decides.

1. ~~**Full project review**~~ **DONE (2026-08-25)**: research notes
   `.docs/research/0002`–`0006` Approved; ~62 findings (17 high). Now at the
   **owner gate**: owner picks which improvements go to planning.
2. **Plan Review of back-filled specs**: the five Draft specs in
   `.docs/spec/` (01–05) need blind plan-evaluator review to reach Approved.
3. **Plugin system completion**: `baeus-plugins` is scaffolded (trait,
   loader, registry, sandbox) but not wired into the app as a user-facing
   capability (FR-029/030).
4. **Success-criteria re-measurement**: validate SC-001..SC-015 (startup,
   search, list scale, log latency) against the current build.

## Remediation slices (spec 06 highs — 7 slices total)

- ~~**Slice A** — CI & deny-gate hardening (0006-H1, H2, H3, H4)~~
  **DONE** (code-eval PASS, PR landing on `slice/a-ci-hardening`).
- ~~**Slice B** — watch cancellation + EKS token refresh (0002-H1, 0002-H2)~~
  **DONE** (code-eval PASS, PR landing on `slice/b-watch-eks`).
- **Slices C–G** — remaining spec 06 highs, to be planned in order.

Candidate opportunities noted during the alignment survey (for the owner to
prioritize, none decided):

- `app_shell.rs` remains very large (4 200+ lines); the
  `pod_detail_render.rs` extraction pattern exists but is applied only
  partially.
- The speckit source repo (`~/git/baeus-spec`) is now historical; decide
  whether to archive it or keep it as read-only reference.
- Windows/Linux support is precluded by Metal (ADR 0001) — revisit only via
  a new ADR if cross-platform becomes a goal.
