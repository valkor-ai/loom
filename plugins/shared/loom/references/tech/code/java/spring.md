# Java Spring Boot Service Quality

This file focuses on Spring Boot wiring and service delivery, not broad architecture reselection.

## When To Use

- The task changes Spring Boot controllers, services, configuration, starters, application properties, exception advice, actuator/runtime hooks, or dependency injection wiring.
- Use it for Spring MVC/Spring Boot service work. For reactive WebFlux chains, also load `tech/code/java/reactive.md` when selected.
- Do not create a Clean Architecture package tree just because this reference mentions boundaries. Adapt to the repository's current package layout unless the task owns project foundation.

## Implementation Focus

- Keep Spring beans cohesive. A controller should not own repository calls directly when service/use-case code already exists; a service should not parse HTTP details or return transport-only response wrappers.
- Add starters/dependencies only for behavior the task owns. Do not add OpenAPI, actuator, validation, MapStruct, Lombok, Flyway, or security dependencies just because they appear in reference examples.
- Use `@ConfigurationProperties` or the project's existing config style for grouped settings. Prefer typed properties over scattered `@Value` when multiple related settings are introduced.
- When adding `@RestControllerAdvice`, map domain, validation, not-found, conflict, and unexpected failures consistently with existing API error behavior. Do not leak stack traces or persistence messages into product errors.
- Use Jakarta validation on request DTOs for field-level constraints and keep business validation in services. Do not assume `@Valid` replaces uniqueness, authorization, state, or cross-record checks.
- Keep `open-in-view` behavior deliberate. If entities are returned to views/API to hide lazy loading problems, fix the query/read model instead.
- If adding actuator or health endpoints, expose only what the phase/runtime needs. Do not expose broad actuator details by default.
- Keep CORS local to actual frontend/runtime needs. Do not hardcode one dev origin if the project already externalizes origins.
- Preserve application startup behavior. New beans should not require unavailable environment variables, external services, or databases unless the task also provides safe local defaults or clear configuration.
- Use constructor injection and avoid circular dependencies. If a new service creates a cycle, revisit boundary placement rather than adding lazy injection.

## Verification Focus

- Run the Spring build/test target and include context startup coverage when wiring changes can break bean creation.
- For new controllers or exception advice, verify at least one success response and one validation/business error response.
- For new configuration properties, test binding or run a startup smoke with default local values.
- For runtime/actuator changes, probe the exact endpoint or startup path that consumes the new configuration.

## Evidence Focus

- In the evidence summary, state which Spring boundary was changed: controller mapping, service wiring, configuration binding, exception handling, startup/runtime behavior, or dependency setup.
