# C# Entity Framework Core Outside ASP.NET

## When To Use

Use this reference for EF Core persistence tasks in C# workers, services, CLIs, desktop applications, or libraries where the ASP.NET Core data reference is not already selected. Apply provider-specific SQL references alongside it when available.

## Implementation Focus

### Context Ownership

Treat `DbContext` as a short-lived unit of work and not thread-safe. Web scopes are handled by ASP.NET guidance; workers/parallel jobs create/dispose contexts per unit through `IDbContextFactory` or an explicit scope.

Do not keep contexts/entities attached across long jobs, threads, UI sessions, queues, or retries. Clear/recreate tracking when lifetime changes.

Configure provider/connection/retry/logging/migrations in composition code, not domain entities or repositories.

### Model And Mapping

Configure required/nullability, lengths, precision/scale, Unicode/collation, indexes/uniqueness, alternate keys, foreign keys, cardinality, owned/complex/value-converted types, delete behavior, generated/default/computed values, concurrency tokens, and global filters according to invariants.

Keep domain/API/UI models separate from EF entities when navigation, persistence metadata, serialization, mutation, or lifecycle differs.

Be explicit about provider semantics. In-memory provider cannot prove relational constraints, transactions, SQL translation, case sensitivity, or type behavior.

### Query Shape

Project read models at the database boundary and use `AsNoTracking`/identity resolution according to object reuse needs. Avoid loading full graphs for summaries.

Prevent N+1 with projection, targeted Include, split/single query choice, or explicit loading. Do not globally eager-load to conceal one path.

Keep filtering/sorting/pagination server-translatable and deterministic with tie breakers. Inspect generated SQL/query plans for claimed query improvements.

Avoid client evaluation, accidental multiple enumeration/query execution, cartesian explosion, unbounded `ToList`, and lazy loading outside context lifetime.

### Writes And Transactions

Place `SaveChanges` at an application/unit-of-work boundary. Helpers/repositories should not commit independently when writes must be atomic.

Use explicit transactions for multiple saves/commands or external coordination when the implicit transaction is insufficient. Integrate execution strategies correctly so the whole retryable unit is replayed safely.

Database retries can repeat application code; require idempotency for generated identifiers, external calls, messages, and side effects. Use outbox/reconciliation for DB plus external effects when architecture requires it.

### Concurrency

Use rowversion/timestamp or another accepted token for stale-update protection. Catch `DbUpdateConcurrencyException`, inspect current/original/database values, and choose reject/merge/retry explicitly.

Do not blindly retry business updates after conflict. Return actionable state and preserve user work where relevant.

Unique/FK/check constraint violations remain race-safe enforcement; map provider exceptions without brittle message parsing where provider APIs expose codes.

### Migrations And Existing Data

Review generated migration operations and model snapshot. Hand-edit intentionally for data backfill, rename, online/batched changes, defaults, constraint rollout, and provider SQL.

Plan expand/backfill/contract when old and new application versions overlap. Avoid destructive drop/recreate or non-null addition without existing-data handling.

Generate idempotent/scripted artifacts according to deployment policy and do not auto-migrate concurrently at every worker instance unless startup ownership is explicit.

### Bulk And Raw Operations

`ExecuteUpdate/Delete`, bulk libraries, raw SQL, and compiled queries bypass or alter tracking/interceptors/domain behavior. Use them for measured/bounded needs and reconcile tracked/cache state.

Parameterize raw SQL and validate dynamic identifiers/order clauses through allowlists. Preserve tenant/global-filter and concurrency behavior deliberately.

## Verification Focus

- Run integration tests against the selected relational provider/container/database for changed translation/mapping/migration semantics.
- Apply migrations to representative existing data and inspect generated SQL/model snapshot.
- Test write/read, rollback, constraint violation, concurrency conflict, retry/idempotency, global filter, and context lifetime when owned.
- Assert query shape/count/order/pagination and no client/N+1/unbounded behavior.
- Verify worker scopes/factories dispose contexts and never share one concurrently.

## Evidence Focus

Name provider, context/unit-of-work owner, mapping/query/transaction/concurrency/migration decision, and provider-backed assertion. Passing EF InMemory tests or migration generation alone does not prove relational behavior or existing-data safety.

## Unsafe Defaults

- This file loaded alongside duplicate ASP.NET Core data guidance.
- Long-lived/shared DbContext in workers or parallel tasks.
- EF InMemory used as relational proof.
- SaveChanges scattered across repository/helper calls.
- Execution-strategy retry repeating non-idempotent side effects.
- Destructive migration accepted without existing-data/cutover plan.
