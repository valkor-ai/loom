# Java Spring Container Fundamentals

This reference owns Spring container mechanics shared across Spring applications: bean construction, dependency injection, proxies, component scanning, package roots, and lifecycle. Spring Boot web, data, security, runtime, testing, cache, async, integration, and observability behavior belongs to the selected backend references.

## When To Use

Use this reference when Java implementation work owns Spring bean construction, dependency injection, component discovery, proxy-backed annotations, configuration classes, or managed lifecycle. It remains applicable across Spring Framework and Spring Boot applications.

Do not use it as a substitute for framework-specific transport, persistence, security, testing, runtime, cache, messaging, or integration guidance. Those concerns require their task-owned backend reference.

## Implementation Focus

### Package And Scan Boundary

Keep the application/bootstrap class at or above owned component packages. Use the existing production package root or build metadata. For a new repository, use the confirmed namespace, `app.<project_slug>`, or `app.generated`; never create production code under tutorial roots.

Avoid broad `scanBasePackages` or repository/entity scan overrides that hide an incoherent package layout. A new module should remain discoverable through the normal package tree unless it is intentionally imported as configuration.

### Constructor Injection

Use constructor injection for required dependencies. It makes invariants visible, supports immutable fields, and permits plain unit construction.

```java
@Service
final class OrderPolicy {
    private final Clock clock;
    private final EligibilityRules rules;

    OrderPolicy(Clock clock, EligibilityRules rules) {
        this.clock = clock;
        this.rules = rules;
    }
}
```

Avoid field injection, static context access, and service-locator lookups. Do not add `@Autowired` to a single constructor unless the repository convention requires it for clarity or tooling.

Use `@Qualifier` when multiple beans intentionally implement one contract. Prefer a typed custom qualifier for a long-lived boundary over repeating fragile string names.

### Bean Ownership

Use stereotypes to communicate role, not as decoration:

- `@Service` for application/domain-facing operations
- `@Repository` for persistence adapters and exception translation
- `@Controller`/`@RestController` for transport adapters
- `@Configuration` for bean assembly
- plain Java classes for values, domain behavior, and helpers that do not need container services

Do not turn every class into a bean. Keep framework-independent logic constructible without a Spring context.

### Proxy Semantics

Spring annotations such as transactions, method security, caching, and async execution commonly rely on proxies. The call must cross the proxy:

- same-class self-invocation bypasses advice
- private/final methods may not be valid advised entry points depending on proxy mode
- objects created with `new` are not container-managed
- annotation order/composition can alter transaction, cache, and security timing

Put advised operations on clear public boundaries and test the framework behavior through the owning Spring Boot reference. Do not add self-injection to force proxy traversal.

### Bean Lifecycle

Keep constructors deterministic and free of I/O. Use explicit initialization only for bounded validation/preparation. Startup calls to databases or external services must follow the runtime dependency contract.

Own resources through the container and close them through supported lifecycle callbacks. Destruction hooks should release resources, not start business workflows.

Avoid circular dependencies. Repair responsibility placement or introduce an explicit contract rather than using `@Lazy` as a structural fix.

### Configuration Classes

Keep configuration classes cohesive. Bean methods should assemble collaborators, not contain business decisions. Conditional beans need an explicit fallback/absence model and tests for each supported condition.

Do not add starters, auto-configurations, Lombok, mapping libraries, validation, security, actuator, or migration dependencies because an example mentions them. Dependencies follow task-owned behavior and the selected baseline.

## Verification Focus

Useful container evidence includes:

- plain unit construction for framework-independent logic
- focused context startup for component scanning and bean selection
- correct `@Qualifier` resolution when multiple implementations exist
- absence of circular dependencies and hidden service-locator access
- proof that an advised call crosses the Spring proxy when relevant
- package layout under the real production root

## Evidence Focus

Record the narrow artifact that proves the owned container behavior: a focused context test for discovery or bean selection, a plain unit test for framework-independent construction, or an integration assertion that advice is applied through the managed proxy. Build success alone does not prove scanning, qualifier selection, lifecycle order, or proxy traversal.

When the implementation changes package roots or configuration imports, include the affected production package and bootstrap configuration in review evidence. When it changes an advised boundary, identify the externally invoked method and the behavior supplied by the advice.

## Unsafe Defaults

- Field injection.
- Broad component scanning to compensate for a misplaced application class.
- `@Lazy` as a circular-dependency fix.
- Same-class calls expected to trigger transactional, async, cache, or security advice.
- Making pure domain/value classes container-managed without a dependency reason.
