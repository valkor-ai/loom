# System Design For Loom Architecture

Use this reference when defining Architecture `foundation`, `domain_contract`, `behavior`, or `runtime_delivery` sections.

## System Boundary

Every Architecture artifact should make the current phase boundary explicit:

| Boundary | Required Description |
|---|---|
| Product boundary | What user or operator capability is delivered now. |
| Module boundary | Which modules own which responsibilities. |
| Data boundary | Which module owns each entity and invariant. |
| Interface boundary | Which APIs, service methods, jobs, or adapters are current-phase contracts. |
| Runtime boundary | Which build/start/probe/environment facts must later tasks preserve. |
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

Module descriptions should be implementation-facing. A task agent should know where code belongs and what not to mix together.

## Interaction Design

For user or system flows:

- Define trigger, actor/system, happy path, validation/blocking path, failure path, and observable result.
- Connect each step to interface refs and state machine refs when available.
- Include state or persistence changes for actions that mutate data.
- Define success and business-blocking feedback for user-visible operations.

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

When a failure affects implementation or verification, create an `architectureQuality.risks[]` entry.

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
