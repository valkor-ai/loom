# Loom Review Finding Quality

Use this reference when writing review findings. A useful finding tells the next Loom step exactly what failed, why it matters, where the evidence is, and which repair owner should fix it.

## Finding Content

Each finding should include:

- A short summary that names the defect, not a generic quality label.
- Concrete evidence from a changed file, diff ref, task result, verification result, or review matrix.
- The affected task, group, acceptance ref, or requirement detail when available.
- The user or system impact if the defect ships.
- The smallest repair owner that can satisfy the current contract.

## Severity

| Severity | Use When |
|---|---|
| critical | Security bypass, data loss, corruption, crash on primary path, or impossible current-phase delivery. |
| major | Accepted behavior missing, important negative path broken, integration failure, weak evidence for must-level scope, or serious maintainability risk. |
| minor | Non-blocking quality issue, localized maintainability issue, or small verification gap with low product risk. |
| note | Review limitation, observation, or positive/neutral evidence that should not block delivery. |

## Category Selection

- Use functional_correctness when implemented behavior is wrong or incomplete.
- Use acceptance_not_satisfied when the acceptance ref itself is not met.
- Use evidence_insufficient when behavior may be correct but the submitted evidence cannot prove it.
- Use test_gap when the missing evidence is specifically an automated or runtime check.
- Use architecture_design_gap, api_contract, code_quality, or frontend_experience when the defect belongs to that quality axis.
- Use task_scope_mismatch for current-phase scope creep or future-phase implementation.
- Use environment_or_dependency when review or execution is blocked by runtime environment rather than product code.

## Evidence Refs

- readRefs prove what the reviewer inspected.
- evidenceRefs point at task results, verification results, diff refs, changed files, or manual notes.
- A blocking finding should avoid empty readRefs and should cite a specific location when source context is available.
- Do not cite broad objects when a narrower task, file, diff, or verification ref is available.

## Actionability

Good findings answer these questions:

- What exact behavior or evidence failed?
- Which accepted contract item is affected?
- What is the smallest repair?
- Which repair owner should handle it?
- What would be enough evidence to approve after repair?

## Anti-Patterns

- "Needs more tests" without naming the unproved behavior.
- "Code is messy" without a concrete maintainability or correctness impact.
- Repeating review signals as findings without explaining the real defect.
- Bundling unrelated defects into one large finding.
- Using human review to avoid naming a clear repair owner.
