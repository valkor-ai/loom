# FastAPI Logging

## When To Use

Use this reference only when the task owns FastAPI logging infrastructure. Route and service tasks that only emit owned events use the configured logger and do not redesign global logging.

## Provider Decision

1. Preserve the repository's existing Python logging provider and configuration.
2. For greenfield FastAPI applications, use the standard `logging` API configured through `dictConfig`.
3. Keep structlog or Loguru only when already established or explicitly required. Do not introduce a second event pipeline beside an existing provider.

## Implementation Focus

Application modules obtain named loggers with `logging.getLogger(__name__)`. Keep Uvicorn access/error integration deliberate so application errors and access events are not duplicated.

## Configuration Ownership

Create one configuration boundary in the application factory or startup module. Keep level, formatter, handler, propagation, and destination choices in structured configuration. Do not run `basicConfig` from imported feature modules.

Use stable event names and structured fields. If the selected formatter cannot preserve fields, configure an adapter/formatter at the composition root instead of concatenating JSON strings in business code.

## Correlation And Boundaries

Create or accept the request id once in ASGI middleware and store safe correlation context with `contextvars`. Reset the context in `finally`. Generate an operation id for scheduled tasks, workers, or consumers that have no request.

Log critical transitions, external outcomes, retry termination, async outcomes, and unexpected failures at one task-owned boundary. Exception handlers map public errors; services and routes must not repeatedly log the same exception.

## Async And File Output

When buffered logging is accepted, use `QueueHandler` and a lifecycle-owned `QueueListener` with a bounded queue and explicit saturation behavior. Start and stop it with the application lifespan; do not leave listener threads open in tests.

Use console output by default. Application-owned files require an accepted requirement, a stable path, process-safe handler choice, size/time rotation, compression/retention policy, and destination failure behavior. Standard rotating handlers are not automatically safe for multiple worker processes.

## Ownership And Failure Policy

The task must identify whether it owns provider setup, event instrumentation, queue buffering, file output, or only verification. Optional logging or exporter failure must not change the business result; required security and recovery events follow the accepted overload policy.

Keep handler lifecycle in the application lifespan and keep deployment, secrets, and external collectors outside this reference.

## Evidence Focus

Record the provider and configuration boundary, owned event, correlation and redaction rules, worker assumptions, and focused runtime evidence for the selected output behavior.

## Verification Focus

- Start the ASGI application through its real lifespan and capture one task-owned event.
- Verify stable fields, correlation propagation, level selection, redaction, and no duplicate Uvicorn/application error.
- Exercise concurrent requests to prove correlation values do not leak between contexts.
- When queue or file output is owned, verify saturation, listener shutdown, multiprocess assumptions, rotation, retention, and unavailable paths.

## Configuration Review

- Derive levels, handlers, and destinations from validated runtime configuration.
- Keep one authoritative configuration path for application and Uvicorn logging.
- Verify process and worker assumptions before selecting file or queue handlers.
- Exercise startup with missing required settings and confirm the failure is actionable.
- Keep redaction and correlation behavior stable across request and worker boundaries.
- Record the selected policy before changing global logging behavior.

## Unsafe Defaults

- `print`, scattered `basicConfig`, or logger configuration during module import.
- Logging entire Pydantic models, request bodies, authorization headers, or provider responses.
- Catch/log/rethrow at route, service, and global handler layers.
- Unbounded `QueueHandler` queues or leaked listener threads.
- Assuming a rotating file handler is safe across all Uvicorn worker configurations.
