# Java Testing Quality

This file guides test selection for Java/Spring tasks; it is not a requirement to add every test type.

## When To Use

- The task changes Java behavior and needs compile/test evidence, or it adds/modifies test infrastructure.
- Use the smallest test scope that proves the behavior. Unit tests are enough for pure domain/service logic; repository/API/security/runtime wiring needs a slice or integration test.
- Follow existing test libraries and naming conventions: JUnit 5, AssertJ, Mockito, Spring test slices, Testcontainers, Gradle/Maven tasks, or local equivalents.

## Implementation Focus

- For service/domain rules, write focused JUnit tests with clear setup, action, assertion, and verification of important collaborator calls. Do not boot Spring for pure domain logic.
- For controllers, use `MockMvc`, `WebTestClient`, or existing API test style to verify request validation, status code, response shape, and error shape.
- For repositories, use `@DataJpaTest` or existing integration style. Test query filters, projections, pagination/count behavior, and empty/not-found outcomes with real persisted rows.
- For database-specific behavior, prefer Testcontainers or the project's target dialect. H2/SQLite tests are not enough when SQL type, migration, or dialect behavior is the risk.
- For security, test allowed and denied requests with realistic roles/tokens. Do not only test the happy login path.
- For reactive code, use `StepVerifier` and `WebTestClient`; do not assert `Mono`/`Flux` by blocking unless the repository has that convention for tests.
- For migrations, run the migration/startup path and assert mappings still validate. If a migration changes data shape, include at least one fixture row or write/read proof.
- Keep test data builders local and readable. Avoid huge fixture JSON or shared mutable fixtures that make business failures hard to diagnose.
- Do not mark tests disabled or relax assertions to pass CI. If a test cannot run locally, record the blocked command and reason in known gaps.
- When fixing a bug, add a regression test that fails for the old behavior whenever feasible.

## Verification Focus

- Run the project command that matches the touched scope: unit test class, module test, `./gradlew test`, `./gradlew check`, `./mvnw test`, or `./mvnw verify`.
- For behavior changes, prove at least one success path and one important failure/business-blocking path.
- For API-visible changes, prove HTTP status and response body, not only service return values.
- For persistence changes, prove write/read or query behavior against the configured test database.
- For security changes, prove both authorization success and denial.

## Evidence Focus

- In the evidence summary, name the test level used, commands run, and why that level was sufficient for the changed Java behavior.
