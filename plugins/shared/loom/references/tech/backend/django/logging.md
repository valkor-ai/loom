# Django Logging

Use this reference only when the task owns Django logging infrastructure. Views, services, commands, and jobs otherwise use the configured logger only at their task-owned diagnostic boundaries.

## Provider Decision

1. Preserve the repository's existing Django/Python logging configuration.
2. For greenfield projects, use standard Python `logging` through Django's `LOGGING`/`dictConfig` boundary.
3. Keep structlog or another provider only when the repository already selects it or an accepted structured-event requirement justifies it.

Use module loggers and Django categories deliberately. Do not disable Django's existing logging tree wholesale or attach the same handler to parent and child categories with propagation enabled.

## Configuration Ownership

Keep formatters, filters, handlers, category levels, and destinations in settings composed from environment-specific configuration. Avoid import-time `basicConfig` and do not put secrets or production paths in base settings.

Separate access/security categories from business diagnostics. Preserve Django security logging behavior and redact sensitive request metadata before custom filters or formatters serialize it.

## Correlation And Boundaries

Establish request correlation once in middleware using safe request metadata and `contextvars` when async execution is supported. Clear context after every response. Management commands, Celery tasks, and other workers create or propagate an operation id at their own entry boundary.

Log critical transitions, dependency outcomes, async results, retries, terminal failures, and unexpected errors once. DRF/Django exception handling owns final unexpected request failures; views and services do not log and rethrow the same exception.

## Async And File Output

Use a lifecycle-owned queue listener only when buffered output is accepted. Bound the queue, define overload behavior, and stop/drain it under the actual WSGI/ASGI or worker lifecycle.

Console output is the greenfield default. File output requires an accepted application requirement plus process-safe rotation, compression, retention, disk bounds, and unavailable-path behavior. Multi-process WSGI workers must not share an unsafe rotating handler.

## Verification Focus

- Load the actual Django settings and verify handler/category propagation does not duplicate events.
- Capture a request or task event and assert stable fields, correlation, level, and redaction.
- Prove expected validation/permission outcomes are not emitted as unexpected server failures.
- When queue or file output is owned, test worker lifecycle, saturation, process assumptions, rotation, retention, and destination failure.

## Unsafe Defaults

- `disable_existing_loggers: true` without auditing Django and server categories.
- Duplicate handlers combined with propagation.
- Logging request bodies, cookies, credentials, tokens, or raw ORM/provider errors.
- Catch/log/rethrow in views, services, tasks, and exception middleware.
- File rotation configured without accounting for multiple application workers.
