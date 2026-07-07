# Loom Architecture Core

Use this file when Loom architecture work needs to turn confirmed scope, technical baseline, and repository context into implementation-facing decisions.

Loom architecture work is not a standalone design essay. It is a delivery contract that must be carried from Architecture into TaskPlan, Execution, TaskResult, Review, and Repair.

## Operating Model

1. Use the confirmed requirement scope, technical baseline, and repository context as source facts.
2. Convert those source facts into implementation-facing architecture constraints.
3. Keep Architecture section artifacts compact, decision-oriented, and consumable by TaskPlan and Execution.
4. Use references to make concrete decisions rather than copying reference prose into Loom JSON artifacts.

## Required Architecture Assets

Production-grade Loom architecture output must produce implementation-facing assets:

| Asset | Purpose | Later Consumer |
|---|---|---|
| Architecture style | Explains the selected structural approach for the current phase. | TaskPlan grouping and execution boundaries. |
| Module boundary | Defines responsibilities and ownership for code changes. | Task write boundaries and review scope. |
| Data architecture | Defines ownership, invariants, transaction boundaries, and migration impact for the already selected stack. | Persistence tasks and engineering quality checks. |
| Behavior model | Defines workflows, state transitions, blocking paths, and success outcomes. | Execution and runtime/UI verification. |
| Runtime boundary | Defines build/start/probe/environment expectations. | Runtime delivery tasks and deploy. |
| ADR decision | Captures context, decision, alternatives, consequences, and verification hints. | TaskPlan architecture quality requirements. |
| NFR target | Captures concrete quality targets and verification strategy. | Task verification and Review. |
| Risk/failure mode | Captures impact, mitigation, owner artifacts, and verification hints. | Task assignment and Review routing. |

## Contract Discipline

- Architecture sections should use compact ids and references, not long prose repeated across tasks.
- Coverage must emit `architectureQuality.decisions`, `architectureQuality.nfrs`, and `architectureQuality.risks`.
- TaskPlan must reference architecture quality items by id. Do not inline full ADR, NFR, or risk records inside every task.
- TaskResult must report `architectureQualityEvidence` only when the task has `architectureQualityRequirementRefs`.
- Review must route missing architecture quality structure to architecture repair, missing task assignment to taskplan repair, and missing implementation evidence to execution repair.

## Decision Inputs

Use these inputs. Ignore unselected or unavailable context.

| Input | Use |
|---|---|
| Current phase scope | Defines what the architecture must support now. |
| Deferred/excluded scope | Defines what must not be implemented now. |
| Requirement details | Defines invariants, workflows, actors, and data behaviors. |
| Technical baseline | Defines selected runtime/framework/storage facts. Consume it; do not redo technology selection here. |
| Repository context | Defines existing code boundaries and style when available. |
| UIX contract | Defines frontend quality only for frontend surfaces; do not mix UI references into architecture references. |

## Must Not

- Do not select a database, language, or framework in Architecture when Technical Baseline already owns that decision.
- Do not produce generic "scalable, maintainable, secure" claims without a task-verifiable target.
- Do not write future phase capabilities as current-phase modules.
- Do not create abstraction layers unless they support current behavior, verification, or isolation.
- Do not leave decisions, NFRs, or risks empty just because the phase looks small.

## Minimum Quality Bar

A usable Architecture section lets a later agent answer:

- Which module or boundary should this task edit?
- Which data invariants must this task preserve?
- Which interface or workflow proves the behavior?
- Which architecture decision or risk is this task responsible for?
- Which verification evidence proves the architecture constraint is respected?

If those answers are missing, repair the Architecture artifact instead of pushing ambiguity into execution.
