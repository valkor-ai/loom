# ASP.NET Core EF Core Quality

Use this topic reference when `tech/backend/aspnetcore/data.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`, selected C# persistence references, and selected SQL references. This file applies EF Core, DbContext, migrations, and query rules to task-owned persistence behavior.

## When To Use

- The task changes DbContext, entity configurations, migrations, relationships, owned types, indexes, query handlers, repositories, transactions, or database-backed business rules.
- Use this when EF Core mapping, migration correctness, query shape, or transaction behavior affects correctness.
- If the task only changes Minimal API endpoints with no persistence behavior, do not load this data reference.

## Implementation Focus

- Configure entities with `IEntityTypeConfiguration` or the repository's existing convention. Keep table names, keys, required fields, precision, lengths, indexes, and delete behavior explicit.
- Use migrations as the schema source of truth. Do not rely on runtime schema creation for production behavior.
- Use async EF Core APIs with `CancellationToken` for all database I/O.
- Use `AsNoTracking`, projections, includes, split queries, and pagination deliberately. Do not load full aggregate graphs only to serialize list rows.
- Keep domain entities out of response DTOs; project to records/read models when returning data.
- Use transactions for multi-record state changes and side effects that must commit together.
- Handle uniqueness, concurrency, not-found, and integrity failures with stable application/API errors.
- Keep provider-specific behavior visible when using SQL Server, PostgreSQL, SQLite, or other EF providers.

## Verification Focus

- Run EF Core tests, integration tests, migration commands, or repository-specific persistence tests.
- Prove write/read round-trip, generated IDs, enum/value object mapping, defaults, constraints, relationship loading, pagination/filtering, and rollback behavior when touched.
- For migrations, verify a clean database can apply them and the app starts with the configured provider.
- For performance-sensitive queries, test result correctness and query shape/query count when the repository supports it.

## Evidence Notes

- Record `aspnetcore.data` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/aspnetcore/data.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the EF Core decision: entity configuration, migration, DbContext boundary, query projection, relationship loading, transaction boundary, provider behavior, or persistence proof.
