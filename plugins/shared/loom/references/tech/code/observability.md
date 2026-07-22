# Application Observability

Use this reference only when the task owns a structured observability concern: request tracing, an async or scheduled flow, an external dependency boundary, a resilience decision, sensitive error handling, or an explicit `implement_observability` action. It is an application implementation reference, not a deployment or infrastructure logging contract.

## Ownership Boundary

- Keep business state and audit records in the domain/persistence boundary. Logs, metrics, and traces explain execution; they are not the source of truth for business history.
- Keep transport error shape in the selected API error reference. Log the server-side diagnostic once at the boundary that has operation and correlation context.
- Keep framework-specific provider wiring, middleware, async handlers, and file appenders in a selected framework `logging.md` reference. When no overlay exists, this file provides the ecosystem-native fallback, but the repository's established provider still takes precedence.
- Do not add a logger, collector, sidecar, container volume, or external service unless the accepted task owns that behavior and its failure policy.

## Provider Decision

Preserve the repository's existing logging abstraction, provider, and configuration first. When the task owns logging infrastructure and no selected framework `logging.md` applies, use the ecosystem-native boundary: Python `logging`, Java/Kotlin SLF4J, .NET `ILogger`, Go `slog`, a repository-established Rust `tracing`/`log` facade, a PSR-3 logger for PHP, or the existing Node logger. For a greenfield Node service that requires structured JSON, prefer Pino. Do not introduce multiple providers or make business modules depend on provider-specific APIs.

Provider selection does not imply file logging, asynchronous buffering, or an external collector. Those mechanics require an accepted operability requirement or an existing repository contract.

## Structured Events

Use stable event names and structured fields rather than interpolated prose. A useful event has:

- `timestamp`, level, service/module, operation, outcome, and stable error/event code
- request, trace, or job correlation id when the execution boundary provides one
- a bounded resource type and safe identifier when needed to diagnose the operation
- dependency, attempt, duration, or queue metadata when it explains a controlled boundary

Keep levels meaningful: expected validation, not-found, conflict, and cancellation outcomes should not be emitted as unexpected server errors. Unexpected failures should carry the exception cause and stack at one owning boundary. Do not log the same failure at controller, service, adapter, and global-handler layers.

A business implementation task adds events only for boundaries it owns: critical state transitions, external dependency outcomes, async job or consumer outcomes, retries and terminal failures, and unexpected errors at the single layer with enough context. Loading this reference does not grant ownership of global logger setup; that requires structured logging-infrastructure ownership, plus the selected framework `logging.md` when an overlay exists.

## Redaction And Cardinality

Never log passwords, access or refresh tokens, cookies, authorization headers, signing keys, connection strings, full request/response bodies, raw provider responses, or personal data unless an accepted redaction policy explicitly permits a safe field. Redaction must happen before serialization and must cover exception context, async payloads, test fixtures, metrics attributes, and trace attributes.

Use bounded values for metric names, tags, span attributes, and event dimensions. Do not use raw URLs with ids, arbitrary exception messages, user ids, order ids, or unbounded tenant values as dimensions. Prefer stable operation, outcome, dependency, and error-code values.

## Correlation And Async Boundaries

Honor the accepted request-id policy for HTTP work and preserve correlation across supported outbound, queue, scheduled, worker, and reactive boundaries. Copy only the immutable context required by the operation; never retain request-scoped objects or credentials in background work. Generate a job/operation correlation id when no request exists.

An async logger must use a bounded queue and an explicit overload policy. It must not block business requests indefinitely, silently discard security or failure events, or grow without limit. Flush or drain according to the framework lifecycle during graceful shutdown.

## File Output, Rotation, And Retention

When the application explicitly owns file logging, configure the repository's logging framework with a stable directory, format, size/time rotation, compression policy, and retention limit. The policy must bound disk use and define what happens when the directory is unavailable or the disk is full. Do not write logs into source, build output, or a path that is silently lost on the intended runtime.

Async file appenders must preserve the same redaction, ordering expectations, backpressure policy, and shutdown behavior as console or structured output. Rotation and retention are application configuration concerns; do not ask Deploy to invent Logback/Serilog settings, log volumes, or retention values.

## Verification Focus

- Assert stable event fields, level choice, correlation propagation, and one-boundary error logging for the owned flow.
- Prove secrets, sensitive payloads, exception details, and high-cardinality values are absent from logs, metrics, and traces.
- Exercise async success, failure, cancellation, retry, queue saturation, and shutdown behavior when the task owns an async logger or worker boundary.
- For file logging, verify directory/configuration, rotation trigger, compression, retention cleanup, bounded disk behavior, and unavailable-destination handling with the selected framework.
- Treat an exporter or local logging dependency outage according to the accepted operability policy; application correctness must not silently depend on telemetry availability unless explicitly required.

## Unsafe Defaults

- `print`/console concatenation for structured application events.
- Logging and rethrowing the same exception at every layer.
- Unbounded async queues, unbounded retention, or rotation without a disk-use limit.
- Request bodies, tokens, credentials, or raw exception/provider messages in telemetry.
- Treating a successful logger configuration or build as proof that correlation, redaction, rotation, and shutdown behavior work.
