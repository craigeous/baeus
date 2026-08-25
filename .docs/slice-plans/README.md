# slice-plans/

Executable plans that break an approved spec into landable slices. Each
slice-plan is the contract the developer implements and the code evaluator
reviews against.

- **Authored by:** the planner role, derived from an **Approved** spec.
- **Reviewed by:** the plan evaluator (blind) before approval.
- **Lifecycle:** Draft → Plan Review → Approved → Implemented →
  (code evaluation) → Ready to Publish → landed, then moved to `archive/`.

**Conflict rule:** `spec/` wins over `slice-plans/`. A slice-plan may never
contradict its parent spec; if reality forces a deviation, the spec changes
first (through a new planning cycle).

Active slice-plans live here; landed ones move to
[`archive/`](archive/README.md).

## Active

- [`slice-a-ci-hardening.md`](slice-a-ci-hardening.md) — Status: Plan Review
  — CI & release hygiene (spec 06 findings 0006-H1, H2, H3, H4).
