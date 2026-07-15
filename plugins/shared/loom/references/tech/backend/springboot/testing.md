# Spring Boot Testing

Use the smallest Spring test boundary that can prove the task-owned behavior. This reference does not make every Spring task a testing task; it applies only when accepted task ownership explicitly includes test implementation.

## Test Boundary Matrix

| Behavior Under Test | Preferred Boundary | What It Proves |
|---|---|---|
| Pure domain/service rule | JUnit 5 with Mockito/fakes only when collaborators exist | Branches, state rules, collaborator contract without Spring startup |
| MVC controller/advice | `@WebMvcTest` + `MockMvc` | Routing, binding, validation, serialization, status, error translation |
| WebFlux controller | `@WebFluxTest` + `WebTestClient` | Reactive routing, body, status, error path |
| JPA repository/mapping | `@DataJpaTest` | Repository query, mapping, constraints, fetch behavior |
| Security filter/method | Web slice or focused security test | Allowed, unauthenticated, forbidden, CSRF/CORS behavior |
| Configuration properties | Binder test, context runner, or focused context | Defaults, validation, invalid/missing configuration |
| Cross-bean workflow | `@SpringBootTest` with targeted collaborators | Wiring, transaction, migration, security, runtime integration |
| Provider-specific persistence | Testcontainers or repository-standard real provider | Dialect, migrations, constraints, native queries, locking |

Do not use `@SpringBootTest` for pure calculation or mapping code. Do not mock the class under test, Spring internals, JPA entities, or value objects.

## MVC And WebFlux Slices

MVC tests should verify the accepted transport behavior, not just that a service method was called.

```java
@WebMvcTest(OrderController.class)
@Import(ApiExceptionHandler.class)
class OrderControllerTest {
    @Autowired MockMvc mvc;
    @MockitoBean OrderApplicationService orders;

    @Test
    void invalidRequestReturnsValidationProblem() throws Exception {
        mvc.perform(post("/api/orders")
                .contentType(MediaType.APPLICATION_JSON)
                .content("""{"supplierName":"","lines":[]}"""))
            .andExpect(status().isBadRequest())
            .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }
}
```

Use the mocking annotation supported by the repository's Spring Boot/Spring Framework version. Newer stacks can use `@MockitoBean`; existing projects may still use `@MockBean`. Do not rewrite the test stack solely to adopt an annotation from an example.

Import or include controller advice, converters, JSON modules, and security configuration needed by the slice. Avoid disabling all filters when the task owns protected behavior.

## Data Tests

`@DataJpaTest` often defaults to an embedded database. Keep that only for provider-neutral mapping/query behavior. Disable replacement and use the selected provider when testing:

- migration SQL
- native queries
- provider-specific column or enum types
- generated IDs and defaults
- locking, isolation, or transaction semantics
- case sensitivity, collation, JSON, array, full-text, or timestamp behavior

Use Flyway/Liquibase in the integration path when migrations are part of runtime. `ddl-auto=create-drop` does not prove migrations.

Test both repository result and persisted state. For write workflows, verify commit/readback where commit behavior matters; a test-level rollback can hide post-commit events, constraint timing, or transaction synchronization.

## Testcontainers

Reuse the repository's container lifecycle and selected provider. Spring Boot service connections are suitable when supported; `@DynamicPropertySource` remains valid for explicit property binding.

Pin a compatible provider image in project configuration rather than this reference. Do not silently replace MySQL, PostgreSQL, SQL Server, Oracle, or a file database with H2.

Container startup failure is environment evidence when the runtime is unavailable. A SQL assertion failure against a running container remains a code/test failure.

## Security Tests

Use realistic roles, authorities, CSRF tokens, and claims. Cover the changed policy through:

- allowed caller
- missing authentication
- insufficient authority
- resource-ownership denial when applicable
- stable `401`/`403` response shape

Do not test protected endpoints only with every filter disabled. Avoid production default users or credentials for test convenience.

## Configuration And Runtime Tests

Use `ApplicationContextRunner`, binder tests, or a focused `@SpringBootTest` to prove:

- `@ConfigurationProperties` defaults
- validation of missing/invalid values
- conditional bean selection
- profile-specific behavior
- absence of accidental startup dependency on external infrastructure

Fix time through an injected `Clock`. Use synchronization primitives, Awaitility, latches, virtual time, or completion signals for async behavior; do not use arbitrary sleeps as the assertion mechanism.

## Integration Boundaries

Test outbound HTTP clients with MockWebServer, WireMock, or the repository's equivalent. Prove serialization, authentication/header propagation, provider errors, timeout, and retry classification without calling a real shared service.

For cache behavior, prove key separation, hit/miss, invalidation, and fallback. For resilience, prove attempts and terminal result without waiting for production-duration timers.

## Isolation And Maintainability

- Keep fixtures close to the business scenario and avoid shared mutable global state.
- Reset external resources deterministically; do not rely on test execution order.
- Use descriptive test names based on behavior and outcome.
- Preserve Spring context caching by avoiding unnecessary per-test configuration changes.
- Add a regression test for a fixed defect when the old behavior is reproducible.
- Do not disable tests or weaken assertions to make the suite pass.

No universal coverage percentage is imposed here. Coverage is useful for finding untested paths, not as a substitute for contract, failure, persistence, and security assertions.

## Verification Focus

Useful test evidence identifies:

- the selected test boundary and why it matches the changed behavior
- exact success and blocking/failure paths covered
- provider and migration path used for persistence behavior
- Spring context or slice configuration involved
- targeted build/test command and result
- environment blockers separated from code failures

## Unsafe Defaults

- Starting the full application context for every unit.
- Using H2 as proof for provider-specific SQL.
- Disabling security filters in the only controller test.
- Relying on test rollback to prove post-commit behavior.
- Fixed sleeps for async tests.
- A hardcoded coverage target with no risk rationale.
