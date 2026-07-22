# MyBatis-Plus Extensions And High-Impact Features

## Generators And IDs

- Confirm package, module, table-prefix, parent class, naming, XML output, and overwrite policy before using `FastAutoGenerator` or `AutoGenerator`.
- Generated files require review of annotations, field types, deletion/version/fill behavior, indexes, and permission entry points.
- `IdType.AUTO`, `ASSIGN_ID`, and a custom `IdentifierGenerator` have different database, distribution, clock, serialization, and historical-data contracts.

## Global Extensions

- A custom SQL injector, base Mapper method, TypeHandler, plugin, or ID generator affects every module using that base. Prefer a local solution first.
- ActiveRecord, `Db`, and `SimpleQuery` are optional tools for small or existing repository patterns; they must not bypass Service transactions, permission, tenant, audit, or cache rules.

## DDL And Multiple Data Sources

- Automatic DDL is not a migration strategy. Use the accepted Flyway/Liquibase or repository migration process with rollback and historical-data review.
- For multiple data sources, verify each `SqlSessionFactory` has the required Mapper paths, TypeHandlers, plugins, and transaction manager.
- Cross-data-source transactions, read/write routing, and plugin order are high-impact architecture decisions, not Mapper-only changes.

## Verification Focus

Review generated diffs, ID uniqueness, extension scope, migration startup, per-data-source registration, transaction behavior, and rollback or recovery boundaries.

## Non-Selection Rule

- Do not select this reference for ordinary entity or CRUD work unless the task owns one of these high-impact extensions.
- Do not introduce an extension merely to reduce local boilerplate or to bypass an existing Service boundary.
- Keep deployment and secret-management decisions in RuntimeDelivery and Deploy contracts.
