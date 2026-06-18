# Review Discipline

Use this reference when producing review results, manual review summaries, or repairable findings.

## Review against two axes

- Spec fidelity: missing requirements, partial behavior, wrong behavior, or scope creep against the request/contract.
- Project standards: violations of documented repo conventions, architecture choices, runtime expectations, or testing expectations.
- Keep the axes separate so a clean implementation cannot hide a wrong product outcome, and a correct feature cannot hide risky code.

## Ground every finding

- Cite the relevant file, artifact, log, command, or request field.
- Prefer concrete behavior over style opinions.
- Distinguish blockers from follow-up risks and non-blocking nits.
- If evidence is missing, say what could not be verified instead of guessing.

## Make repair actionable

- Describe the smallest repair that would satisfy the contract.
- Keep repair instructions inside the current Loom boundary and editable files.
- Do not add new scope during review repair.
- If there are no findings, state that clearly and record residual test or manual-review risk.
