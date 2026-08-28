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

- [`slice-c-injection-errors-async-tests.md`](slice-c-injection-errors-async-tests.md) —
  Status: Plan Review — AWS credential-injection typed errors + async
  wizard tests (spec 06 findings 0002-H3, 0002-H4). Branch:
  `docs/slice-c-plan`.

## Archived

- [`archive/slice-b-watch-cancellation-eks-refresh.md`](archive/slice-b-watch-cancellation-eks-refresh.md) —
  Status: Archived — Core watch cancellation + EKS token refresh (spec 06
  findings 0002-H1, 0002-H2). Code-eval PASS; PR landing on branch
  `slice/b-watch-eks`.
- [`archive/slice-a-ci-hardening.md`](archive/slice-a-ci-hardening.md) —
  Status: Archived — CI & release hygiene (spec 06 findings 0006-H1, H2, H3,
  H4). Code-eval PASS; PR landing on branch `slice/a-ci-hardening`.
- [`archive/slice-a2-ci-toolchain-pin.md`](archive/slice-a2-ci-toolchain-pin.md) —
  Status: Archived — CI toolchain pin (owner-directed 2026-08-26, spec 06
  Amendments). Code-eval PASS; PR landing on branch `slice/a2-ci-toolchain-pin`.
