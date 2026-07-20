# Spring Boot Data Access Implementation

This reference owns Spring Data repository behavior, Spring transaction proxies, query integration, and Boot migration startup. JPA entity semantics remain in the Java persistence reference; SQL syntax and provider behavior remain in the selected SQL/provider references.

## Spring Data Boundary

Use repository interfaces for persistence operations with clear cardinality. Keep business workflows in application/domain services.

| Need | Spring Data Shape |
|---|---|
| Lookup by unique key | `Optional<T>` plus a database uniqueness constraint |
| Existence check | `boolean existsBy...` for early feedback; still handle write races |
| Bounded collection | `Page<T>`, `Slice<T>`, or an explicitly limited projection |
| Dynamic filters | `Specification<T>` or a dedicated query component |
| List/detail read model | Interface/class/record projection with only required fields |
| Bulk update/delete | `@Modifying` query inside an explicit transaction with persistence-context behavior defined |

Derived query names are suitable for short, stable predicates. Replace unreadable method names with JPQL, Specifications, Query by Example, or a dedicated query implementation. A native query is provider-specific and requires the selected provider reference and a provider-compatible test.

## Transaction Ownership

Put transaction boundaries on public application/service methods reached through the Spring proxy.

```java
@Service
final class OrderApplicationService {
    private final OrderRepository orders;

    OrderApplicationService(OrderRepository orders) {
        this.orders = orders;
    }

    @Transactional
    public OrderResponse approve(OrderId id, Version expectedVersion) {
        Order order = orders.findById(id.value()).orElseThrow(OrderNotFound::new);
        order.approve(expectedVersion);
        return OrderResponse.from(order);
    }
}
```

Account for Spring proxy semantics:

- same-class calls do not activate a different `@Transactional` propagation mode
- private methods are not transaction entry points
- checked-exception rollback differs from unchecked-exception rollback unless configured
- `readOnly = true` is an optimization hint, not an authorization or immutability boundary
- long external calls inside a database transaction increase lock time and failure coupling

Move `REQUIRES_NEW` work to another proxied bean when an independent commit is truly required. Do not use it as a logging shortcut without defining what survives parent rollback.

External payment, messaging, email, and HTTP calls are not part of the database transaction. Define an outbox, post-commit event, compensating behavior, or explicit retry boundary when an external side effect must follow a committed write.

## Repository Query Shape

Use projections for read-heavy list/detail paths. Keep fetch plans use-case specific:

- `@EntityGraph` for a stable association set
- fetch join for a bounded detail query
- DTO projection for tables, exports, and summaries
- batch fetching only when measured and supported by the access pattern

Do not combine collection fetch joins with pageable queries without proving count and row semantics. Do not globally switch associations to eager to hide `LazyInitializationException`. Keep Open Session in View deliberate; new APIs should not depend on view-time lazy loading.

For `@Modifying` operations, decide whether `clearAutomatically` or `flushAutomatically` is required. Bulk JPQL bypasses managed entity state, callbacks, and optimistic locking unless the query enforces version behavior explicitly.

## Specifications And Filters

Specifications should compose stable predicates without accepting arbitrary property names from clients.

```java
static Specification<OrderEntity> hasStatus(OrderStatus status) {
    return status == null ? null : (root, query, cb) -> cb.equal(root.get("status"), status);
}

static Specification<OrderEntity> submittedAfter(Instant since) {
    return since == null ? null : (root, query, cb) ->
        cb.greaterThanOrEqualTo(root.get("submittedAt"), since);
}
```

Use explicit joins for relationship predicates and apply `distinct` only when join multiplicity requires it. Keep filtering, sorting, and pagination field names aligned with the accepted API and actual entity/projection fields.

## Spring Data Auditing And Events

Enable Spring Data auditing only when actor and time semantics are accepted. Supply `AuditorAware` and time through application-facing identity and clock boundaries that also work for HTTP requests, jobs, migrations, and system actions. Audit columns and nullability must remain aligned with migrations.

Repository `save` events and `@DomainEvents` run around repository lifecycle; they do not make an external message atomic with the database commit. Use a transaction-aware listener for bounded in-process follow-up or an accepted outbox/durable mechanism for external delivery. Do not publish remote side effects from entity callbacks.

Translate optimistic-lock and known constraint failures at the application boundary after preserving the original rollback behavior. JPA identity, relationship, fetch, and provider mapping semantics remain in the Java persistence reference.

## Migration Integration

When Flyway or Liquibase is selected, migrations own schema evolution. Hibernate auto-DDL must not silently modify the production schema.

Boot startup must establish a coherent order:

1. datasource properties bind
2. migration tool reaches the selected provider
3. migrations apply or validate
4. JPA mappings initialize
5. optional Hibernate schema validation compares compatible types

Keep migration SQL provider-specific through selected SQL references. Do not copy PostgreSQL `BIGSERIAL`, MySQL-specific DDL, or H2 syntax into a provider-neutral reference.

Separate schema creation, data backfill, constraint activation, and cleanup when one migration cannot safely perform all steps. Define forward repair for a partially applied or non-transactional migration.

## Concurrency And Integrity

Use database constraints as the final integrity boundary. Pre-write `existsBy...` checks improve error messages but cannot prevent concurrent duplicates.

Choose optimistic locking, pessimistic locking, serialization, idempotency, or domain rejection from the accepted data architecture. Tests should prove the selected conflict behavior. Do not catch and ignore `DataIntegrityViolationException`; translate known constraints and preserve unexpected failures.

## Verification Focus

Useful data evidence includes:

- repository cardinality and query behavior with real persisted rows
- write/read round-trip for IDs, enums, timestamps, versions, defaults, and relationships
- transaction rollback and post-commit side-effect boundaries
- duplicate, not-found, stale-version, and constraint failure behavior
- list projection, filters, sorting, pagination, and count correctness
- migration startup and mapping validation against the selected provider
- query-count or fetch-plan evidence for an N+1 correction

## Unsafe Defaults

- Returning entities as API DTOs.
- Putting `@Transactional` on controllers or private helper methods.
- Calling `REQUIRES_NEW` through same-class self-invocation.
- Holding a transaction open across an unbounded external call.
- Treating H2 compatibility as proof for another production provider.
- Enabling `ddl-auto=update` as the migration strategy.
- Adding cache, native queries, or eager relationships without a task-owned reason.
