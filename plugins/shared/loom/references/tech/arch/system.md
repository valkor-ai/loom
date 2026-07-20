# System Design For Loom Architecture

Use this reference when defining architecture foundation, domain/interface contracts, behavior, or runtime delivery sections.

## System Boundary

Every Architecture artifact should make the current phase boundary explicit:

| Boundary | Required Description |
|---|---|
| Product boundary | What user or operator capability is delivered now. |
| Module boundary | Which modules own which responsibilities. |
| Data boundary | Which module owns each entity and invariant. |
| Interface boundary | Which APIs, service methods, jobs, or adapters are current-phase contracts. |
| Runtime boundary | Which build/start/probe/environment facts the implementation must preserve. |
| Deferred boundary | Which tempting future capabilities are deliberately not current tasks. |

## Component Shape

For each current-phase module, capture:

- `moduleId`
- responsibility
- owned entities or state
- exposed interfaces
- accepted input/output contracts
- dependencies
- acceptance refs and requirement detail refs
- risks or decisions that apply

Module descriptions should be implementation-facing. A reader should know where code belongs and what must not be mixed together.

## Context And Trust Boundaries

Identify every current-phase caller, operator, external system, and durable store that crosses the product boundary. For each crossing, define:

- who initiates it and which component owns the receiving boundary
- whether communication is synchronous, asynchronous, scheduled, or file-based
- authentication, authorization, or data-sensitivity implications when applicable
- accepted input/output ownership and failure visibility
- whether the dependency is required for startup, required for one capability, or optional

Do not draw an external box without defining the interaction contract and trust assumptions that make it relevant.

## Interaction Design

For user or system flows:

- Define trigger, actor/system, happy path, validation/blocking path, failure path, and observable result.
- Connect each step to interface refs and state machine refs when available.
- Include state or persistence changes for actions that mutate data.
- Define success and business-blocking feedback for user-visible operations.
- For external dependencies, define timeout ownership, retry or no-retry rationale, duplicate protection, fallback/degraded behavior, and the observable signal when recovery is needed.
- For asynchronous interactions, define delivery guarantee, ordering boundary, idempotency owner, replay behavior, and how completion or terminal failure becomes visible.

## Capacity And Growth Triggers

Describe capacity only where a current requirement or a bounded product-quality minimum makes it relevant:

- workload or data condition that changes the architecture behavior
- bounded query, batch, queue, payload, or concurrency rule
- scale unit, such as application instance, worker, partition, or read model
- trigger that would justify a later structural change

Do not invent traffic numbers, multi-region topology, or horizontal scaling machinery without an accepted target. “Scalable” is not a design until the workload, boundary, and trigger are stated.

## Runtime Design

Runtime design is a code-level contract, not deployment success.

Include:

- build command and working directory when known
- start command or runtime entry when known
- runtime surfaces and probe paths
- environment variables required by the current phase
- generated artifacts that must be preserved
- constraints that later deploy should consume

Do not require Docker, registry access, cloud deployment, or clean install during Architecture.

## Failure Design

Each stateful flow should identify:

- validation failure
- dependency failure
- persistence failure
- duplicate or replayed request
- partial write or inconsistent state risk
- recovery or user-visible blocking response

When a failure affects implementation or verification, create an architecture risk record.

## Anti-Patterns

- Component diagrams without ownership.
- Generic "frontend/backend/database" boxes with no module responsibility.
- Runtime surfaces that list `/api` as a probe when it is only a prefix, not a real endpoint.
- Ignoring failure paths until Review.
- Treating deferred phase components as current dependencies.

## Example Direction

For a CRUD-like internal product, a strong system design usually defines:

- app shell or worker/admin surface
- API boundary for each business operation
- domain service or module owning validation
- persistence model and migration ownership
- list/detail/readback paths
- failure feedback for duplicate, invalid state, permission, and storage errors
