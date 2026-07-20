# SQL Server Schema Mapping

Use this file with `tech/code/sql/schema.md` when the accepted persistence provider is SQL Server and the task owns schema, migration, entity mapping, or database-backed invariants.

## When To Use

- Read the SQL Server version, compatibility level, driver, ORM, migration tool, and existing migration style before choosing syntax.
- Apply provider rules only to fields, constraints, indexes, and migrations owned by the task.
- Keep instance administration and unrelated platform work outside this implementation reference.

## Implementation Focus

- Choose `IDENTITY`, sequences, application-generated UUIDs, or another key strategy from domain scale, migration policy, ORM support, and existing schema.
- Use `DECIMAL` with explicit precision and scale for exact quantities. Match the database definition to the application and API representation.
- Use `datetime2` for timestamps without offset and `datetimeoffset` when the stored value represents an instant with offset semantics. Do not let driver conversion choose the business meaning.
- Use `nvarchar` for user-visible Unicode text and define length only when it is a business, storage, or index constraint. Treat `bit` as a boolean mapping decision rather than a general integer.
- Use JSON functions over validated text only when the flexible shape and query path are part of the accepted contract. Keep stable, frequently queried fields typed.

## Constraints And Indexes

- Define primary keys, foreign keys, unique constraints, check constraints, and nullability at the database layer for durable invariants.
- Use filtered indexes only when the filter predicate exactly matches the active-record or query contract. Use included columns only for a named read path that benefits from covering behavior.
- Choose clustered/nonclustered key placement from access patterns and write behavior; do not apply a universal clustered-key rule.
- Keep cascade behavior aligned with domain ownership and migration safety.

## Migration And ORM Alignment

- Keep migration DDL, ORM mappings, generated values, enum/state conversion, nullability, defaults, and API DTOs aligned.
- Review compatibility level and generated migration SQL before using provider-specific functions, filtered indexes, computed columns, or temporal features.
- Verify clean installation and upgrade behavior when existing rows, indexes, constraints, or computed values are in scope.

## Compatibility Checklist

- Confirm SQL Server version and compatibility level for JSON, string aggregation, filtered indexes, computed columns, and pagination syntax.
- Check `datetime2`/`datetimeoffset` conversion, precision, and application serialization.
- Check identity/sequence behavior and generated-key retrieval through the actual driver or ORM.
- Check foreign-key types, length, collation, and nullability on both sides of every relationship.
- Check computed-column determinism and indexability before using it as an access path.
- Keep row-level security, temporal tables, and columnstore choices in an explicit architecture or data decision; they are not default schema features.

## Persistence Shape Review

- Name the table owner, durable invariant, migration owner, and query path affected by the change.
- State whether the change is additive, compatible with existing rows, or requires a backfill.
- Keep API read/write models separate from computed columns, internal flags, and storage-only values.
- Verify that a failed migration or partial write does not leave a state the application cannot read.

## Verification Focus

- Run the changed migration or application startup against SQL Server or the repository's provider-compatible path.
- Prove generated identity, Unicode/length behavior, decimal precision, timestamp semantics, constraints, filtered/indexed paths, and mappings touched by the task.
- Record SQL Server version, compatibility level, and provider behavior verified.

## Evidence Focus

- Name the schema decision proved: type mapping, identity, collation, constraint, index, computed value, migration compatibility, or ORM alignment.

## Risks To Avoid

- Treating `datetime`, `bit`, implicit conversions, or collation defaults as portable semantics.
- Adding filtered indexes or computed columns without verifying the exact predicate and compatibility level.
- Testing SQL Server-specific behavior only with SQLite, H2, or an in-memory mock.
