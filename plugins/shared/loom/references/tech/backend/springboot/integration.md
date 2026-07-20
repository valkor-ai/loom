# Spring Boot External Service Integration

This reference owns external client/adapter construction, provider contract mapping, authentication propagation, failure translation, and adapter test boundaries. HTTP-specific rules apply only when the accepted interaction protocol is HTTP. Spring Cloud discovery/gateway and resilience policies are separate concerns.

## Client Choice

Use the client aligned with the application stack:

| Application Shape | Client |
|---|---|
| Spring MVC/blocking service | `RestClient` or the repository's established blocking client |
| WebFlux/reactive service | `WebClient` with non-blocking composition |
| gRPC service | Generated stub/channel wrapped behind an application-owned port |
| Existing generated/provider SDK | Wrap the SDK behind an application-owned adapter |

Do not add WebFlux to a blocking application only to obtain `WebClient`. Do not call `.block()` from a reactive request path. A blocking worker can use a blocking client with explicit connection/read timeouts.

## Typed Client Boundary

Keep provider transport outside domain/application logic.

```java
@Component
final class PricingHttpClient implements PricingGateway {
    private final RestClient client;

    PricingHttpClient(RestClient.Builder builder, PricingClientProperties properties) {
        this.client = builder.baseUrl(properties.baseUrl().toString()).build();
    }

    @Override
    public PriceQuote quote(QuoteRequest request) {
        ProviderQuote body = client.post()
            .uri("/quotes")
            .body(ProviderQuoteRequest.from(request))
            .retrieve()
            .body(ProviderQuote.class);
        return body.toDomain();
    }
}
```

Use provider DTOs separate from internal domain and public API DTOs. Keep base URL, credentials, timeouts, payload limits, and optional proxy settings in validated typed configuration.

## HTTP Behavior

For an HTTP interaction, define:

- exact relative path and method
- request/response media types and charset
- provider status-to-domain error mapping
- connection, response, and overall operation time budgets
- maximum response/body buffering
- authentication/header propagation
- correlation/request identifiers when selected
- redirect and compression behavior when relevant

Do not log full provider payloads by default. Redact credentials and sensitive fields. Reject unexpected successful empty bodies when the contract requires data.

Map provider errors into stable application exceptions such as not found, rejected, rate limited, unavailable, timeout, or invalid provider response. Preserve safe provider codes needed for support without leaking raw payloads to callers.

## Non-HTTP Provider Adapters

For generated SDKs and gRPC, keep generated/provider types at the adapter boundary. Configure channel/client lifecycle once, apply accepted deadlines and message limits, and translate provider status codes into the same application failure model used by callers. Do not expose stubs, channels, SDK sessions, or provider exceptions through domain/application interfaces.

Event and job interactions require their selected messaging/async boundary for delivery, acknowledgement, ordering, duplicate handling, and durability. Do not model them as HTTP clients merely to reuse `RestClient`/`WebClient` guidance.

## Authentication Propagation

Propagate end-user credentials only when the trust model requires delegation. Service credentials, OAuth client credentials, API keys, and signed requests require separate ownership and rotation. Do not forward every inbound header to a downstream service.

Keep token acquisition/caching in a security-aware client component, not in each business service method.

## Reactive Client Behavior

For `WebClient`, keep side effects inside the pipeline and map status before body decoding. Apply retry only through the selected resilience policy. Bound large response aggregation and use streaming only when the accepted interface supports it.

## Verification Focus

Use a protocol-appropriate fake or test server, such as MockWebServer/WireMock for HTTP, to prove:

- serialized request method/path/body/headers
- operation/message and provider status mapping for non-HTTP adapters
- successful response mapping
- provider validation/not-found/conflict response mapping
- malformed or unexpected payload behavior
- connection/response timeout behavior
- authentication and correlation propagation
- response size or streaming behavior where owned
- absence of environment-specific hardcoded values

## Unsafe Defaults

- Building a new client for each request.
- Hardcoding a provider URL or credential.
- Returning provider DTOs through the product API.
- Forwarding all inbound headers downstream.
- Retrying inside the client without the accepted operation policy.
- Swallowing provider errors into `Optional.empty()` or fake success data.
