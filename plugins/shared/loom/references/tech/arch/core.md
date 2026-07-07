# Loom Architecture Core

Use this file when Loom architecture work needs to turn confirmed scope, technical baseline, and repository context into implementation-facing decisions.

Loom architecture work is not a standalone design essay. It is a delivery contract that must be carried into planning, implementation, evidence, review, and repair.

## Operating Model

1. Use the confirmed requirement scope, technical baseline, and repository context as source facts.
2. Convert those source facts into implementation-facing architecture constraints.
3. Keep Architecture section artifacts compact, decision-oriented, and consumable by planning and implementation.
4. Use references to make concrete decisions rather than copying reference prose into delivery artifacts.

## Required Architecture Assets

Production-grade Loom architecture output must produce implementation-facing assets:

| Asset | Purpose | Used By |
|---|---|---|
| Architecture style | Explains the selected structural approach for the current phase. | Task grouping and implementation boundaries. |
| Module boundary | Defines responsibilities and ownership for code changes. | Task write boundaries and review scope. |
| Data architecture | Defines ownership, invariants, transaction boundaries, and migration impact for the already selected stack. | Persistence tasks and engineering quality checks. |
| Behavior model | Defines workflows, state transitions, blocking paths, and success outcomes. | Implementation and runtime/UI verification. |
| Runtime boundary | Defines build/start/probe/environment expectations. | Runtime delivery tasks and deploy. |
| ADR decision | Captures context, decision, alternatives, consequences, and verification hints. | Planning and implementation quality requirements. |
| NFR target | Captures concrete quality targets and verification strategy. | Task verification and review. |
| Risk/failure mode | Captures impact, mitigation, owner artifacts, and verification hints. | Task assignment and review. |

## Contract Discipline

- Architecture sections should use compact ids and references, not long prose repeated across tasks.
- Coverage should include decision, NFR, and risk records with stable ids.
- Task planning should reference architecture quality items by id. Do not inline full ADR, NFR, or risk records inside every task.
- Implementation evidence should mention only the architecture quality items that task owned.
- Review should identify whether a gap belongs to architecture structure, task assignment, or implementation evidence.

## Decision Inputs

Use these inputs. Ignore unselected or unavailable context.

| Input | Use |
|---|---|
| Current phase scope | Defines what the architecture must support now. |
| Deferred/excluded scope | Defines what must not be implemented now. |
| Requirement details | Defines invariants, workflows, actors, and data behaviors. |
| Technical baseline | Defines selected runtime/framework/storage facts. Consume it; do not redo technology selection here. |
| Repository context | Defines existing code boundaries and style when available. |
| Frontend quality contract | Defines frontend quality only for frontend surfaces; do not mix UI references into architecture references. |

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

If those answers are missing, repair the architecture output instead of pushing ambiguity into implementation.
