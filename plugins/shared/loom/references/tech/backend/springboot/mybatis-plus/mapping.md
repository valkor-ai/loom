# MyBatis-Plus Entity Mapping

## When To Use

Use this reference when a task owns MyBatis-Plus entity annotations, column mapping, IDs, logical deletion, optimistic locking, enum or JSON fields, or automatic audit fill.

## Implementation Focus

- Use `@TableName` only when the table, schema, result-map, or excluded-property behavior differs from the repository defaults.
- Use `@TableId` with an explicit `IdType` when the ID generation contract is not obvious from the database and existing configuration.
- Use `@TableField` for non-default column names, field strategy, fill behavior, or a deliberate TypeHandler.
- `@TableLogic` requires confirmed delete values, query behavior, restore behavior, and an index/uniqueness strategy for retained rows.
- `@Version` requires explicit handling of an update count of zero as a concurrency conflict.

## Enum, JSON, And Audit Fields

- Keep the persistence value of an enum separate from its API label or localized display text.
- Use one project-wide TypeHandler convention for JSON, encrypted, or provider-specific values. Cover read, write, null, malformed, and historical values.
- `autoResultMap = true` may be required for field-level TypeHandlers; verify the actual result mapping path.
- `MetaObjectHandler` is for accepted audit fields such as creation time or actor. It must not hide missing business input or assign business state implicitly.
- Entity mapping changes must stay aligned with the migration and API contracts.

## Verification Focus

Test persisted round trips for IDs, field names, enums, JSON, nulls, defaults, logical deletion, automatic fill, and stale-version updates against the selected provider where provider behavior matters.

## Boundary Checks

- Do not return persistence entities directly as API response models when the repository uses DTOs.
- Keep field strategies and fill handlers consistent for `updateById`, `update(Wrapper)`, imports, jobs, and administrative actions.

## Evidence Focus

Name the affected entities, table and field contracts, handlers, migration dependency, and round-trip or concurrency evidence for the changed mapping.
