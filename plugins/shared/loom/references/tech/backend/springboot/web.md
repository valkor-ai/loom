# Spring Boot Web Implementation

Use the accepted HTTP interfaces as the authority for method, path, request shape, response shape, status codes, and error behavior. This reference owns their Spring MVC or WebFlux implementation, not API redesign.

## Web Stack Boundary

Confirm which web stack the selected baseline and repository use before adding code:

| Stack | Controller Shape | Client/Test Shape | Persistence Boundary |
|---|---|---|---|
| Spring MVC | Synchronous controller methods and servlet filters | `MockMvc`, `RestClient`, servlet security | Blocking repositories are allowed outside controllers |
| Spring WebFlux | `Mono`/`Flux` controller methods and reactive filters | `WebTestClient`, `WebClient` | Blocking calls require an explicit bounded scheduler; R2DBC is preferred for reactive persistence |

Do not add WebFlux to a Spring MVC application for one endpoint. Do not call `.block()` inside a reactive request chain. Do not return `Mono` merely to wrap a blocking service call.

## Controller Boundary

Controllers own transport work:

- bind path, query, header, and body input
- trigger Jakarta Validation
- resolve authenticated caller data through the established security mechanism
- call one application/service operation
- map the result to the accepted response and status

Controllers do not own repository access, transaction boundaries, state-transition rules, cross-record validation, or downstream retry loops.

```java
@RestController
@RequestMapping("/api/orders")
final class OrderController {
    private final OrderApplicationService orders;

    OrderController(OrderApplicationService orders) {
        this.orders = orders;
    }

    @PostMapping
    ResponseEntity<OrderResponse> create(@Valid @RequestBody CreateOrderRequest request) {
        OrderResponse created = orders.create(request);
        URI location = ServletUriComponentsBuilder.fromCurrentRequest()
            .path("/{id}").buildAndExpand(created.id()).toUri();
        return ResponseEntity.created(location).body(created);
    }
}
```

Preserve the real Spring Boot base package discovered from production source or build metadata. Keep controllers, advice, configuration, and application services under that package tree; never introduce tutorial roots such as `com.example` into production code.

## DTO And Validation Boundary

Use request DTOs for external input and response DTOs or read models for output. Do not serialize JPA entities, lazy proxies, internal version fields, credential fields, or bidirectional relationships.

```java
public record CreateOrderRequest(
    @NotBlank String supplierName,
    @NotEmpty List<@Valid OrderLineRequest> lines
) {}
```

Apply annotations such as `@NotBlank`, `@Size`, `@Pattern`, and nested `@Valid` to transport-shape rules. Keep uniqueness, ownership, lifecycle eligibility, authorization, inventory, and cross-record rules in the owning application/domain service. A custom validator that queries a repository does not replace a database constraint and is vulnerable to races unless the write path also handles the constraint violation.

Use validation groups only when create/update contracts truly differ and separate DTOs would create more ambiguity. Avoid annotations on JPA entities as the sole API validation contract.

## Error Translation

Use one `@RestControllerAdvice` aligned with the accepted API error contract. Spring Boot 3 supports `ProblemDetail`; preserve an existing project envelope when one already exists.

```java
@RestControllerAdvice
final class ApiExceptionHandler {
    @ExceptionHandler(OrderStateConflict.class)
    ResponseEntity<ProblemDetail> handleConflict(OrderStateConflict error) {
        ProblemDetail problem = ProblemDetail.forStatus(HttpStatus.CONFLICT);
        problem.setTitle("Order cannot change state");
        problem.setDetail(error.userMessage());
        problem.setProperty("code", error.code());
        return ResponseEntity.status(HttpStatus.CONFLICT).body(problem);
    }
}
```

Translate validation, not-found, business conflict, authentication, authorization, temporary dependency failure, and unexpected failure separately when the accepted interface declares them. Never expose SQL text, persistence exception names, token parsing failures, class names, stack traces, or file paths.

When the interface declares retryable availability responses, preserve its exact status set and emit `Retry-After` only when the application can provide a meaningful delay. This caller-visible policy does not authorize an internal retry loop; downstream retry behavior requires a separate resilience boundary.

Do not log expected validation and not-found outcomes as server errors. Log unexpected failures once at the boundary that has request correlation data.

## Collection Endpoints

For unbounded collections, use the accepted pagination/filter contract with deterministic sorting. Keep the controller default and maximum page size aligned with service/query behavior. Avoid returning Spring `Page` internals when the API contract defines a stable response model; map page content and metadata explicitly.

Validate sortable/filterable fields against an allowlist before constructing queries. Empty lists are successful collection responses unless the API contract states another behavior.

## HTTP Caching And Conditional Requests

Implement accepted HTTP cache behavior at the transport/read-model boundary. Use `Cache-Control`, `ETag`, and `Last-Modified` consistently with the interface contract, and evaluate conditional requests before serializing a full response.

For a conditional read, return `304` without a response body only when the current validator matches. For concurrency-sensitive writes, keep `If-Match`/version checks aligned with the accepted stale-update status and the service or persistence version boundary. A weak timestamp or hash that can miss meaningful state changes is not a safe validator.

HTTP validators and cache-control headers do not require Spring Cache, Redis, or an application object cache. Add `@Cacheable` only when a separate application-cache requirement owns source of truth, keys, freshness, invalidation, and failure behavior.

## CORS And Browser Binding

CORS belongs to the actual browser/runtime boundary. Use same-origin routing without CORS when frontend and API share an origin. For cross-origin clients:

- externalize allowed origins
- use explicit methods and headers
- never combine credentialed requests with wildcard origins
- keep Spring Security CORS configuration and MVC/WebFlux CORS configuration consistent
- test preflight behavior for protected write operations

Do not hardcode a single development origin as a production rule.

## HTTP Client Separation

Keep outbound HTTP clients outside controllers. `RestClient` and `WebClient` configuration, provider error translation, authentication propagation, and timeout ownership belong to the Spring Boot integration reference. WebFlux operator and backpressure behavior belongs to the Java reactive reference when selected.

## Verification Focus

Useful web evidence includes:

- a focused MVC or WebFlux test proving the accepted success status and response DTO
- validation and business-blocking responses with stable error shape
- controller discovery through the real component-scan root
- list filtering, deterministic sorting, pagination, and empty results when owned
- cache-control, validators, `304`, and stale-update behavior when declared by the interface
- CORS preflight behavior when a cross-origin browser boundary exists
- absence of entity/lazy-proxy serialization in the response path

## Unsafe Defaults

- Adding `/v1` because a tutorial uses versioned paths.
- Returning entities directly from controllers.
- Catching every exception in each controller.
- Performing repository writes or external calls in controller methods.
- Retrying outbound requests inside a controller.
- Treating HTTP validators as a reason to add an application cache.
- Enabling permissive CORS globally.
- Mixing servlet and reactive web stacks without an accepted boundary.
- Creating production controllers under `com.example` or `org.example`.
