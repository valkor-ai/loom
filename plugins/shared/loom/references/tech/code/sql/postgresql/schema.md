# PostgreSQL Schema Mapping

Use this file with `tech/code/sql/schema.md` when the accepted persistence provider is PostgreSQL and the task owns schema, migration, entity mapping, or database-backed invariants.

## When To Use

- Read the repository's PostgreSQL version, driver, ORM, migration tool, extension policy, and existing migration style before choosing syntax.
- Apply provider rules only to fields, constraints, indexes, and migrations owned by the task.
- Keep server administration and unrelated platform work outside this implementation reference.

## Implementation Focus

- Choose identity columns, application-generated UUIDs, or another key strategy from domain needs, migration policy, ORM support, and existing schema. Do not introduce `SERIAL` only because it is familiar.
- Use `TIMESTAMPTZ` when the business value represents an instant across time zones. Keep application serialization and comparison rules aligned.
- Use `NUMERIC` for exact quantities and match precision/scale to domain and API contracts.
- Use `TEXT` when no length invariant exists; use bounded text types when the business rule or index strategy requires a limit.
- Use JSONB, arrays, INET, or CIDR only when the domain and query contract need their semantics. Stable, frequently queried fields should remain typed columns when that keeps ownership clear.
- Do not use `gen_random_uuid()` or another extension function unless the extension is an accepted migration dependency and is verified in the target environment.

## Constraints And Indexes

- Define primary keys, foreign keys, unique constraints, check constraints, and nullability at the database layer for durable invariants.
- Use partial indexes, GIN, GiST, BRIN, or covering indexes only for a named query/access path and supported provider version.
- Treat partitioning as an architecture decision backed by data volume, retention, write pattern, and query evidence. It is not a default schema step.
- Keep cascade behavior aligned with domain ownership and migration safety.

## Migration And ORM Alignment

- Keep migration DDL, ORM mappings, extension installation, enum representation, nullability, defaults, and API DTOs aligned.
- Review a clean migration path and an upgrade path when existing data is in scope.
- Do not hide provider features in auto-generated schema changes without reviewing the generated migration.

## Compatibility Checklist

- Confirm whether required extensions are an accepted migration dependency and are available in every environment owned by the task.
- Compare application nullability with PostgreSQL column nullability and default expressions.
- Keep timestamp storage, application serialization, and timezone comparisons aligned.
- Check enum, domain, JSONB, array, network, and numeric mappings in the driver and ORM before changing an existing column.
- Check foreign-key types, collations, and referenced key definitions on both sides of every relationship.
- Confirm that partial, GIN, GiST, BRIN, or covering indexes are supported by the target PostgreSQL version and match the query predicate.
- Treat partitioning as a separate architecture decision with migration and query ownership, not as a schema decoration.

## Persistence Shape Review

- Name the entity or table owner, durable invariant, migration owner, and query path affected by the change.
- State whether the change is additive, compatible with existing rows, or requires a data backfill.
- Keep API read/write models separate from generated values, internal flags, and storage-only fields.
- Verify that a failed migration or partial write does not leave a state that the application cannot read.

## Verification Focus

- Run migrations or application startup against the configured PostgreSQL target or compatible provider.
- Prove UUID/identity generation, timestamp semantics, JSONB/array mapping, constraints, index behavior, and relations touched by the task.
- Record the PostgreSQL version, extension dependency, and provider behavior verified.

## Evidence Focus

- In the evidence summary, name the schema decision made: type mapping, identity, extension dependency, constraint, index, migration compatibility, or ORM alignment.

## Risks To Avoid

- Assuming every PostgreSQL installation has the required extension enabled.
- Introducing partitioning, RLS, or custom types because the provider supports them without a current requirement.
- Testing PostgreSQL-specific types or indexes only with SQLite, H2, or an in-memory mock.
