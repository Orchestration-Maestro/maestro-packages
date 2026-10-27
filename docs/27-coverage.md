# Pipeline behaviour coverage (#27)

Exercise the engine through its public run methods against local HTTP
fixtures. Assert reports, persisted evidence, checkpoint progression and
commands received by the runner, not private state or prompt wording.

Regression tests precede fixes in signed commits. Existing correct
behaviour tests use the `baseline_` prefix. The approved scope includes
keeping target text a single shell argument during recon and deepening
only confirmed findings. No live bridge or runner is required.

Record hosted proof runs and per-module covered lines for pipeline,
phases and methodology in the pull request. Name uncovered behaviour in
follow-up issues. The proof workflow stays on `ci/27-proof-1`, not this PR.
