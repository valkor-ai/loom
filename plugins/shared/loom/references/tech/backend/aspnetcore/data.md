# Entity Framework Core Persistence

Apply this reference only when Entity Framework Core is selected and the task owns persistence. The accepted data architecture and database provider determine mapping and migration behavior; ASP.NET Core does not imply EF Core or SQL Server.

## DbContext Boundary

Register `DbContext` with the provider and lifetime already selected by the application. A scoped context usually represents one unit of work; it is not thread-safe and must not be stored in singletons or used concurrently across tasks.

Keep provider setup and connection configuration in the composition root. Validate required connection/options at startup without logging credentials. Use `IDbContextFactory<T>` for accepted background/concurrent scopes that need independent contexts, not to bypass normal request scope.

Do not call `EnsureCreated` in a migration-owned production database. Keep design-time factory/setup aligned with runtime provider and migrations.

## Model Configuration

Use `IEntityTypeConfiguration<T>` or the repository's established configuration style. Make keys, generated values, required/optional fields, lengths, precision/scale, Unicode/collation, indexes, alternate keys, concurrency tokens, and delete behavior explicit where correctness depends on them.

```csharp
public sealed class OrderConfiguration : IEntityTypeConfiguration<Order>
{
    public void Configure(EntityTypeBuilder<Order> builder)
    {
        builder.HasKey(x => x.Id);
        builder.Property(x => x.Status).HasConversion<string>().HasMaxLength(32);
        builder.Property(x => x.Total).HasPrecision(18, 2);
        builder.HasIndex(x => new { x.TenantId, x.Number }).IsUnique();
        builder.Property(x => x.Version).IsConcurrencyToken();
    }
}
```

Choose owned/complex types, value converters, backing fields, and join entities based on domain lifecycle and provider support. A converter changes persisted representation but may not preserve query translation or comparison semantics.

Set `DeleteBehavior` from ownership. Cascades are not a convenience default. Handle required relationships, orphan behavior, cycles, and soft-delete filters deliberately.

## Query Shape

Use `AsNoTracking` for read-only queries unless identity resolution or updates are required. Project directly to response/read models and select only required columns.

Use `Include` for aggregate loading only when the entity graph is actually needed. Prefer projections for lists; use split queries when multiple collections would create cartesian explosion and verify their consistency/performance tradeoff.

Bound filters, sorting, and pagination with deterministic order. Avoid client evaluation, `ToList` before filtering, lazy-loading N+1 behavior, and unbounded collection materialization. Inspect generated SQL for complex or performance-sensitive queries.

Use compiled queries only after measurement shows repeated translation cost matters. Provider indexes and query plans remain the primary performance boundary.

## Writes, Concurrency, And Transactions

Use async EF APIs and propagate `CancellationToken`. Attach/update graphs deliberately; broad `Update(entity)` can mark every field modified and overwrite concurrent changes.

Database constraints are the final uniqueness/integrity boundary. Translate known `DbUpdateException` cases through provider-aware adapters and do not parse message text throughout services.

Use row-version/concurrency tokens or explicit state/version predicates when stale writes matter. Catch `DbUpdateConcurrencyException`, decide reload/merge/reject behavior, and map the accepted conflict response.

`SaveChanges` is transactional for its batch. Use an explicit transaction for multiple saves/contexts or coordinated operations that require one database atomic boundary. Keep network/message/email work outside the transaction; use an accepted outbox for durable publication.

## Migrations And Data Evolution

Generate migrations from the intended model, inspect every operation, and keep migration history in source control. Use expand/backfill/switch/contract steps for compatibility-sensitive changes rather than one destructive migration.

Large backfills, non-null additions, index creation, and provider-specific online behavior need bounded operational strategy. Data migrations must be deterministic and restart-safe when the deployment process can retry.

Do not auto-apply migrations from every application replica unless the runtime contract explicitly coordinates it. Verify both clean creation and upgrade from a representative prior schema when upgrade behavior is claimed.

## Provider Fidelity

SQL Server, PostgreSQL, MySQL, and SQLite differ in generated values, decimals, date/time, JSON, collations, indexes, computed columns, migrations, locking, and concurrency. Use the selected provider for provider-specific claims.

SQLite is suitable only when it is the accepted production provider or the tested behavior is provider-neutral. An EF InMemory provider does not prove relational constraints, transactions, query translation, or migration behavior.

## Verification

- Prove create/update/readback for mappings, defaults, generated values, enums/value objects, relationships, and decimals/timestamps.
- Assert uniqueness/check/delete and optimistic-concurrency outcomes.
- Verify projection, filtering, deterministic ordering, pagination, and relevant generated SQL/query count.
- Exercise transaction rollback and absence of partial side effects.
- Apply migrations to a clean selected-provider database and test representative upgrade paths when changed.
- Confirm cancellation and context lifetime behavior for background/concurrent use when owned.

## Delivery Evidence

Identify the EF configuration/query/migration and the selected-provider assertion proving it. A passing mock, InMemory test, generated migration file, or successful startup alone cannot prove relational integrity, query translation, concurrency, or upgrade safety.

## Unsafe Defaults

- EF Core selected because ASP.NET Core is present.
- `EnsureCreated` used alongside migrations in production.
- `DbContext` shared across threads or singleton services.
- Entities returned from HTTP responses.
- `Include` used to load full graphs for list endpoints.
- `Update` applied to detached client-shaped objects.
- SQLite/InMemory evidence claimed for provider-specific behavior.
- Destructive migrations without compatibility/backfill planning.
