# Spring Boot Data Quality

This file applies Spring Data and migration rules to task-owned persistence behavior.

## When To Use

- The task changes Spring Data JPA repositories, entity mappings, migrations, transactions, auditing, projections, Specifications, repository queries, or database-backed business rules.
- Use this when Spring Data repository semantics, transaction boundaries, query shape, or migration startup behavior affects correctness.
- If the task only adds a web DTO or controller with no persistence change, do not load this data reference.

## Implementation Focus

- Treat entities as persistence models with invariants and lifecycle, not HTTP DTOs. Keep response/read models separate from lazy relationships and internal audit/version fields.
- Put multi-record writes, state transitions, and business validation inside service-level `@Transactional` methods. Repository interfaces should expose persistence operations, not hide full workflows.
- Use Spring Data derived queries only for simple predicates. Use `@Query`, Specifications, projections, or query methods with explicit names when filters, joins, or performance concerns need clarity.
- Prefer DTO/interface projections or dedicated read queries for list/detail read models. Do not load full aggregate graphs only to serialize a table row.
- Keep relationships lazy by default and load required relationships for a specific use case with projections, fetch joins, `@EntityGraph`, or explicit query shape. Do not globally switch mappings to eager to mask N+1.
- Define optimistic locking, auditing annotations, soft delete, cascade, and orphan removal only when the business lifecycle requires them.
- Keep Flyway/Liquibase migrations, entity annotations, enum storage, nullability, column lengths, and database dialect behavior aligned. Do not rely on Hibernate auto-DDL for production schema changes.
- For SQLite or other type-affinity databases, ensure JPA/Flyway column choices and Hibernate validation behavior match the actual runtime.

## Verification Focus

- Run `@DataJpaTest`, repository tests, migration startup, or integration tests against the configured test database.
- Prove write/read round-trip for changed persistence behavior, including generated IDs, enum/status mapping, timestamps/auditing, nullability/defaults, and relationships touched by the task.
- Test repository filters, projections, sorting, pagination/counts, not-found, duplicate/unique constraint, and transaction rollback branches when relevant.
- For migrations, verify the app starts with schema validation enabled when the repository uses it, and record any dialect limitation as a known gap.

## Evidence Focus

- In the evidence summary, name the Spring Data decision: repository query, transaction boundary, projection, entity lifecycle, migration alignment, dialect compatibility, auditing, or persistence proof.
