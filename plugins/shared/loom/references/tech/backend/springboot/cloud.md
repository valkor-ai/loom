# Spring Cloud Application Integration

This reference applies only when Technical Baseline selects Spring Cloud and the task owns Config, service discovery, Gateway, or client-side load balancing. Generic outbound HTTP clients and resilience policies have separate references.

## Capability Gate

Do not introduce a Spring Cloud component because Spring Cloud dependencies are available. Each component requires an accepted runtime or integration role:

| Component | Required Current Capability |
|---|---|
| Config Client/Server | Central configuration ownership and runtime availability contract |
| Discovery Client/Registry | Logical service lookup and registration lifecycle |
| Gateway | Public/internal route ownership, rewrite, security, and failure boundary |
| LoadBalancer | Discovery-backed client selection with defined health/retry behavior |

## Spring Cloud Config

For Config clients, define:

- application/profile/label identity
- authentication and transport security
- fail-fast versus local fallback
- retry classification and bounded attempts
- startup behavior when config is unavailable
- refreshable versus restart-required settings

Do not expose broad refresh endpoints by default. Dynamic refresh can replace bean state while requests are active; use it only for properties proven safe to refresh. Secrets and structural datasource/security changes generally require stronger rotation/restart behavior.

Config Server is a separate runtime role. Do not create one inside an application task that only needs an externalized property source.

## Service Discovery

Use logical service names only when discovery is the accepted boundary. Define registration name, metadata, health status, lease behavior, lookup failure, and local/test substitution.

Do not hardcode instance URLs alongside discovery-backed clients. Do not treat a registry entry as proof that an instance can serve the required endpoint. Registration, readiness, and client-side health semantics must agree.

Avoid adding a registry server when the selected hosting environment already provides service discovery.

## Gateway Routing

Gateway routes must preserve the accepted public API contract:

- route id and ownership
- host/path/method predicates
- exact prefix strip/rewrite behavior
- authentication and trusted header handling
- CORS boundary
- request/response size limits when required
- rate-limit policy when accepted
- downstream timeout and failure mapping

```java
@Bean
RouteLocator orderRoutes(RouteLocatorBuilder routes) {
    return routes.routes()
        .route("orders", route -> route
            .path("/api/orders/**")
            .filters(filters -> filters
                .stripPrefix(1)
                .removeRequestHeader("X-Internal-Actor"))
            .uri("lb://orders-service"))
        .build();
}
```

Treat path rewrites as contract behavior. Test the externally visible path and downstream path. Never trust caller-supplied identity headers unless a trusted gateway replaces and signs/controls them.

Gateway retries are disabled unless the accepted operation is retry-safe. Do not retry arbitrary `POST`, `PATCH`, or state-transition traffic. A fallback must not fabricate successful business data.

## Load Balancing

Client-side load balancing requires discovery-backed service instances and a health model. Preserve request timeout and retry ownership in the integration/resilience layer. Do not stack gateway retry, client retry, and library retry without an attempt budget.

## Local And Test Behavior

Provide a deterministic local/test path that does not require a shared registry or config server unless the task explicitly validates those runtime roles. Use test configuration, static service instances, or mocked discovery/config clients at the owned boundary.

## Verification Focus

Useful Spring Cloud evidence includes:

- Config startup with available and unavailable source behavior
- profile/label/property precedence and refresh-safe properties
- discovery registration and lookup failure behavior
- Gateway predicate and path rewrite tests
- security/header/CORS behavior across the gateway
- downstream unavailable mapping without false success
- confirmation that non-idempotent operations are not retried
- local/test startup without unrelated shared cloud infrastructure

## Unsafe Defaults

- Creating Config Server, Eureka, or Gateway because the dependency exists.
- Enabling discovery locator catch-all routes.
- Wildcard CORS for credentialed traffic.
- Trusting inbound identity headers.
- Retrying every route three times.
- Returning fake domain data from a fallback.
