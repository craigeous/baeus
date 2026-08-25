# evaluations/

Records of blind evaluations — one file per evaluation run.

- **Plan evaluations** (plan evaluator): verdicts on research notes, ADRs,
  specs, and slice-plans at Research Review / Plan Review.
- **Code evaluations** (code evaluator): verdicts on implemented slices,
  including the re-run gate result.

Evaluators are blind: they judge an artifact against its upstream authority
and the playbook rubric, with no knowledge of who authored it. Each record
captures the artifact reviewed, the verdict (PASS / FAIL), findings, and the
resulting status transition.
