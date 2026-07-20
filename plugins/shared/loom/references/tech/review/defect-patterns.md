# Cross-Stack Defect Patterns

Use this reference after spec compliance to inspect implementation risks that recur across languages and frameworks. Apply only patterns relevant to changed files and selected technology guidance.

## Functional Correctness

- Boundary values: empty/missing/null, zero/negative, max/min, overflow, duplicate, unknown ID, invalid enum, stale version, malformed encoding, timezone/day boundary.
- Branch completeness: success plus validation, forbidden, conflict, not-found, unavailable, cancellation, timeout, partial result, and retry where behavior differs.
- Identity: commands, rows, cache keys, route params, updates, and deletes act on the displayed/requested stable target rather than index or mutable selection.
- Ordering: sorting, pagination, cursor continuation, deduplication, tie breakers, locale/collation, and date ordering remain deterministic.
- State transitions: illegal moves blocked, terminal states preserved, repeated/reopen/retry behavior defined, and history/audit corresponds to committed state.

Trace invariants across every write path, not only the primary endpoint or screen.

## Concurrency And Repetition

- Read-check-write races allow duplicate or invalid transitions without transaction/constraint/version checks.
- Retries, double clicks, callbacks, message redelivery, or process restart repeat durable/external effects.
- Old async responses overwrite newer route/filter/account state.
- Locks cover too much, deadlock in inconsistent order, or fail to protect process/distributed boundaries.
- Optimistic updates lack operation identity, rollback, conflict handling, or server reconciliation.

Check idempotency scope/key storage/expiry and whether failures before/after side effects produce a safe retry.

## Data And Persistence

- Constraints/defaults/nullability/indexes/foreign keys do not match domain and migration behavior.
- Multi-write invariants cross a transaction boundary or external effect without reconciliation/outbox/compensation.
- Migration fails on existing data, is not reversible/cutover-safe as required, or runtime starts before schema compatibility.
- N+1 queries, unbounded reads, offset/cursor bugs, missing deterministic order, or filtering after loading all data.
- ORM lifecycle/cascade/lazy behavior causes unexpected deletion, serialization, connection use, or startup validation failure.

Check provider/dialect semantics through the selected persistence references rather than generic SQL assumptions.

## Interfaces And Integration

- Method/path/body/query/status/error/auth/exposure differs between producer, consumer, tests, and runtime routing.
- Validation occurs after side effects or accepts fields/statuses the contract rejects.
- Raw provider exceptions/messages leak while actionable errors collapse into one generic failure.
- External calls lack timeout/cancellation, retry unsafe operations, or retry without jitter/budget/classification.
- Webhook/event consumers do not authenticate, deduplicate, validate version, or handle out-of-order delivery.

Inspect serialization of dates, decimals, large integers, nullable/optional fields, enums, and unknown fields.

## Security And Privacy

- Authentication/authorization/ownership/tenant checks exist only in UI or one endpoint path.
- User input reaches SQL, shell, path, template/HTML, redirect, URL fetch, deserialization, regex, or logging without appropriate validation/encoding/parameterization.
- Secrets or sensitive records enter client bundles, logs, errors, test fixtures, version control, telemetry, caches, or broad DTOs.
- CSRF/CORS/cookie/token/session configuration trusts unsafe origins, forwarding headers, algorithms, issuers, audiences, or redirect targets.
- Mass assignment/object binding lets callers control identity, owner, role, status, price, audit, or version fields.

Review deny paths and cross-user/tenant reads as carefully as privileged writes.

## Reliability And Resource Lifecycle

- Errors are swallowed, logged and ignored, transformed twice, or retried indefinitely.
- Files, sockets, streams, DB connections, transactions, workers, goroutines/tasks/threads, listeners, timers, observers, or temp resources leak on cancellation/error.
- Startup/config failure appears late at request time or silently uses a local/insecure production fallback.
- Health/readiness reports healthy before migrations/dependencies/routes are usable or couples liveness to transient downstream failure.
- Partial external/database operations have no cleanup, reconciliation, or observable recovery state.

## Performance And Capacity

- Work or payload is unbounded by pagination, size, rate, timeout, memory, queue depth, concurrency, or cache policy.
- Hot paths allocate/copy/serialize repeatedly, block event loops/executors, or perform synchronous I/O unexpectedly.
- Cache key omits identity/input/version, invalidation is incomplete, TTL contradicts correctness, or stampede behavior is uncontrolled.
- Frontend changes cause broad subscriptions/rerenders, large eager bundles/assets, leaked resources, or inaccessible virtualization.
- Batch processing loads all records, commits per item, or cannot resume safely.

Require measurement or plausible workload impact before blocking on optimization.

## Maintainability And Architecture

- Domain/application rules duplicated across API, UI, persistence, jobs, or integrations.
- Dependency direction crosses accepted ownership or generic helpers hide auth/tenant/transaction/runtime behavior.
- New abstractions have no clear complexity boundary, while one function/module combines unrelated policy and infrastructure.
- Names differ from established business concepts or misleadingly imply guarantees the code does not provide.
- Production source retains tutorial or placeholder namespaces such as `com.example`, `org.example`, `com.company`, `com.demo`, or `com.sample`; use the repository/project identity or its documented safe fallback.
- Comments repeat syntax while non-obvious invariants, ordering, compatibility, or recovery decisions remain unexplained.

## Tests And Configuration

- Tests pass only because significant collaborators/authorization/transactions are mocked away.
- Shared state/time/random/network/order leaks between tests or creates flaky timing sleeps.
- Configuration keys are defined but not consumed, consumed under different names, or use unsafe environment defaults.
- Generated artifacts/lockfiles/migrations are stale relative to source declarations.

## Unsafe Review Defaults

- Flagging every possible edge case without requirement or plausible impact.
- Recommending caching, abstractions, or retries without correctness policy.
- Applying one ORM/framework/runtime rule to another stack.
- Reporting symptoms separately when one ownership defect explains them.
- Calling maintainability preference a major defect without concrete risk.
