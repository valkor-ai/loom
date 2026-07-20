# TypeScript Implementation Pattern Quality

## When To Use

- Load when the task owns an API client, repository or gateway, factory, builder, reducer, state machine, mapper, dependency boundary, or reusable TypeScript abstraction.
- Do not introduce a pattern object or class when a direct function and a named type express the behavior clearly.
- Existing repository boundaries and dependency conventions take precedence over generic TypeScript examples.

## Decision Rules

- Use a factory or builder for validated multi-step construction, fixture assembly, or complex dependency setup; keep simple object creation simple.
- Centralize HTTP client behavior when adding calls: base URL, headers, response parsing, error normalization, cancellation, and typed mapping must not be duplicated in screens.
- Align repository or gateway interfaces with real business operations. Do not expose generic CRUD when the workflow has named commands, validation, or state transitions.
- Use a reducer or explicit state machine for multi-step UI or workflow state when combinations of booleans can become invalid.
- Map DTOs to domain and view models at one boundary. Dates, money, enum labels, IDs, and optional fields should not be reformatted independently in components.
- Use `Result` or `Either` only when the repository already uses it or the task owns error-handling architecture; otherwise follow the local throw/return convention.
- Inject clocks, UUID generators, network clients, and storage only when nondeterminism affects behavior. Do not inject every pure helper.

## Implementation Focus

- Keep abstractions testable without hiding the actual ownership boundary behind a global mutable singleton.
- Move shared contracts to a neutral module when a pattern would otherwise create upward or circular imports.
- Keep type-level API safety paired with runtime response validation; a generic client signature does not prove server data.
- Make invalid transitions and construction failures explicit rather than returning partially initialized objects.

## Failure Modes

- Do not add a builder, repository, or service class when it only forwards one function and increases indirection.
- Do not hide network errors behind a generic success type or let each caller invent a different error mapping.
- Do not introduce a singleton store or client that makes tests share mutable state across cases.

## Verification Focus

- Test the behavior provided by the pattern: state transitions, API error mapping, repository contracts, factory validation, or mapper round-trips.
- For API clients, cover success, non-2xx responses, malformed payloads where applicable, and cancellation or timeout when implemented.
- For reducers and state machines, cover every allowed transition and at least one disallowed transition.
- Run typecheck to prove call sites do not need broad assertions.

## Evidence Focus

- Record the selected pattern, the ownership boundary that required it, and the behavior used to prove it was necessary.
