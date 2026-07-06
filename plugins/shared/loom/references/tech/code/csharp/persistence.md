# C# Entity Framework Quality

Use this topic reference when `tech/code/csharp/persistence.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes EF Core DbContext setup, entities, configurations, migrations, repositories, query projections, transactions, tracking behavior, interceptors, or database-backed business rules.
- Use this for relational persistence decisions in C#/.NET. If persistence uses Dapper, MongoDB, or another provider, apply only the relevant transaction/query/DTO principles.
- If a task changes only HTTP DTOs with no persistence behavior, do not expand into EF changes.

## Implementation Focus

- Keep API DTOs and EF entities separate when entities contain navigation properties, audit fields, concurrency tokens, soft-delete flags, or internal state.
- Configure required fields, lengths, precision, indexes, relationships, delete behavior, owned types, and query filters explicitly when they matter to business invariants.
- Use the correct `DbContext` lifetime. In web apps it is normally scoped per request; do not inject a scoped context into singletons or background services without a proper scope factory.
- Use `AsNoTracking` and projection DTOs for read-only list/detail queries that do not need change tracking. Do not load full aggregate graphs only to render summary rows.
- Avoid N+1 queries with targeted projections, includes, split queries, or explicit loading according to the read path. Do not globally eager-load relationships to hide one query issue.
- Put multi-entity writes and business state transitions in a clear transaction boundary. Keep `SaveChangesAsync` placement deliberate rather than scattered across helper methods.
- Forward `CancellationToken` into EF async calls. Avoid sync EF calls on request paths.
- Review generated migrations before accepting them. Reject unintended table/column drops, broad nullable changes, data-loss operations, and provider-specific type surprises.
- Use optimistic concurrency tokens only when concurrent updates are a real risk, then handle conflicts with a clear response or retry policy.
- Use bulk updates/deletes, compiled queries, interceptors, and raw SQL only for a demonstrated need and keep the behavior covered by tests or review evidence.

## Verification Focus

- Run repository or integration tests against the configured provider or a provider-compatible test database.
- If schema changed, generate/review migration output and run migration/database update or a startup validation path when available.
- Test write/read round-trip for changed persistence behavior, including transaction rollback or failure branches when relevant.
- Test filters, sorting, pagination, projection fields, not-found/empty results, and soft-delete/global-filter behavior touched by the task.

## Evidence Notes

- Record `csharp.persistence` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/csharp/persistence.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the persistence decision: entity mapping, DTO projection, tracking choice, transaction boundary, migration review, query optimization, or concurrency handling.
