# Loom Review Core

Use this reference when reviewing a completed Loom phase run. A review is a delivery gate: it decides whether the accepted phase contract is actually satisfied, whether the implementation is safe to keep, and which repair owner should handle any defect.

## Review Posture

- Treat task results as claims, not proof. Verify each important claim against the changed files, selected evidence, review matrices, and allowed references.
- Review the current phase contract first. A polished implementation that misses accepted scope is still a failed delivery.
- Separate product defects from process defects so the current review contract can select the right repair owner.
- Do not modify project files during review. The review output names findings and the next action; repairs happen in their own requests.
- Prefer one precise blocking finding over several vague findings that point to the same root cause.

## Review Order

1. Confirm the phase run scope: task ids, group ids, acceptance refs, run status, and next-phase handoff.
2. Read compact task and result summaries to understand what was claimed.
3. Read change context and diff refs or current-file context for files touched by the task results.
4. Check review matrices and review signals before making a decision.
5. Apply spec compliance before judging code quality.
6. Apply implementation quality only to code that belongs in the accepted current phase.
7. Judge verification evidence last: it can support approval, but it cannot override missing behavior or known defects.

## Decision Discipline

- Approve only when accepted scope is satisfied, review signals are satisfied, and verification evidence is credible for the changed behavior.
- Approve with notes only for non-blocking limitations that do not change current-phase correctness or safety.
- Request changes when a repairable product or quality defect exists and the repair owner is clear.
- Block completion when review cannot be completed because evidence or environment access is unavailable and no smaller automatic repair is justified.
- Escalate to the user only when the accepted contract is ambiguous or conflicts with a user-owned decision.

## Repair Ownership

Identify the owner of the defect before choosing the final review action. The exact action value and priority come from the current review output contract, not from this reference.

| Defect Shape | Owner To Consider |
|---|---|
| Implemented behavior violates accepted requirements | The task implementation or verification owner. |
| Required work was never planned or assigned | The task plan owner. |
| Architecture facts needed for this phase are missing or wrong | The architecture artifact owner. |
| User intent is ambiguous or contradictory | The user decision owner. |
| Review cannot inspect required evidence due to environment limits | A manual review or environment-resolution owner. |
| Phase is complete | The current review contract decides whether delivery continues or closes. |

## Must Not

- Do not approve by trusting the task result narrative alone.
- Do not require speculative future-phase work in the current review.
- Do not push a product defect to human review when an implementation repair can fix it.
- Do not convert style preferences into blocking findings when formatter, linter, or existing project convention already decides the style.
- Do not paste reference guidance into the review output. Convert it into concrete findings, limitations, coverage assessments, or next actions.
