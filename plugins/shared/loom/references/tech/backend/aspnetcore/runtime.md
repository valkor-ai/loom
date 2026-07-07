# ASP.NET Core Runtime Quality

This file applies ASP.NET Core runtime configuration, health, logging, and diagnostics rules; deployment assets remain under Loom deploy references.

## When To Use

- The task changes appsettings/options binding, environment-specific configuration, health checks, logging, metrics, OpenTelemetry, AOT/trimming readiness, middleware pipeline behavior, HTTP clients, resilience, or startup diagnostics.
- Use this when the app must start reliably across local/test/runtime environments or expose operational state needed by later tasks.
- Do not use this file for Dockerfile, Compose, Kubernetes, registry, or cloud deployment asset generation.

## Implementation Focus

- Use strongly typed options with validation for grouped settings. Keep secrets, connection strings, ports, and external URLs outside source code.
- Keep environment behavior explicit: development-only Swagger, production-safe error handling, HTTPS/CORS policies, and test-friendly defaults.
- Add health checks for dependencies the app truly needs to serve requests; keep liveness lightweight and readiness dependency-aware.
- Configure logging and tracing to diagnose failures without exposing secrets or sensitive payloads.
- Use `HttpClientFactory`, timeouts, retry/circuit-breaker policies, and cancellation tokens for external calls with current failure modes.
- Treat AOT/trimming changes as runtime behavior: verify reflection, serializers, EF/provider support, and startup path before claiming support.
- Keep middleware order deliberate: exception handling, routing, CORS, auth, authorization, and endpoint mapping must preserve behavior.

## Verification Focus

- Run app startup or integration tests for options binding, middleware, health endpoints, logging/tracing hooks, and dependency health changes.
- Probe exact health/runtime endpoints changed by the task.
- Test invalid configuration behavior when new required options are introduced.
- For external clients/resilience, verify timeout/error mapping and avoid unsafe retries for non-idempotent operations.

## Evidence Focus

- In the evidence summary, name the runtime decision: options binding, environment behavior, health check, logging/tracing, HttpClientFactory, resilience, AOT/trimming, middleware order, or startup smoke.
