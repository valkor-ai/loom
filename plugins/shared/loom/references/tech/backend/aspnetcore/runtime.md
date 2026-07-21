# ASP.NET Core Runtime And Hosting

This reference owns application runtime configuration, hosting, middleware, dependency health, outbound clients, background services, and diagnostics. Docker, Compose, Kubernetes, proxy topology, registry, and deployment asset generation remain in Loom deploy guidance.

## Configuration And Options

Use the existing configuration precedence across base settings, environment settings, environment variables, user secrets/local overrides, and external providers. Never put production secrets or environment-specific URLs in committed defaults.

Bind cohesive settings to typed options and validate required values at startup:

```csharp
builder.Services
    .AddOptions<ProviderOptions>()
    .BindConfiguration(ProviderOptions.SectionName)
    .ValidateDataAnnotations()
    .Validate(options => Uri.TryCreate(options.BaseUrl, UriKind.Absolute, out _),
        "Provider BaseUrl must be absolute")
    .ValidateOnStart();
```

Use `IOptions<T>` for stable settings, `IOptionsSnapshot<T>` for scoped reload semantics, and `IOptionsMonitor<T>` only when live updates are supported safely. Do not read scattered configuration keys in business methods.

Keep local defaults runnable and production defaults safe. A missing mandatory dependency should fail startup clearly; an optional capability should expose an explicit disabled/degraded state.

## Middleware And Host Pipeline

Keep exception handling before response-producing middleware and preserve the repository's routing, forwarded headers, HTTPS, static files, CORS, authentication, authorization, rate limiting, caching, and endpoint order.

Trust forwarded headers only from configured proxies/networks. Do not force HTTPS redirects inside a topology where TLS termination and forwarded proto are not configured correctly.

Use graceful shutdown and propagate `ApplicationStopping`/cancellation to hosted work. Configure request/body limits, timeouts, and server endpoints from accepted runtime requirements, not tutorial constants.

## Health And Readiness

Separate lightweight process liveness from dependency-aware readiness when the runtime platform consumes both. Liveness should not fail because a database or provider is temporarily unavailable; repeated restarts can worsen an outage.

Tag readiness checks and include only dependencies required to serve the advertised capability. Bound health-check timeouts and avoid heavy business queries, migrations, writes, or fan-out calls.

Map health paths and response detail according to the runtime contract. Do not expose credentials, internal hostnames, exception text, or dependency topology publicly.

## Outbound HTTP And Resilience

Use `IHttpClientFactory`/typed clients with validated base address, bounded timeout, authentication handler, serialization, and provider error translation. Propagate cancellation and dispose response streams correctly.

Apply retry, circuit breaking, hedging, and timeout policies only when the accepted interaction owns them. Retry only transient/idempotent operations or use an explicit idempotency mechanism. Keep total timeout budget and retry count bounded; do not stack client, library, proxy, and application retries blindly.

Refresh tokens/credentials through a concurrency-safe handler/provider and redact headers/bodies from logs. DNS/handler lifetime should follow platform behavior rather than manually creating `HttpClient` per request.

Service discovery or gateway routing must follow the accepted runtime architecture. Resolve logical service names through the selected platform/client configuration and keep browser/public paths distinct from internal service addresses. Do not hardcode Compose or cluster hostnames in application code.

## Application Caching

Add `IMemoryCache`, `IDistributedCache`, HybridCache, or a provider adapter only when an application-cache requirement owns source of truth, key dimensions, TTL/freshness, invalidation, consistency, size, and failure behavior.

Include tenant/authorization/locale/version dimensions in keys when they affect values. Never cache credentials, unsafe mutations, or user-specific data under shared keys. Distributed-cache loss should follow the accepted degradation policy; it must not become a second source of truth.

HTTP output caching is a separate transport concern. Do not add Redis merely because an endpoint has `Cache-Control` or ETag semantics.

## Background Services

Implement `BackgroundService`/`IHostedService` only for accepted jobs, consumers, or maintenance work. Create service scopes per iteration/message when scoped dependencies are needed.

Honor `stoppingToken`, handle partial failures, and define retry/dead-letter/idempotency/concurrency behavior. Avoid unbounded in-memory queues and arbitrary `Task.Delay` loops when a scheduler or broker owns timing.

Startup must not launch duplicate workers under test, design-time migrations, or hot reload. Shutdown should stop intake, finish/cancel bounded work, and close clients cleanly.

## Framework Instrumentation Boundary

Use `ILogger`, `ActivitySource`, `Meter`, or the repository's OpenTelemetry setup as the ASP.NET adapter for the selected application observability contract. The cross-stack reference owns event fields, levels, correlation, redaction, cardinality, async logging, file rotation, and retention. Keep exporter failure behavior aligned with the accepted operability model and do not block business requests indefinitely.

## AOT, Trimming, And Serialization

Enable native AOT/trimming only when the selected runtime and dependencies support it. Verify reflection-heavy serializers, DI scanning, validators, EF provider behavior, OpenAPI generation, dynamic proxies, and configuration binding.

Use source-generated serialization/metadata where required and run the published artifact, not only `dotnet build`, before claiming AOT/trimming support.

## Verification

- Start the application with valid and invalid required options and assert the intended startup outcome.
- Exercise middleware ordering, forwarded-header, CORS/auth, limits, and exception behavior through the real host when changed.
- Probe liveness/readiness and dependency transitions without exposing internals.
- Test outbound timeout, cancellation, transient mapping, and retry/idempotency boundaries with a controllable provider.
- Verify cache key isolation, invalidation/freshness, miss/degraded behavior, and source-of-truth recovery when caching is owned.
- Verify background service scope, shutdown, duplicate prevention, and failure handling.
- Validate structured telemetry fields and run the published artifact for AOT/trimming claims.

## Delivery Evidence

Identify the runtime boundary and the startup/host/probe/client/worker assertion proving it. A configuration file, registered health check, or successful build cannot prove precedence, middleware order, dependency transitions, cancellation, shutdown, or published-runtime compatibility.

## Unsafe Defaults

- Docker/Kubernetes assets duplicated in application runtime guidance.
- Secrets or production URLs committed to `appsettings.json`.
- Dependency checks included in liveness.
- `HttpClient` created per request or retries added to non-idempotent writes.
- Hosted services ignoring cancellation or resolving scoped services from the root provider.
- Sensitive/high-cardinality data emitted as telemetry labels.
- AOT/trimming claimed from compilation without running the published artifact.
