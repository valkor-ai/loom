# NestJS Logging

Use this reference only when the task owns NestJS logging infrastructure. Controllers, providers, consumers, and jobs otherwise emit task-owned events through the configured logger without replacing application-wide logging.

## Provider Decision

1. Preserve the repository's current Nest logger provider and module wiring.
2. Use Nest's built-in `Logger` when it satisfies the accepted console and lifecycle behavior.
3. For greenfield structured JSON console output, use Pino through the established Nest integration.
4. Use Winston only when the repository already selects it or application-owned file rotation is an accepted requirement.

Do not install Pino and Winston together or call provider-specific APIs throughout feature services. Bind a provider once at bootstrap/composition boundaries.

## Configuration Ownership

Keep level, redaction, serializer, destination, and provider options in typed validated configuration. Bootstrap must fail clearly for invalid required settings without printing secrets. Preserve Nest system/startup diagnostics needed to operate the service.

Use stable event names and object fields. Avoid JavaScript string interpolation that flattens fields or serializing complete request, response, DTO, error, or user objects.

## Correlation And Boundaries

Create or accept correlation once in middleware/interceptor and propagate it with the selected request-context mechanism. Queue consumers, schedulers, WebSocket handlers, and standalone applications establish their own operation context; they cannot assume an HTTP request scope.

Log critical state changes, dependency outcomes, retries, async outcomes, and terminal failures at one task-owned boundary. Exception filters own final unexpected transport failures. Providers do not catch, log, and rethrow errors that the filter will record again.

## Async And File Output

Use Pino transports or the selected provider's async mechanism only when accepted. Bound buffering, define drop/block behavior, preserve severe events, and flush during Nest shutdown hooks.

Console JSON is the structured greenfield default. Application-owned files require an accepted requirement; use the selected provider's proven rotating transport with size/time limits, compression, retention, disk bounds, and destination failure behavior. Deploy does not select or configure this transport.

## Ownership And Failure Policy

The task must identify whether it owns provider selection, event instrumentation, async buffering, file output, or only verification. Logger or exporter failure must not silently change a business result unless the accepted contract makes telemetry a required dependency.

Keep provider lifecycle at bootstrap and shutdown boundaries. Feature modules receive the repository logger abstraction and do not configure transports independently.

## Verification Focus

- Bootstrap the real Nest application or testing module with the selected logger provider.
- Capture one owned event and assert level, stable fields, correlation, and redaction.
- Verify one unexpected exception is emitted once rather than by provider, controller, and filter.
- Exercise non-HTTP context propagation when a consumer, scheduler, or gateway is owned.
- When transport buffering or files are owned, verify saturation, shutdown flush, rotation, retention, and unavailable destinations.

## Configuration Review

- Derive provider options from validated configuration at bootstrap.
- Keep transport registration and redaction policy in one composition boundary.
- Verify HTTP, consumer, scheduler, and shutdown lifecycle assumptions separately.
- Exercise startup with missing required settings and confirm secrets are not printed.
- Keep correlation and event fields stable across provider adapters.
- Record the selected policy before changing the application-wide logger.

## Delivery Evidence

Record the selected provider, bootstrap configuration, owned event boundary, correlation and redaction assertions, and the runtime command or focused test proving the selected logger behavior. A successful Nest compile does not prove transport lifecycle or event safety.

## Unsafe Defaults

- Multiple logger providers or provider-specific calls spread through feature modules.
- Whole DTO/request/error objects passed to the logger.
- Request-scoped logger providers introduced only for correlation convenience.
- Duplicate exception logs in providers, controllers, and filters.
- Unbounded transports or rolling files without retention and disk limits.
