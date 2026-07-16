# Oracle Schema Mapping

Use this file with `tech/code/sql/schema.md` when the accepted persistence provider is Oracle and the task owns schema, migration, entity mapping, or database-backed invariants.

## When To Use

- Read the Oracle version, compatibility mode, driver, ORM, migration tool, and existing migration style before choosing syntax.
- Apply provider rules only to fields, constraints, indexes, and migrations owned by the task.
- Keep database platform administration and unrelated environment work outside this implementation reference.

## Implementation Focus

- Choose identity columns, sequences, application-generated UUIDs, or another key strategy from domain needs, migration policy, ORM support, and existing schema.
- Use `NUMBER` with explicit precision and scale for exact quantities. Do not rely on implicit numeric conversion at the application boundary.
- Use `VARCHAR2`/`NVARCHAR2` for text according to character semantics and length requirements. Remember that Oracle treats an empty string as `NULL` in character columns.
- Use `TIMESTAMP` or `TIMESTAMP WITH TIME ZONE` from the business instant/local-time contract. Keep driver conversion and API serialization aligned.
- Use native JSON, JSON functions, generated columns, function-based indexes, or custom types only when the Oracle version and migration contract support them.

## Constraints And Indexes

- Define primary keys, foreign keys, unique constraints, check constraints, and nullability at the database layer for durable invariants.
- Use B-tree, bitmap, function-based, or domain indexes only for a named access path and workload. Bitmap indexes require an explicit workload decision and are not a default OLTP choice.
- Keep cascade behavior aligned with domain ownership and migration safety.

## Migration And ORM Alignment

- Keep migration DDL, ORM mappings, sequence/identity configuration, enum/state conversion, nullability, defaults, and API DTOs aligned.
- Review generated migration SQL and object naming/quoting before using Oracle-specific features.
- Verify clean installation and upgrade behavior when existing rows, sequences, indexes, constraints, or backfills are in scope.

## Compatibility Checklist

- Confirm Oracle version and compatibility mode for identity, JSON, pagination, analytic, and temporal syntax.
- Check `NUMBER` precision/scale, timestamp zone semantics, and empty-string/null behavior in the driver and ORM.
- Check sequence allocation and generated-key retrieval through the actual data-access path.
- Check foreign-key types, length semantics, collation, and nullability on both sides of every relationship.
- Treat partitioning, flashback/history, row-level security, and custom types as explicit architecture/data decisions.

## Persistence Shape Review

- Name the table owner, durable invariant, migration owner, and query path affected by the change.
- State whether the change is additive, compatible with existing rows, or requires a backfill.
- Keep API read/write models separate from generated values, internal flags, and storage-only fields.
- Verify that a failed migration or partial write does not leave a state the application cannot read.

## Verification Focus

- Run the changed migration or application startup against Oracle or the repository's provider-compatible path.
- Prove generated identity, numeric precision, timestamp/zone semantics, empty-string behavior, constraints, indexes, and mappings touched by the task.
- Record Oracle version, compatibility mode, and provider behavior verified.

## Evidence Focus

- Name the schema decision proved: type mapping, identity, sequence, constraint, index, migration compatibility, or ORM alignment.

## Risks To Avoid

- Treating empty strings as distinct from `NULL` in application validation or uniqueness logic.
- Relying on implicit numeric/date conversion or session NLS settings.
- Adding bitmap indexes, partitioning, or custom types without a named workload and migration boundary.
- Testing Oracle-specific behavior only with SQLite, H2, or an in-memory mock.
