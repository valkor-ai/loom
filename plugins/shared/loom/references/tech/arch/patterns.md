# Architecture Patterns For Loom Delivery

Use this reference when the current Architecture section must choose or justify a structural approach such as monolith, modular monolith, service split, event-driven flow, CQRS-style read/write separation, or background job processing.

## Decision Factors

Use system and business constraints only:

| Factor | What To Inspect |
|---|---|
| Business boundary clarity | Are capabilities independent or tightly coupled? |
| Data consistency | Does the phase need one transaction boundary, eventual consistency, or audit replay? |
| State complexity | Are there lifecycle states, compensations, reversals, or blocking conditions? |
| Integration pressure | Does the phase call external services, async workers, queues, or imports/exports? |
| Runtime boundary | Does the phase require separate build/start surfaces or a single deployable app? |
| Failure recovery | What breaks when a dependency fails, and what state must be preserved? |
| Current phase closure | What is the smallest structure that can be implemented, verified, and reviewed now? |

## Pattern Guide

### Single Application

Use when the current phase is one cohesive product surface or backend with shared transaction semantics.

Good fit:
- Small or medium domain slice.
- Strong consistency across current-phase entities.
- One runtime surface is enough.
- Separate services would add deployment and data consistency risk without current value.

Architecture obligations:
- Make module boundaries explicit even inside one app.
- Keep domain rules close to the owning module.
- Avoid global utility dumping grounds.
- Record future split points as consequences, not current tasks.

### Modular Monolith

Use when the product is one deployable application but has multiple domain modules with distinct responsibilities.

Good fit:
- Multiple capabilities share a runtime but need clean boundaries.
- Current phase touches domain logic, persistence, and UI/API together.
- Future phases need extension without reworking the first phase.

Architecture obligations:
- Define module ids, responsibilities, owned entities, exposed interfaces, and forbidden cross-module shortcuts.
- Keep cross-module calls explicit.
- Avoid shared mutable models that bypass module invariants.

### Service Split

Use only when the current phase has a real independent runtime boundary.

Good fit:
- Separate lifecycle or deployment is required now.
- Independent data ownership is necessary now.
- Failure isolation changes current-phase behavior.
- External integration or protocol boundary is part of the requirement.

Architecture obligations:
- Define service ownership, API contract, data ownership, retry/failure behavior, and operational cost.
- Record consistency trade-offs.
- Do not split just because later phases may become complex.

### Event-Driven Flow

Use when the current phase needs asynchronous processing, durable side effects, integration decoupling, or audit-oriented workflow.

Architecture obligations:
- Define event producers, consumers, payload ownership, idempotency, ordering assumptions, and replay behavior.
- Record what is eventually consistent and what remains synchronous.
- Plan failure visibility and retry limits.

### CQRS-Style Separation

Use when command behavior and query/read models have materially different needs in the current phase.

Architecture obligations:
- Define command source of truth, read model derivation, staleness expectation, and rebuild strategy.
- Do not introduce CQRS for ordinary CRUD screens with identical read/write needs.

### Background Worker Or Scheduled Job

Use when the current phase has work that should not run inside the request/response path, such as imports, notifications, reconciliation, report generation, cleanup, or periodic synchronization.

Architecture obligations:
- Define trigger, schedule or queue source, idempotency, retry limit, failure visibility, and ownership of any durable state.
- Define how users or operators observe job progress, partial failure, and completion.
- Keep the worker in the same deployable application unless the current phase requires a separate runtime surface.

### Serverless Function

Use only when the selected technical baseline or existing repository already uses function-style runtime, or when current requirements explicitly need event-triggered ephemeral execution.

Architecture obligations:
- Define trigger source, timeout, cold-start tolerance, state boundary, retries, idempotency, and observability.
- Keep durable state, secrets, and external calls explicit.
- Do not introduce serverless solely to avoid designing a normal module or worker.

### Hybrid Or Custom Structure

Use when current-phase boundaries genuinely combine patterns or do not fit a named pattern.

Architecture obligations:
- Name every composed pattern or give the custom structure a stable descriptive name.
- Define module, data, interaction, and runtime rules with the same precision required for known patterns.
- Explain which current-phase force prevents a simpler known pattern from fitting.
- Treat `custom` as an open classification, not as an exemption from ownership, failure, or verification rules.

## Pattern Decision Evidence

A defensible pattern decision states:

- current consistency and transaction needs
- module and data ownership boundaries
- synchronous, asynchronous, and external interaction pressure
- runtime surfaces that must start, scale, or fail independently
- recovery behavior when a boundary is unavailable
- concrete structural rules that implementation can preserve

Pattern names alone are not architecture. A “modular monolith” with shared mutable ownership or a “service split” with one shared database and no failure boundary has not established the claimed structure.

## Anti-Patterns

- Choosing microservices for prestige.
- Choosing event-driven design without a concrete async requirement.
- Using CQRS to avoid designing good queries.
- Creating a background worker when synchronous behavior is simpler and acceptable.
- Introducing serverless without an existing/runtime requirement.
- Splitting modules while sharing all database tables and domain objects without ownership.
- Writing "future scalable" as a reason without current verification impact.
