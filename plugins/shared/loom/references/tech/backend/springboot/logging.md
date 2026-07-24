# Spring Boot Logging

Use this reference only when the task owns Spring Boot logging infrastructure. Business tasks that only emit task-owned events use the configured logger and do not replace the provider or global configuration.

## Provider Decision

1. Preserve the repository's existing SLF4J provider and configuration when they satisfy the accepted contract.
2. For greenfield Spring Boot applications, use the Boot starter's SLF4J and Logback defaults.
3. Use Log4j2 or another provider only when the accepted baseline or repository already selects it. Exclude the displaced provider deliberately; never leave multiple SLF4J bindings.

Application code depends on the SLF4J API. Do not call Logback classes from business services. Lombok `@Slf4j` is acceptable only when Lombok is already an established repository dependency.

## Configuration Ownership

Keep environment-neutral levels and event shape in `application.yml`/`application.properties` or the repository's existing `logback-spring.xml`. Environment overrides own destinations and level changes. Do not hardcode production paths, credentials, or debug levels in source.

Use stable structured fields from `tech/code/observability.md`. Add a JSON encoder dependency only when structured JSON is required and no existing provider supplies it. Preserve human-readable local output when that is the repository convention.

## Correlation And Boundaries

Establish request correlation once in a filter or accepted observation/tracing integration. Put only safe identifiers in MDC and clear them reliably. For Reactor, executors, scheduled work, and messaging, use supported context propagation; do not copy arbitrary ThreadLocals.

Business code logs only its owned critical transitions, dependency outcomes, retry termination, async outcomes, and unexpected errors. A controller, service, repository, and exception handler must not all log the same failure.

## Async And File Output

Use an async appender only when the accepted operability requirement calls for it. Bound the queue, define discard/block behavior, preserve high-severity events, and flush during graceful shutdown.

When application-owned file logging is required, configure a rolling policy with an explicit directory, time and/or size trigger, compression, history, and total-size cap. Keep console-only output when file ownership is not accepted. Do not ask Deploy to generate Logback settings or invent a log volume.

## Runtime Failure Policy

- Logging must not become a hidden correctness dependency for request handling, persistence, or message consumption.
- If the appender, encoder, or destination is unavailable, follow the accepted overload policy and keep the business outcome explicit.
- Preserve error events needed for security, recovery, and incident diagnosis when applying queue limits or discard rules.
- Keep logger configuration failures visible during startup without leaking secrets or environment credentials.

## Structured Event Contract

- Use stable event names and bounded fields such as operation, outcome, error code, dependency, duration, and correlation id.
- Do not use raw request URLs, arbitrary exception messages, user input, or unbounded identifiers as metric or log dimensions.
- Redact authorization headers, cookies, tokens, passwords, connection strings, and sensitive payloads before serialization.
- Keep the event schema aligned with the repository's API error and tracing contracts so one operation can be followed across boundaries.

## Selection Boundary

The task must state whether it owns provider selection, event instrumentation, async buffering, file output, or only verification. A logging reference does not authorize changes to unrelated business modules, deployment topology, or external collectors.

Record that ownership decision in the task evidence and keep unowned logging behavior unchanged.

## Verification Focus

- Start the application and assert exactly one SLF4J provider is active.
- Capture one owned event and verify level, stable fields, correlation, and redaction.
- Prove one unexpected failure is recorded once at the selected boundary.
- When async output is owned, exercise queue pressure and shutdown flush behavior.
- When file output is owned, verify rotation, compression, retention, total-size bounds, and an unavailable directory.

## Unsafe Defaults

- Adding a second provider beside Boot's default Logback binding.
- Logging request bodies, tokens, credentials, or raw provider responses.
- Putting a catch/log/rethrow block in every layer.
- Unbounded async appenders or rolling files without a total-size limit.
- Treating Actuator exposure as a substitute for application logging behavior.
