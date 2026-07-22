# ASP.NET Core Logging

Use this reference only when the task owns ASP.NET Core logging infrastructure. Other tasks use the configured `ILogger<T>` at their owned boundaries without replacing providers or application-wide settings.

## Provider Decision

1. Preserve existing providers, configuration, and enrichment conventions.
2. For greenfield applications, use `Microsoft.Extensions.Logging` and `ILogger<T>` with the host's built-in providers.
3. Use Serilog or NLog only when the repository already selects it or an accepted requirement needs mechanics the built-in providers do not own, such as application-managed rolling files.

Do not register competing console providers or mix direct Serilog/NLog APIs into business code. Provider-specific APIs stay in the composition root.

## Configuration Ownership

Keep category levels and provider options in `appsettings.json` plus environment-specific overrides. Bind and validate custom logging options through the existing options pattern. Never put secrets, connection strings, or production-only absolute paths in committed defaults.

Use source-generated logging methods for stable high-volume events when appropriate. Prefer named structured properties over string interpolation so scopes and providers retain field identity.

## Correlation And Boundaries

Use `Activity`, the accepted trace integration, and `ILogger.BeginScope` for request or operation correlation. Establish the scope once in middleware or the owning worker boundary and dispose it reliably. Preserve only safe immutable context across hosted services and message consumers.

Record critical state transitions, dependency outcomes, retries, terminal failures, and unexpected errors only where the task owns those decisions. Global exception handling owns the final unexpected HTTP error; lower layers do not log and rethrow the same exception.

## Async And File Output

Do not add an async queue merely because logging exists. When an accepted requirement selects buffered output, use the selected provider's bounded sink/queue and define overflow and shutdown behavior.

The built-in console provider remains the greenfield default. Application-managed rolling files require an accepted file-output requirement and an existing or deliberately selected Serilog/NLog sink with size/time limits, retention, and unavailable-path behavior. Deploy does not choose the provider or create retention policy.

## Verification Focus

- Host the application with the real provider registration and assert duplicate providers are absent.
- Capture owned events and verify level, event id/name, structured properties, scope correlation, and redaction.
- Exercise one expected failure and one unexpected failure to prove level choice and single-boundary logging.
- When buffered or file output is owned, verify overflow, shutdown flush, rotation, retention, and destination failure.

## Unsafe Defaults

- Injecting a static/global logger instead of `ILogger<T>` or the repository abstraction.
- String-interpolated events that discard structured property names.
- Serilog or NLog added when built-in providers already satisfy the contract.
- Sensitive claims, headers, payloads, or exception data in log properties.
- Unbounded buffering or rolling files without retention and disk bounds.
