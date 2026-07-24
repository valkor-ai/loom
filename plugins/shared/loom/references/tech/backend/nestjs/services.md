# NestJS Services, Modules, And Dependency Injection

Nest providers implement application behavior and connect explicit module dependencies. Preserve the repository's domain and adapter boundaries instead of turning every class into a generic service or every module into a shared container.

## Service Responsibility

Keep business rules, state transitions, transaction orchestration, repository calls, and external-port coordination outside controllers. A service method should represent an application operation with explicit inputs, outputs, failure modes, and side-effect order.

Do not couple application behavior to HTTP DTOs or Nest exceptions when the repository already separates domain/application layers. Translate transport models at the controller boundary and map domain failures through the established exception layer.

Split providers when they own distinct lifecycles or dependencies, not simply because a file grows. Avoid pass-through services that add no policy and duplicate an existing repository/client abstraction.

## Module Boundaries

Feature modules own cohesive controllers and providers. Import modules that export required capabilities and export only providers intentionally consumed elsewhere.

Avoid global modules for ordinary feature dependencies. They hide ownership and make tests compile accidentally. Detect circular dependencies as a boundary problem; `forwardRef` is a temporary escape hatch only when both directions are genuinely unavoidable and documented.

Dynamic modules fit configurable infrastructure adapters or reusable platform capabilities. Keep `forRoot`/`forRootAsync` configuration typed, validated, and separate from per-feature registration.

## Provider Tokens And Factories

Use constructor injection. Class tokens are suitable for concrete internal providers; symbols or stable constants are safer for ports and non-class values than free-form strings.

Factory providers must declare every dependency in `inject`, validate configuration before constructing clients, and define lifecycle/cleanup behavior for connections or workers. Do not read scattered `process.env` values inside feature methods.

Use `useExisting` for aliases, `useClass` for replaceable implementations, `useValue` for stable values/test doubles, and `useFactory` for construction that depends on configuration or other providers. Keep the choice visible in the owning module.

```typescript
export const ORDER_REPOSITORY = Symbol("ORDER_REPOSITORY");

@Module({
  providers: [
    OrdersService,
    { provide: ORDER_REPOSITORY, useClass: TypeOrmOrderRepository },
  ],
  exports: [OrdersService],
})
export class OrdersModule {}
```

The adapter class and token should follow the selected persistence stack and existing naming; the example demonstrates the port boundary, not a requirement to introduce TypeORM.

## Provider Scope And Context

Singleton is the default and should not retain request-specific mutable state. Request scope propagates through dependency graphs and can add allocation/latency; use it only when context cannot be passed explicitly or provided through an established context boundary.

Transient providers create a new instance per injection and are not a general fix for shared-state bugs. Verify scope interactions for gateways, scheduled jobs, queue consumers, and durable workers where no HTTP request context exists.

## Persistence And Transactions

Follow the selected persistence adapter (TypeORM, Prisma, MikroORM, repository port, or another established stack). Do not introduce an ORM from NestJS examples.

Keep multi-write invariants inside one explicit transaction boundary. Pass a transaction-aware repository/client through the operation rather than mixing transactional and global clients. Translate known unique/constraint/concurrency failures without matching opaque provider strings throughout business code.

Do not hold database transactions open across email, HTTP, queue, or file operations. For durable cross-boundary effects, follow the accepted outbox, job, or compensation design.

## External Effects And Failure Mapping

Inject external clients behind the repository's adapter/port convention. Define timeout, cancellation, retry, idempotency, and partial-failure behavior from accepted architecture decisions; do not add retries around non-idempotent writes by habit.

Order durable state and external effects deliberately. The selected application observability reference owns the single correlation-aware logging boundary; do not log and rethrow the same exception at every provider layer.

Use typed domain/application failures or focused Nest exceptions according to the existing layering. Never expose raw ORM/client errors, secrets, or stack traces.

## Lifecycle And Background Providers

Use Nest lifecycle hooks only for owned startup/shutdown behavior. Startup should fail clearly for mandatory dependencies and should not launch duplicate timers/workers during tests or hot reload.

Close clients, consumers, and application resources in shutdown hooks. Scheduled or queue-driven providers require idempotent handlers and explicit concurrency/error behavior because request-scoped assumptions do not apply.

## Verification

- Compile the owning `TestingModule` with real provider tokens and module imports where wiring changed.
- Unit-test application decisions with typed mocks/fakes at repository and external-port boundaries.
- Prove not-found, conflict, authorization, transaction rollback, and external-failure behavior owned by the operation.
- Exercise the selected persistence provider for transaction, constraint, or query behavior that mocks cannot prove.
- Verify provider scope, dynamic-module configuration, lifecycle cleanup, and absence of open handles when changed.
- Run focused lint/typecheck/build for module metadata and injection-token errors.

## Delivery Evidence

Name the service/provider operation, its injected boundaries, and the assertion proving the business or transaction outcome. A compiled module proves DI wiring, not state transitions, persistence durability, or external-effect ordering.

## Unsafe Defaults

- Manual `new Service()` construction inside application code.
- Feature providers exported or marked global without a consumer contract.
- `forwardRef` used instead of resolving a cyclic ownership model.
- Request-scoped providers introduced for convenience.
- ORM choice copied from an example rather than the accepted stack.
- External calls performed inside a database transaction.
- Broad catch/log/rethrow blocks at every provider layer.
