# ASP.NET Core Application Architecture

Implement the architecture style and module boundaries accepted in the architecture contract. Clean Architecture, vertical slices, CQRS, and MediatR are options, not framework defaults; preserve the repository's established shape unless the task owns a structural decision.

## Dependency Direction

For a layered architecture, dependencies point toward application/domain policy:

| Boundary | Owns | Must not own |
|---|---|---|
| Domain | entities, value objects, invariants, domain failures | ASP.NET, EF Core, transport DTOs, external clients |
| Application | use cases, ports, orchestration, transaction intent | HTTP binding, provider configuration |
| Infrastructure | EF/client/message/file adapters | product policy hidden in adapters |
| Web/API | transport binding, identity context, result mapping | persistence queries and business workflows |

Project references and namespaces should enforce the accepted direction. A folder named `Domain` inside the web project does not create an architectural boundary by itself.

For a modular monolith or vertical-slice design, keep capability ownership equally explicit: one slice/module owns its commands, queries, rules, storage adapter, and public contract. Shared projects should contain stable cross-cutting primitives, not miscellaneous code imported by every feature.

## Use-Case Boundary

Represent each mutating or query operation with an application service/handler whose input, output, cancellation, authorization context, failures, and transaction responsibility are visible.

```csharp
public sealed record ApproveOrder(Guid OrderId, Guid ActorId);

public sealed class ApproveOrderHandler(
    IOrderRepository orders,
    IUnitOfWork unitOfWork,
    IClock clock)
{
    public async Task<OrderResult> Handle(ApproveOrder command, CancellationToken ct)
    {
        var order = await orders.GetForUpdate(command.OrderId, ct)
            ?? throw new OrderNotFound(command.OrderId);
        order.Approve(command.ActorId, clock.UtcNow);
        await unitOfWork.SaveChangesAsync(ct);
        return OrderResult.From(order);
    }
}
```

The example shows an explicit application boundary, not a requirement to create repository/unit-of-work wrappers around EF Core when the accepted design uses `DbContext` directly.

## CQRS And MediatR

Use command/query separation when read and write responsibilities, authorization, validation, transactions, or scaling genuinely differ. Do not duplicate identical DTOs/handlers for ceremonial CQRS.

MediatR is useful only when the repository selects it and pipeline behaviors remove real cross-cutting duplication. Register handlers from the correct assemblies and keep behavior order deliberate. Validation, authorization, transaction, idempotency, and logging behaviors must not each execute the handler or hide side effects.

Do not put EF queries in every endpoint simply because query handlers are thin. Keep query ownership aligned with the selected application/data boundary and project directly to read models.

## Dependency Injection And Composition

Register application services and adapter implementations in explicit composition extensions close to their owning projects. Use scoped lifetimes for request/unit-of-work services, singleton only for thread-safe stateless/shared resources, and transient for cheap independent components.

Avoid service locator access through `IServiceProvider` in business code. Factories are appropriate when runtime selection is part of the accepted design; inject narrow factory interfaces instead of the container.

Validate required options during startup and keep infrastructure details out of domain/application constructors. Prevent circular project references rather than masking them with shared utility assemblies.

## Validation And Failure Boundaries

Transport validation handles malformed HTTP input. Application validation handles use-case preconditions and authorization. Domain objects enforce invariants that must hold across entry points. Database constraints remain the final concurrent integrity boundary.

Use typed domain/application failures and translate them once at the API boundary. Do not make domain exceptions inherit ASP.NET HTTP exception types or leak EF/provider exceptions through handler responses.

## Transactions And Side Effects

One use case owns the transaction for its persisted invariants. Repositories should not silently commit independently. Keep HTTP/email/queue/file calls outside database transactions unless the accepted design explicitly coordinates them.

For durable events, use the accepted outbox or messaging boundary and make handlers idempotent where duplicate delivery is possible. In-process MediatR notifications are not a durable event bus.

## Verification

- Build the affected project graph to catch illegal/missing references and DI registration errors.
- Test domain invariants without ASP.NET or EF infrastructure.
- Test handlers/services for authorization, state transition, transaction, cancellation, and failure mapping.
- Compile a production-representative service provider when registrations or pipeline behaviors change.
- Add architecture dependency tests only when the repository uses them and the decision is important enough to enforce mechanically.
- Exercise HTTP mapping separately when endpoints are also task-owned.

## Delivery Evidence

Identify the accepted decision, owner module/project, changed dependency direction, and the test/build evidence proving it. A namespace layout, generated project tree, or passing endpoint alone does not prove architecture boundaries or transaction ownership.

## Unsafe Defaults

- Clean Architecture or MediatR introduced without an accepted architecture decision.
- Domain types depending on ASP.NET, EF Core, or transport DTOs.
- Shared projects becoming a dumping ground for feature logic.
- One handler class per trivial getter with no meaningful separation.
- `IServiceProvider` used as a service locator.
- Repositories committing independently inside one use case.
- In-process notifications claimed as durable integration events.
