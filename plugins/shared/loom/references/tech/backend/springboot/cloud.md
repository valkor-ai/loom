# Spring Boot Cloud And Integration Quality

Use this topic reference when `tech/backend/springboot/cloud.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`, selected Java async/security references, and selected API references when present. This file applies Spring Cloud, gateway, service discovery, WebClient, and resilience rules to task-owned integration behavior.

## When To Use

- The task changes Spring Cloud Config, service discovery, Spring Cloud Gateway, route filters, downstream HTTP clients, WebClient, load-balanced clients, circuit breakers, retries, timeouts, rate limiting, or integration error mapping.
- Use this when cross-service communication, configuration refresh, gateway routing, or external dependency failure behavior affects correctness.
- If the task only changes local controller/service/repository code with no cross-service or external dependency behavior, do not load this cloud reference.

## Implementation Focus

- Keep Spring Cloud features demand-driven. Do not introduce Config Server, Eureka, Gateway, or service registry dependencies unless the current architecture requires that runtime role.
- Externalize service URLs, discovery names, credentials, timeouts, retry counts, and gateway route predicates through typed configuration or the repository's existing config style.
- For Spring Cloud Config, define fail-fast, retry, profile, and local/test fallback behavior deliberately so local development and tests do not depend on unavailable shared infrastructure.
- For service discovery, keep registration, lookup names, health behavior, and client-side load balancing aligned with the accepted service boundary. Do not hardcode instance URLs when discovery is the chosen mechanism.
- For Gateway routes, keep predicates, path rewrites, headers, CORS, rate limiting, and fallback routes explicit and covered by tests. Do not use catch-all routing that hides ownership boundaries.
- For WebClient/downstream clients, centralize base URL/discovery name, timeout, serialization, auth propagation, and error mapping. Do not call blocking clients from reactive flows or block a reactive chain without an explicit boundary.
- Add Resilience4j/Spring Cloud CircuitBreaker policies only for current failure modes. Tune retry, timeout, circuit breaker, and fallback behavior so unsafe operations are not duplicated.
- Keep integration exceptions translated into the API or domain error model already used by the repository. Do not leak provider payloads, stack traces, or transport-only exceptions to callers.
- Keep container, registry, and cloud platform deployment assets under Loom deploy references; this file is for application integration code.

## Verification Focus

- Run focused tests for gateway route matching, rewrite behavior, headers, rate limiting, fallback paths, or CORS when those surfaces change.
- For downstream clients, test success, non-2xx, timeout, connection failure, auth propagation, and provider payload mapping with mocked HTTP servers or existing test utilities.
- For resilience policies, verify retry counts, timeout behavior, fallback result, circuit-open behavior, and that non-idempotent operations are not retried unsafely.
- For Config/Discovery changes, run context startup with local/test profile and assert missing external infrastructure fails only when the architecture requires fail-fast.

## Evidence Notes

- Record `springboot.cloud` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/springboot/cloud.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the integration decision: Spring Cloud Config, discovery, gateway route, WebClient/downstream client, load balancing, timeout, retry, circuit breaker, fallback, rate limit, or integration failure mapping.
