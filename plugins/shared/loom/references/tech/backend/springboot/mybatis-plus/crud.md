# MyBatis-Plus Mapper And Service CRUD

## When To Use

Use this reference when a task implements or changes MyBatis-Plus Mapper, Service, list, detail, batch, paging, or stream-query behavior.

## Implementation Focus

- Keep controllers and transport adapters out of direct Mapper calls when the repository has a Service/application layer.
- `BaseMapper` is a persistence primitive; `IService` and `ServiceImpl` are optional project conventions, not mandatory boilerplate.
- Keep authorization, tenant scope, audit, transaction, and domain rules in the established application/service boundary.

## Reads And Writes

- Bound list queries with `Page`/`IPage`, an explicit limit, or a task-owned result-size contract. Do not use unbounded `selectList` for user-controlled collections.
- Use `saveBatch` and batch updates only with an explicit batch size, transaction boundary, duplicate behavior, and failure handling.
- Treat `removeById` according to the accepted logical-delete contract; physical deletion requires an explicit business reason.
- Stream large results only with a defined connection, transaction, resource-close, and cancellation boundary.

## Verification Focus

Prove cardinality, not-found and duplicate behavior, pagination totals, batch rollback or partial-failure semantics, logical deletion, authorization/tenant conditions, and read-after-write behavior where required.

## Failure Boundaries

- Translate known constraint and optimistic-lock failures at the application boundary without hiding the original rollback behavior.
- Keep external calls outside a database transaction unless the accepted design defines an outbox or compensation boundary.
- Do not make a passing Mapper unit test stand in for provider-specific SQL or migration evidence.
- Keep batch operations idempotent when the task can be retried by the execution or runtime workflow.
- Preserve deterministic ordering for every paged or streamed collection.

## Evidence Focus

Record the owned Mapper or Service methods, query bounds, transaction boundary, and evidence for pagination, failure, authorization, and retry behavior.
