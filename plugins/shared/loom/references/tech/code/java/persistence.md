# Java JPA Persistence Quality

Use this topic reference when `tech/code/java/persistence.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file turns JPA/Hibernate guidance into task-level persistence rules.

## When To Use

- The task changes JPA entities, repositories, migrations, persistence configuration, transaction behavior, read models, query performance, or database-backed business rules.
- Use this for blocking Spring Data JPA/Hibernate. For R2DBC/reactive persistence, use `java.reactive` when selected.
- If the task only changes a controller DTO with no persistence behavior, do not expand into repository/entity work.

## Implementation Focus

- Treat entities as persistence models with business invariants, not API payloads. Keep API DTOs/read models separate from lazy associations, audit columns, version columns, and internal flags.
- Define ownership for relationships before adding mappings. Use helper methods for bidirectional associations so both sides stay consistent; do not add cascade or orphan removal unless the parent truly owns child lifecycle.
- Choose fetch strategy intentionally. Keep associations lazy by default, then use DTO projections, `@EntityGraph`, or fetch joins for specific read paths that need related data. Do not solve N+1 by globally switching relationships to eager.
- For list/read APIs, prefer DTO projection or dedicated read model queries. Avoid loading full aggregate graphs only to render table rows or option lists.
- Put write operations that mutate multiple records inside a service-level `@Transactional` boundary. Repository methods should not hide multi-step business workflows.
- Use `@Transactional(readOnly = true)` for query services when the project uses transaction annotations. Do not place read/write transactions randomly on controllers.
- If adding a repository query, define expected cardinality and pagination. Large or user-facing collections need `Pageable`, bounded filters, or an explicit reason they are bounded.
- When adding migrations, keep JPA mappings and SQL types aligned with the selected database. Do not rely on Hibernate auto-DDL for production behavior if the project uses Flyway/Liquibase.
- Use optimistic locking/version columns only when concurrent updates are a real risk for the entity. If added, tests should prove conflict behavior or at least mapping compatibility.
- Batch operations, second-level cache, native queries, and Criteria API are advanced tools. Use them only for a demonstrated volume, dynamic query, or performance reason, and keep evidence with the task.

## Verification Focus

- Run repository or integration tests against the configured test database. If the project uses Testcontainers, prefer the real target dialect for query/migration behavior.
- For write flows, prove write/read round-trip: create or update through service/API, then read back the state that the user or downstream code relies on.
- For repository queries, test filters, sorting, pagination/count behavior, and not-found/empty results.
- For migrations, run the migration/app startup path and confirm Hibernate validation does not fail because of mismatched column names, nullability, enum storage, or database-specific type affinity.
- For N+1 or performance fixes, include query-shape evidence when feasible: projection query, fetch join/entity graph, or a test that exercises the association path.

## Evidence Notes

- Record `java.persistence` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/java/persistence.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the persistence decision made: entity mapping, transaction boundary, projection/read model, migration alignment, query optimization, or write/read proof.
