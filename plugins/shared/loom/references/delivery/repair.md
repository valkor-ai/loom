# Repair Discipline

Use this reference for bug fixes, failed checks, regressions, review repairs, and deploy-sourced application repairs.

## Stay inside loom

- Treat the current `instruction`, `TaskExecutionRequest`, repair request, and editable file list as the boundary.
- Do not change unrelated files to make the failure disappear.
- Record the repair path and verification evidence in the returned `TaskResult` or repair result.

## Build the feedback signal

Before speculative edits, establish the tightest runnable signal that can catch the reported problem when feasible:

- a focused unit, integration, or end-to-end test
- a CLI command with fixture input
- a browser or HTTP check
- a build/typecheck/lint failure window
- a captured log, request, or replayable payload

The signal should exercise the user-visible symptom or the exact failing contract. If no runnable signal is feasible, record what was tried and why the remaining work needs user/environment input.

## Diagnose before changing broadly

- Reproduce the problem at least once, or explain why reproduction is impossible.
- Minimize the failing case before making wide edits.
- Keep two to four concrete hypotheses in mind and test the most distinguishing one first.
- Change one variable at a time when diagnosing.
- Use temporary instrumentation only with a unique removable marker such as `[LOOM-DEBUG-<id>]`, then remove it before submit.

## Fix and prove

- Prefer the smallest fix that preserves the requested behavior and existing contracts.
- Add or update a regression test when there is a stable seam.
- Re-run the original failing signal after the fix, not only the new test.
- Include the failing signal, fix summary, verification command, and any residual risk in `TaskResult.evidence` or the closest matching result field.
