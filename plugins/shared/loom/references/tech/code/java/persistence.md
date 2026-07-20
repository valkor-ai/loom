# Java JPA And Hibernate Fundamentals

This reference owns JPA/Hibernate entity semantics independent of Spring Boot: identity, value mapping, relationships, fetch behavior, persistence context, locking, and provider interaction. Spring Data repositories, transaction proxies, Boot migration startup, and Spring test slices belong to Spring Boot data/testing references.

## When To Use

Use this reference when Java implementation work changes JPA/Hibernate entities, mappings, relationships, fetch behavior, persistence-context semantics, optimistic/pessimistic concurrency, or provider-facing lifecycle behavior. It applies whether repositories are implemented with Spring Data, another framework, or direct JPA.

Do not use it for repository API design, transaction proxy placement, migration startup, datasource configuration, or Spring test slices. Those concerns belong to the selected backend framework reference.

## Implementation Focus

### Entity Boundary

Entities are persistence models with identity and lifecycle. They are not HTTP request/response DTOs. Keep lazy proxies, audit/version columns, credentials, and internal flags out of transport serialization.

Use access style consistently. Keep constructors/factories sufficient to establish required invariants. Avoid unrestricted setters when state transitions have rules.

Equality and hash code must remain stable while an entity moves from transient to persisted state. Do not include mutable fields, collections, or lazy associations. Be careful with generated IDs before persistence.

### Value Mapping

Align mappings with the selected provider and migration:

- explicit nullability and lengths for constrained strings
- enum storage with stable external values or deliberate string mapping
- `BigDecimal` precision/scale for money-like values
- `Instant`, `OffsetDateTime`, or domain-specific time semantics with an explicit zone policy
- converters for value objects only when round-trip and query behavior are defined
- generated IDs compatible with the selected provider

Do not assume one provider's boolean, UUID, enum, timestamp, JSON, sequence, or identity behavior applies to another.

### Relationship Ownership

Define the aggregate/lifecycle owner before choosing annotations. For bidirectional relationships, helper methods must update both sides.

Use cascade only for operations the parent truly owns. `CascadeType.ALL` and `orphanRemoval` can delete or rewrite data unexpectedly when the child has an independent lifecycle. Avoid many-to-many mappings when the join relation has attributes or lifecycle behavior; model the join entity explicitly.

Keep collections initialized and avoid exposing mutable collections directly when callers could bypass invariants.

### Fetch Behavior

Associations are lazy by default unless a specific bounded access path requires otherwise. Solve access paths with projection, fetch join, entity graph, or batch fetching at the query boundary. Do not switch every relation to eager to hide initialization failures.

Multiple collection fetch joins can create cartesian multiplication. Collection fetch joins and pagination need special care because row-level pagination may not represent aggregate-level pages.

Keep Open Session in View deliberate. Business/API mapping should normally load required data inside an owned service/query boundary.

### Query Shape And Pagination

Choose the fetch plan from the read model rather than from the entity graph:

| Read need | Preferred shape | Boundary to verify |
|---|---|---|
| List or summary | DTO/projection with only required columns | deterministic ordering and bounded page size |
| Detail with one bounded association | entity graph or one fetch join | no lazy access after the service boundary |
| Detail with multiple collections | staged queries, batch fetching, or explicit read model | no row multiplication or duplicate aggregates |
| Dynamic filters | criteria/specification with an allowlisted field map | no client-controlled property or sort expression |

For paged queries, keep the data query and count query semantically aligned. Do not combine collection fetch joins with pageable results without proving aggregate-level page boundaries. Add a stable tie-breaker to ordering, and keep page size limits in the accepted API/runtime contract.

Treat query tuning as an evidence-backed change: inspect the generated SQL and provider query plan, then change the fetch shape or index contract. Do not use a global eager mapping or a cache annotation as a substitute for understanding the access path.

### Batch And Bulk Boundaries

For large writes, use a bounded batch size and flush/clear the persistence context at a deliberate boundary. The exact batching setting and identifier strategy must be compatible with the selected provider; generated identity strategies can change batching behavior.

Bulk JPQL/native updates bypass managed entities, entity callbacks, and normal dirty checking. After a bulk operation, clear or refresh affected managed state before it is read or returned, and verify the committed database result. Streaming large reads requires a transaction/resource scope that remains open for the stream and closes it deterministically.

### Cache And Measurement

Second-level or query caching is a provider-level optimization, not a default JPA feature. Enable it only when the accepted performance requirement defines cache scope, invalidation, tenant/authorization isolation, staleness, and memory bounds. Never cache mutable entities or sensitive data globally by default.

Measure before and after changes using SQL count/latency, allocation or result size, and the provider query plan. A generic AOP timer or a passing unit test is not proof that an N+1 query, a paging plan, or a cache invalidation problem was fixed.

### Persistence Context

Understand managed, detached, removed, and transient states. Bulk JPQL/native updates bypass managed entity state and callbacks. Flush timing can expose constraints before commit; tests that only inspect in-memory entities do not prove database state.

Avoid calling `save` repeatedly on already managed entities without a reason. Dirty checking persists managed changes at flush/commit.

### Concurrency

Use `@Version` when optimistic concurrency is part of the accepted data architecture. Translate stale updates into a stable conflict outcome. Pessimistic locks require bounded lock duration, ordering, and timeout behavior.

Do not use synchronization inside one JVM as a substitute for database concurrency control in a multi-instance or multi-threaded runtime.

### Lifecycle Hooks And Auditing

JPA callbacks should remain local and deterministic. Do not call repositories, remote services, or asynchronous workflows from entity callbacks. Auditing identity/time must be available in API, job, migration, and system execution paths that create records.

## Verification Focus

Useful JPA evidence includes:

- mapping/migration agreement for IDs, enums, time, money, nullability, lengths, and versions
- relationship helper and cascade/orphan behavior
- write/flush/clear/read round-trip
- lazy/fetch behavior for owned read paths
- query shape, SQL count, deterministic ordering, and page/count agreement for list paths
- bulk update visibility after clearing or refreshing the persistence context
- bounded batch flush/clear behavior for large writes
- stale-version or lock conflict behavior
- provider-compatible schema validation

## Evidence Focus

Prefer provider-backed evidence for behavior that depends on generated identifiers, dialect mappings, flush order, locking, constraints, or fetch plans. A unit test over detached objects cannot prove persistence-context or database behavior.

For mapping changes, identify the entity field, migration column, provider, and round-trip assertion. For relationship or concurrency changes, record the lifecycle operation and the exact cascade, orphan, version, lock, or conflict outcome that was verified.

## Unsafe Defaults

- Entities as API DTOs.
- Lombok-generated equality or `toString` over relationships.
- Global eager loading.
- `CascadeType.ALL` without lifecycle ownership.
- JPA callbacks that invoke external or repository work.
- Hiding provider differences behind an in-memory test database.
- Collection fetch joins combined with pageable results without aggregate-level evidence.
- Bulk updates followed by reads from stale managed entities.
- Global second-level/query caching without an invalidation and isolation contract.
