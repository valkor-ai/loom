# SQL Schema Quality

This file applies to portable relational schema and migration work. Load the selected provider overlay when the target database changes type, index, identity, JSON, or constraint behavior.

## When To Use

- The task changes migrations, table definitions, constraints, keys, indexes, audit/history tables, soft deletes, tenant columns, ORM mappings, seed/reference data, or database-backed invariants.
- Use this when schema structure affects business correctness, data integrity, query behavior, or application startup.
- If the task only changes read queries without schema changes, use `sql.queries` or `sql.optimization` when selected.
- Do not load this file for a controller, API DTO, or frontend task that does not own persistence mapping.

## Implementation Focus

- Normalize business data enough that facts have one owner. Denormalize only for a named read/performance reason and keep the source-of-truth relationship clear.
- Use primary keys, foreign keys, unique constraints, check constraints, and not-null/default rules to enforce important invariants at the database layer.
- Choose data types for domain semantics and target dialect behavior: money/decimal precision, timestamp timezone, text length, enum/state storage, UUID/integer keys, and JSON only for truly flexible data.
- Define relationship ownership before adding cascade rules. `ON DELETE CASCADE`, soft delete, restrict, and set-null each encode different business behavior.
- Add indexes for foreign keys, common filters, uniqueness, and sort/pagination paths. Do not add indexes unrelated to a known query or invariant.
- For many-to-many relationships, use a join table with explicit uniqueness and any relationship attributes. Avoid comma-separated IDs or unconstrained polymorphic references for core data.
- Soft-delete and audit/history tables must preserve query semantics. Include active-record indexes or views/scopes where the application expects active-only behavior.
- Migration files should be deterministic, reviewable, and compatible with the repository's migration tool. Avoid relying on ORM auto-DDL for production schema behavior when migrations exist.
- Keep ORM mappings, schema constraints, and application validation consistent. Application validation may improve user feedback but should not be the only protection for core invariants.
- Keep transaction ownership and state-transition invariants in the service/application boundary defined by Architecture. The schema enforces durable invariants; it does not replace domain workflow logic.
- Treat provider-specific type and index choices as a separate dialect decision. Do not duplicate PostgreSQL or MySQL syntax in this common file.

### Temporal, Audit, And Soft-Delete Data

- Use a history table or valid-time columns when the requirement is to answer what was true at a past time. Define interval boundaries, overlap rules, current-row uniqueness, and the read path; do not add history columns without a historical query contract.
- Keep business audit events distinct from technical migration or database log records. An audit record needs actor/system identity, operation, time, affected subject, and a redaction policy for old/new values. Do not make triggers the only source of business meaning when the application owns the state transition.
- Soft delete is a query and uniqueness contract, not only a nullable timestamp. Define the active predicate, restore behavior, foreign-key behavior, retention, and indexes/unique constraints for active rows. Every owned read path must apply the same visibility rule.

### Migration Compatibility

- Classify a schema change as additive, backfill, compatible rewrite, or destructive removal before writing the migration.
- For existing data, define the expand/backfill/contract order and the state the application can read at each intermediate step. Add defaults or nullable staging columns only when their temporary semantics are explicit.
- Verify clean installation and upgrade from a representative prior schema. A migration that succeeds on an empty database does not prove compatibility with existing rows, indexes, constraints, or application read paths.

## Verification Focus

- Run migration/app initialization for the target test database and verify schema validation when an ORM is present.
- Test constraint behavior for required fields, unique business keys, foreign keys, check constraints, and delete/update behavior touched by the task.
- For ORM-backed changes, prove write/read mapping, enum/state conversion, nullable/default handling, and relation loading needed by the task.
- For soft delete/audit/history changes, test active query behavior and recorded historical data.
- Run the repository's migration validation path from a clean schema when migration files changed. Do not validate only that the migration file parses.
- For a compatibility migration, verify the intermediate schema during backfill and the final schema after the old representation is removed.

## Evidence Focus

- In the evidence summary, name the schema decision: normalization, key strategy, constraint, data type, cascade behavior, index, soft delete, audit/history, migration compatibility, or ORM alignment.

## Risks To Avoid

- Relying on ORM auto-DDL when the project has a migration contract.
- Adding a provider-specific column type without target-provider verification.
- Using UI validation as the only protection for a durable invariant.
- Adding indexes without a known filter, join, uniqueness, or ordering path.
- Adding history, audit, or soft-delete columns without defining their read, restore, retention, and uniqueness semantics.
- Treating a clean-database migration run as proof that an upgrade path is safe.
