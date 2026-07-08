# Spring Boot Testing Quality

This file applies Spring Boot test-slice and integration-test rules to task-owned changes.

## When To Use

- The task changes Spring controllers, services, repositories, transactions, security, configuration, messaging, reactive endpoints, or tests.
- Use this when Spring test slices, context startup, MockMvc, WebTestClient, Testcontainers, or application configuration are needed to prove behavior.
- If the task changes only pure Java code with no Spring wiring, use Java testing references without this Spring Boot testing reference.

## Implementation Focus

- Choose the smallest Spring test slice that proves the change: plain unit test for pure service logic, `@WebMvcTest` for MVC controllers/advice, `@DataJpaTest` for repositories/mappings, and `@SpringBootTest` for cross-bean/runtime wiring.
- Use `MockMvc` for MVC endpoints and `WebTestClient` for WebFlux/reactive endpoints. Do not mix blocking and reactive test clients for the same surface.
- Use Testcontainers or the repository's configured integration database when dialect, migrations, constraints, or transaction behavior matters.
- Mock external services, clocks, message buses, and HTTP clients at owned boundaries. Do not mock Spring internals or the class under test.
- Keep test data isolated with transactions, database refresh, fixtures, or builders. Avoid execution-order dependencies and shared mutable fixtures.
- Test validation, not-found, conflict, security denial, and happy paths when those behaviors are in task scope.
- Keep context-loading tests focused. A blanket `@SpringBootTest` for every class is slower and can hide missing slice coverage.
- For configuration properties, include binding/default/failure tests when a missing or invalid property would break runtime behavior.

## Verification Focus

- Run the targeted Gradle/Maven test command and the specific Spring slice/integration tests touched by the task.
- Include context startup evidence when adding beans, configuration properties, security chains, repositories, migrations, or actuator/runtime hooks.
- For persistence changes, assert database state rather than only service return values.
- For security changes, test allowed, unauthenticated, and unauthorized cases.

## Evidence Focus

- In the evidence summary, name the proof type: unit test, `@WebMvcTest`, `@DataJpaTest`, `@SpringBootTest`, Testcontainers, MockMvc, WebTestClient, security test, or configuration binding proof.
