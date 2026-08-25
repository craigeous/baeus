# Roadmap

Milestone order — what's next and why. Owned by the project owner; loom
records, the owner decides.

1. **Full project review** (owner-requested, 2026-08-25): a deep,
   end-to-end review of the codebase in its current state — "it's ok, but it
   needs to be better." Run as a loom research pass: parallel readers over
   the subsystems, findings into `.docs/research/`, then ADRs/specs for the
   improvements the owner approves. This is the immediate next scope for
   `/loom:run`.
2. **Plan Review of back-filled specs**: the five Draft specs in
   `.docs/spec/` (01–05) need blind plan-evaluator review to reach Approved.
3. **Plugin system completion**: `baeus-plugins` is scaffolded (trait,
   loader, registry, sandbox) but not wired into the app as a user-facing
   capability (FR-029/030).
4. **Success-criteria re-measurement**: validate SC-001..SC-015 (startup,
   search, list scale, log latency) against the current build.

Candidate opportunities noted during the alignment survey (for the owner to
prioritize, none decided):

- `app_shell.rs` remains very large (4 200+ lines); the
  `pod_detail_render.rs` extraction pattern exists but is applied only
  partially.
- The speckit source repo (`~/git/baeus-spec`) is now historical; decide
  whether to archive it or keep it as read-only reference.
- Windows/Linux support is precluded by Metal (ADR 0001) — revisit only via
  a new ADR if cross-platform becomes a goal.
