# Loom Review Spec Compliance

Use this reference before implementation quality review. The question is: did the current phase build the right behavior for the accepted Loom contract?

## Compliance Inputs

- Accepted acceptance refs and requirement detail refs.
- Task scope, task objectives, dependencies, and write boundaries.
- Review matrices for concept coverage, detail coverage, architecture quality, API contract, code quality, frontend quality, and runtime signals.
- Changed files and task result evidence.
- Deferred and excluded scope when it appears in current phase handoff or summaries.

## Missing Requirement Checks

- Every must-level acceptance ref has supporting task result evidence or an explicit limitation.
- Every required business rule has a code path, validation path, state transition, UI path, API contract, or runtime behavior that can be inspected.
- Negative paths are implemented when the requirement says an action must be blocked, rejected, frozen, audited, retried, or surfaced to the user.
- Readback behavior exists when the requirement expects state changes to be observable.
- Persistence and integration behavior are present when the requirement depends on durable state or cross-surface behavior.

## Scope Creep Checks

- New workflows, permissions, integrations, background jobs, caches, abstractions, or infrastructure are justified by current-phase requirements.
- Future-phase entities do not become current-phase behavior unless they are required as a minimal boundary or stub.
- Generated UI, API, or runtime surfaces do not expose delivery notes, technical explanations, or scaffolding that users did not ask for.
- Extra dependencies do not create a new platform direction outside the accepted technical baseline.

## Interpretation Gap Checks

- Ambiguous terms are resolved consistently with confirmed concept grounding and existing repository behavior.
- Similar existing features use the same status model, error model, ownership boundary, and naming language.
- Assumptions in implementation evidence are not silently promoted to accepted requirements.
- User-facing behavior matches the confirmed actor workflow, not only the database or API shape.

## Compliance Finding Shape

A spec compliance finding should name:

- The accepted requirement or acceptance ref.
- What the implementation actually does.
- The evidence source: changed file, task result, verification result, or review matrix signal.
- Why this is missing, extra, or misinterpreted for the current phase.
- The smallest repair owner that can fix it.

## Approval Bar

Approval requires the current phase to do the right thing before the review praises architecture, test style, naming, or implementation polish. If spec compliance fails, code quality comments can be included only when they help the same repair; they should not distract from the blocking contract miss.
