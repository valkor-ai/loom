# Java Unit And Component Testing

This reference owns JUnit 5, AssertJ, Mockito, parameterized tests, deterministic fixtures, and framework-independent component tests. Spring context tests, slices, MockMvc/WebTestClient, Spring Security tests, and Boot Testcontainers integration belong to Spring Boot testing.

## When To Use

Use this reference only when the accepted task owns Java test creation or modification with JUnit, AssertJ, Mockito, parameterized tests, deterministic fixtures, or framework-independent component tests. Production implementation work does not receive testing guidance solely because the selected language is Java.

Use the Spring Boot testing reference for context loading, test slices, framework security, HTTP adapters, or provider-backed Boot integration. This file remains focused on Java test design and tools.

## Implementation Focus

### Test Shape

Structure tests around observable behavior:

- arrange only the inputs and collaborators needed by the scenario
- invoke one behavior boundary
- assert returned state, thrown error, durable effect, or collaborator contract
- name the business condition and outcome

Avoid tests that only mirror implementation lines or assert private methods.

### JUnit 5

Use lifecycle hooks sparingly and keep fixtures isolated. Parameterized tests are useful for validation tables, state transitions, parsers, and value boundaries.

```java
@ParameterizedTest
@CsvSource({"0,false", "1,true", "10,true"})
void quantityEligibility(int quantity, boolean accepted) {
    assertThat(policy.accepts(quantity)).isEqualTo(accepted);
}
```

Use `assertThrows`/AssertJ exception assertions for expected failures and verify stable domain fields, not only message prose.

### Mockito

Mock owned ports and external collaborators, not values, entities, collections, or the class under test. Prefer real small collaborators when setup is simpler than mocking.

Use strict stubbing where supported. Verify important side effects and absence of unsafe calls, but avoid asserting every internal interaction. Captors are useful when the command sent to a dependency is part of the contract.

Do not mock static/global state as a default design. Inject `Clock`, ID generators, and external ports.

### Assertions

Assert complete meaningful outcomes:

- state transition and retained invariants
- stable error code/type
- collection ordering and contents
- monetary/time values with correct comparison semantics
- no duplicate side effect on retry

Avoid assertions that only check non-null, collection size without content, or that no exception was thrown.

### Fixtures

Use builders/factories that expose scenario-relevant values and safe defaults. Keep mutable fixtures per test. Avoid giant shared JSON and hidden random data.

Fix time through an injected `Clock`. Seed randomness when randomness is part of the behavior. Generate unique data deterministically enough to diagnose failures.

### Test Levels

Plain unit tests suit domain and application logic without framework wiring. Component/integration tests suit real serializers, persistence providers, HTTP clients, messaging, and framework configuration. Select the narrowest boundary that proves the risk.

No universal coverage percentage is imposed. Use coverage to locate untested branches, then prioritize business blockers, failures, concurrency, and contracts.

## Verification Focus

Useful Java test evidence includes:

- regression scenario for the old defect
- important success and failure/business-blocking branches
- deterministic clock/ID/external collaborators
- meaningful state and side-effect assertions
- parameterized boundary cases where appropriate
- targeted and module-level test commands

## Evidence Focus

Record the test class or case that proves each owned behavior and the command that executed it. A passing module command is useful only when the relevant test is included and its assertions prove a business outcome, contract failure, state transition, or side effect.

For defect fixes, preserve a regression case that fails for the old behavior. For parameterized tests, name or display the input boundary so failures remain diagnosable. Do not treat coverage percentage, test count, or context startup alone as proof of behavior.

## Unsafe Defaults

- Booting a framework for pure Java behavior.
- Mocking the class under test or value objects.
- Shared mutable fixtures and order-dependent tests.
- Arbitrary sleeps.
- Weak non-null/no-exception assertions.
- Disabling tests or lowering assertions to pass.
