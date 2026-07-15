# Loom Architecture Core

Use this file when Loom architecture work needs to turn confirmed scope, technical baseline, and repository context into implementation-facing decisions.

Loom architecture work is not a standalone design essay. It is an implementation-facing design contract whose ownership, behavior, data, runtime, and failure boundaries must remain observable in the delivered system.

## Architecture Judgment

1. Start from confirmed current-phase behavior, constraints, and existing repository boundaries.
2. Select the smallest structure that preserves ownership, invariants, failure recovery, and runtime closure.
3. Make every boundary observable in code ownership, an interface, a state transition, a data rule, or a runtime surface.
4. Treat future extensibility as a consequence of a current decision, not as permission to implement speculative layers.

## Required Architecture Assets

Production-grade Loom architecture output must produce implementation-facing assets:

| Asset | Purpose | Engineering Effect |
|---|---|---|
| Architecture style | Explains the selected structural approach for the current phase. | Establishes the composition and dependency rules implementation must preserve. |
| Module boundary | Defines responsibilities and ownership for code changes. | Keeps behavior, state, and dependencies inside explicit owners. |
| Data architecture | Defines ownership, invariants, transaction boundaries, and migration impact for the already selected stack. | Governs persistence mappings, writes, reads, and schema evolution. |
| Behavior model | Defines workflows, state transitions, blocking paths, and success outcomes. | Makes success, rejection, failure, and state effects implementable. |
| Runtime boundary | Defines build/start/probe/environment expectations. | Makes runtime entry points and dependency behavior explicit. |
| ADR decision | Captures context, decision, alternatives, consequences, and verification hints. | Preserves the selected trade-off and its observable consequences. |
| NFR target | Captures concrete quality targets and verification strategy. | Turns quality claims into measurable implementation obligations. |
| Risk/failure mode | Captures impact, mitigation, owner artifacts, and verification hints. | Connects failure exposure to an owned mitigation and evidence signal. |

## Decision Discipline

- A decision must change implementation ownership, behavior, data consistency, runtime shape, security, operability, or verification.
- Name the forces that distinguish the selected structure from realistic alternatives.
- Define where the rule is enforced and what observable evidence proves it.
- Keep related decisions coherent: a service split without data ownership, failure behavior, and runtime independence is not a complete decision.
- Do not create an ADR merely to restate a framework or database already selected by the technical baseline.

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
- Do not produce generic "scalable, maintainable, secure" claims without an observable target and evaluation boundary.
- Do not write future phase capabilities as current-phase modules.
- Do not create abstraction layers unless they support current behavior, verification, or isolation.
- Do not leave decisions, NFRs, or risks empty just because the phase looks small.

## Minimum Quality Bar

A usable Architecture section lets an implementation reader answer:

- Which module or boundary owns the implementation?
- Which data invariants must the implementation preserve?
- Which interface or workflow proves the behavior?
- Which architecture decision or risk governs that owner?
- Which verification evidence proves the architecture constraint is respected?

If those answers are missing, the architecture is incomplete; do not displace the ambiguity into implementation.
