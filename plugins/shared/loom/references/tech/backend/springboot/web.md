# Spring Boot Web Quality

This file applies Spring MVC/Spring Boot web-layer rules to task-owned HTTP behavior.

## When To Use

- The task changes Spring Boot REST controllers, request/response DTOs, validation annotations, controller advice, CORS, WebClient calls, route mappings, or HTTP-facing service orchestration.
- Use this when Spring MVC request lifecycle, validation, exception mapping, or external HTTP client behavior affects correctness.
- If the task only changes internal service logic or persistence with no HTTP surface, do not load this web reference.

## Implementation Focus

- Keep controllers thin: map request input, apply `@Valid`, call an application/service method, and return a response DTO. Do not put repository queries, state transitions, or cross-record business rules directly in controller methods.
- Use request DTOs for external input and response DTOs/read models for output. Do not expose JPA entities or lazy associations through JSON serialization.
- Put field-level syntax constraints in Jakarta Validation annotations, and keep business validation such as uniqueness, ownership, eligibility, and state transitions inside services.
- Use `@RestControllerAdvice` or the repository's existing exception handler for validation, not-found, conflict, forbidden, and unexpected failures. Keep user-facing error payloads consistent with the API contract.
- Keep route paths, status codes, pagination parameters, and response shapes aligned with the accepted API contract. Do not invent versioned paths, envelopes, or OpenAPI annotations unless the task owns that API concern.
- Use `Pageable`, bounded filters, or explicit limits for list endpoints that can grow. Keep sorting deterministic.
- For WebClient or downstream HTTP calls, centralize base URL/configuration, timeouts, error mapping, and retries according to existing project style. Do not call blocking clients from reactive paths.
- Configure CORS only for the runtime/frontend boundary the task owns, and externalize origins when the project already uses configuration.

## Verification Focus

- Run `@WebMvcTest`, `MockMvc`, full integration tests, or the repository's chosen web-layer test for changed controllers and advice.
- Prove at least one success response and one validation/business error response for new or changed endpoints.
- For list endpoints, verify filtering, sorting, pagination, empty result, and stable response DTO shape when touched.
- For external HTTP clients, test success, non-2xx/error mapping, timeout behavior when configured, and no hardcoded environment values.

## Evidence Focus

- In the evidence summary, name the Spring web decision: controller boundary, DTO mapping, validation split, exception advice, pagination, CORS, WebClient behavior, or web-layer proof.
