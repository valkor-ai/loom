# MySQL Schema Mapping

Use this file with `tech/code/sql/schema.md` when the accepted persistence provider is MySQL and the task owns schema, migration, entity mapping, or database-backed invariants.

## When To Use

- Read the repository's MySQL version, driver, ORM, migration tool, SQL mode, and existing migration style before choosing syntax.
- Apply provider rules only to fields, constraints, indexes, and migrations owned by the task.
- Keep server administration and unrelated platform work outside this implementation reference.

## Implementation Focus

- Use InnoDB for transactional business tables unless the repository contains an explicit legacy engine decision that the task must preserve.
- Choose integer identity, UUID, or another key strategy from domain scale, external references, ORM support, and existing schema. `UNSIGNED` is not automatically better when values cross application or service boundaries.
- Use `DECIMAL` for money and other exact quantities. Match precision and scale to the domain and serialized API representation.
- Treat MySQL boolean storage, enum storage, nullable fields, default expressions, and timestamp behavior as explicit mapping decisions. Do not let driver defaults define the contract.
- Use `utf8mb4` for user-visible text and choose collation from comparison, ordering, and case-sensitivity requirements. Do not copy a collation without checking the target MySQL version.
- Use JSON or generated columns only when the flexible shape and indexed access path are part of the current requirement. Frequently queried stable fields belong in typed columns when that preserves the domain model.

## Constraints And Indexes

- Define foreign keys, uniqueness, not-null rules, and check behavior in a form supported by the target MySQL version and migration tool.
- Add indexes for actual foreign-key, filter, join, uniqueness, or ordering paths. Index column order must follow the query predicates and sort requirements, not a blanket selectivity slogan.
- Treat full-text, spatial, functional, and generated-column indexes as explicit provider features. Include the query that needs them and verify the resulting plan.
- Keep cascade behavior aligned with domain ownership. Do not use cascade deletes to hide an unexamined lifecycle decision.

## Migration And ORM Alignment

- Keep migration column definitions, ORM annotations/configuration, enum conversion, nullability, generated values, and API DTOs aligned.
- Do not rely on Hibernate or another ORM to silently create production schema when the repository has migrations.
- Review changes against a clean MySQL schema and an upgrade path when existing data is in scope.

## Compatibility Checklist

- Confirm whether the migration tool emits or preserves the selected engine, charset, collation, generated values, and foreign-key behavior.
- Compare application nullability with MySQL column nullability. A nullable application field and a non-null column are different contracts.
- Check timestamp defaults and update behavior against the application's explicit clock and timezone policy.
- Check enum or state storage against future state additions. A database enum is not automatically safer than a validated string or reference table.
- Check UUID and binary storage mappings in the driver and ORM before changing an existing identifier representation.
- Check generated columns and functional indexes against the exact MySQL version used by the project.
- Check foreign-key column types, signedness, length, and collation on both sides of every relationship.

## Persistence Shape Review

- Name the entity or table owner, durable invariant, migration owner, and query path affected by the change.
- State whether the change is additive, compatible with existing rows, or requires a data backfill.
- Keep API read/write models separate from generated columns, internal flags, and storage-only values.
- Verify that a failed migration or partial write does not leave a state that the application cannot read.

## Verification Focus

- Run the changed migration or application startup against the configured MySQL target or compatible provider.
- Prove write/read mapping, generated identity, decimal precision, timestamps, enum/state conversion, constraints, and indexes touched by the task.
- Record the MySQL version or compatibility source and the exact provider behavior verified.

## Evidence Focus

- In the evidence summary, name the schema decision made: type mapping, identity, collation, constraint, index, migration compatibility, or ORM alignment.

## Risks To Avoid

- Copying database administration examples into application code.
- Assuming MySQL syntax is valid for MariaDB or another SQL engine.
- Using MyISAM for a transactional workflow without an accepted legacy decision.
- Testing MySQL-specific behavior only with SQLite, H2, or an in-memory mock.
