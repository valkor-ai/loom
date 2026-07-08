# Loom Review Defect Patterns

Use this reference when judging implementation quality after current-phase spec compliance is understood. These patterns are language and framework neutral; apply only the items that match the changed files and selected technology references.

## Functional Correctness

- Boundary values: empty lists, null or missing inputs, max/min numbers, duplicate records, unknown ids, stale versions, and invalid enum values.
- State transitions: illegal transitions are blocked, terminal states stay terminal, and retry/reissue/reopen flows do not corrupt old state.
- Ordering and filtering: pagination, sorting, search, and date boundaries are deterministic and match API/UI expectations.
- Idempotency: repeated submit, refresh, retry, or callback handling does not create duplicate durable effects unless duplicates are valid business behavior.
- Concurrency: shared state, optimistic locking, queue consumers, async callbacks, and background jobs do not race into inconsistent results.

## Data And Persistence

- Transactions cover all writes that must succeed or fail together.
- Database constraints enforce invariants that cannot rely only on UI validation.
- Migrations match the runtime schema and do not break existing data unexpectedly.
- Query shape is bounded for list views and batch operations; loops do not hide one query per row.
- File, object-store, and external resource writes have cleanup or reconciliation strategy when the database write fails.

## Security And Access Control

- Authentication and authorization are checked at the boundary that owns the operation, not only in the UI.
- User-controlled input is validated before use in queries, file paths, shell commands, templates, redirects, or HTML.
- Sensitive data is not logged, returned in broad DTOs, embedded in frontend bundles, or committed as config.
- Cross-tenant, role, ownership, and workflow-state restrictions are enforced in read and write paths.
- Destructive or privileged actions require the confirmation, audit, or business block expected by the accepted contract.

## Reliability And Error Handling

- Errors are converted into the repository's existing error model instead of leaking raw exceptions.
- External calls have timeout, retry, fallback, or explicit failure behavior appropriate to the current task.
- Cleanup happens for partially completed operations when the task owns resources.
- Async operations propagate cancellation and do not leave unresolved promises, goroutines, tasks, threads, subscriptions, or timers.
- Runtime configuration has safe defaults and clear environment overrides without hardcoded local machine assumptions.

## Performance And Resource Use

- Lists and reporting paths use pagination or bounded reads.
- Large payloads are streamed, chunked, or rejected according to domain needs.
- Expensive computations are cached only when invalidation and correctness are clear.
- UI state updates avoid unnecessary re-render storms or unbounded subscriptions.
- Server startup, test setup, and build scripts do not perform unrelated heavy work for every request.

## Maintainability

- New abstractions have at least two real consumers or isolate a clear complexity boundary.
- Domain rules live in the domain/service layer instead of being duplicated across UI, API, and persistence.
- Names reflect business concepts from the accepted contract and existing codebase.
- Production source should not retain tutorial or placeholder namespaces such as `com.example`, `org.example`, `com.company`, `com.demo`, or `com.sample`; these indicate generated demo code rather than a professional project namespace.
- Functions and components have one clear responsibility and can be tested at the right level.
- Comments explain non-obvious decisions, not what the code already says.
