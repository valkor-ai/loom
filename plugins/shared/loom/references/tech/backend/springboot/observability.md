# Spring Boot Observability

This reference owns Spring Boot health, metrics, and tracing mechanics. Cross-stack event behavior lives in `tech/code/observability.md`; SLF4J provider wiring, logging configuration, async appenders, and rolling files live in `tech/backend/springboot/logging.md` when that reference is selected.

## Actuator Exposure

Expose only endpoints required by the runtime contract. Health and info are common; metrics and Prometheus require an actual collector/inspection boundary. Environment, config properties, beans, mappings, heap dumps, loggers, and thread dumps are sensitive administrative surfaces and are not public defaults.

Secure management endpoints through a separate port/network or explicit security policy when required. Do not use `show-details: always` for unauthenticated health responses.

## Liveness And Readiness

Keep probe meanings distinct:

- **liveness**: the process can continue; failure causes restart
- **readiness**: the instance can receive traffic for its required capabilities
- **capability/dependency status**: an optional or scoped capability may be unavailable without killing the whole process

Do not put slow external business calls in every health probe. A dependency health indicator must be bounded, cached or lightweight as appropriate, and consistent with whether the dependency is required at startup or per capability.

Do not mark liveness down for a recoverable downstream outage; restart loops can amplify the outage.

## Metrics

Metrics need stable names, units, and bounded tags. Good tags include outcome class, operation, dependency, or stable error category. Never tag with user id, order id, URL containing identifiers, exception message, raw query, or arbitrary tenant unless cardinality is explicitly bounded.

Use timers for latency, counters for occurrences, gauges for current bounded state, and distribution configuration only when it serves an accepted NFR or operational question.

## Tracing

Use Micrometer Observation/Tracing or the repository's established mechanism. Preserve trace context across supported HTTP, messaging, Reactor, and async boundaries. Do not manually create spans around every method.

Sampling probability is an environment decision, not a hardcoded `1.0` production default. Avoid sensitive span tags and high-cardinality business identifiers.

## Domain And Failure Signals

Emit a stable observable signal for critical state transitions and terminal failures when the architecture NFR/risk requires it. Telemetry is not the source of truth; business audit records belong in durable domain storage when required.

## Ownership And Failure Policy

The task must identify whether it owns endpoint exposure, probe behavior, metrics, tracing, or only evidence collection. Optional telemetry exporter failure must not fail business requests; required readiness dependencies must follow the accepted startup and recovery policy.

Keep Actuator configuration environment-specific and review management exposure separately from the public API surface.

## Verification Focus

Useful observability evidence includes:

- exact Actuator endpoints and access policy
- liveness/readiness behavior for required and optional dependency outages
- safe health detail exposure
- Spring request/trace context propagation across the changed boundary
- bounded metric tags and expected increments/timing
- trace propagation across async/reactive/client calls when owned
- startup without a collector when telemetry export is optional

## Unsafe Defaults

- Exposing all Actuator endpoints.
- `show-details: always` on a public endpoint.
- Calling a slow external service during every liveness probe.
- Logging request/response bodies, tokens, or credentials.
- Tagging metrics with unbounded IDs or exception messages.
- Hardcoding full trace sampling.
- Treating restart as recovery for every dependency outage.
