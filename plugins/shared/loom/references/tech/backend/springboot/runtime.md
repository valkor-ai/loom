# Spring Boot Runtime And Configuration

This reference owns application configuration, profiles, bean startup, lifecycle, and graceful shutdown. Observability, async executors, caches, external clients, and deployment assets have separate references.

## Typed Configuration

Use typed configuration for related settings and validate values that determine startup or behavior.

```java
@ConfigurationProperties("orders.client")
@Validated
public record OrderClientProperties(
    @NotNull URI baseUrl,
    @NotNull Duration connectTimeout,
    @NotNull Duration readTimeout
) {}
```

Register properties through the repository's established convention: configuration-properties scanning, explicit enablement, or auto-configuration. Keep property names stable and document units through types such as `Duration` and `DataSize`, not comments around raw integers.

Secret values belong in environment/secret providers. It is valid for `application.yml` to contain placeholders and non-secret safe defaults; it is not valid to commit credentials, signing material, or production endpoints.

## Profiles And Defaults

Profiles represent coherent environments or runtime modes, not individual feature flags. Keep local/test defaults sufficient for tasks that do not own shared infrastructure. A bean may fail fast for a required dependency only when the accepted runtime contract says the application cannot serve without it.

Avoid scattered `@Profile` and `@Value` branches that create untestable combinations. Prefer typed properties, conditional configuration, and explicit feature boundaries.

Property precedence matters. Verify command-line, environment, profile, and default configuration behavior when a task changes externally supplied values.

## Boot Conditions And Startup

Use `@ConditionalOnProperty`, classpath, or bean conditions only when optionality is real and test both outcomes. A condition should explain which capability exists when it matches and what remains available when it does not. Avoid overlapping conditions that produce multiple candidates or silently remove a required bean.

Keep Boot auto-configuration overrides narrow. Prefer an explicit application bean over excluding broad auto-configuration, and preserve repository-standard customization points. Bean construction, injection, scanning, qualifiers, and proxy mechanics remain in the Java Spring container reference.

Startup work belongs in an explicit lifecycle boundary. `CommandLineRunner` and `ApplicationRunner` must be idempotent, bounded, and failure-aware. Do not perform schema creation that conflicts with Flyway/Liquibase.

## Required And Optional Dependencies

Classify each runtime dependency:

| Dependency Class | Startup Behavior | Request Behavior |
|---|---|---|
| Required for every capability | Fail startup or readiness according to accepted runtime contract | Do not claim healthy before usable |
| Required for one capability | Start application; expose that capability as unavailable | Return actionable unavailable behavior |
| Optional enhancement | Start without it | Use explicit fallback without corrupting business meaning |

Do not catch startup exceptions and continue in a partially initialized state. Do not make local/test startup require production-only discovery, tracing, broker, or secret infrastructure when the task does not own it.

## Lifecycle And Shutdown

Use Spring lifecycle hooks deliberately. On shutdown:

- stop accepting new work before abandoning in-flight work
- stop schedulers and executors with bounded wait
- close clients and resources managed outside the container
- preserve retryable or durable work according to its ownership contract
- avoid starting new database/external work from destruction callbacks

Configure graceful server shutdown only when the runtime contract and hosting model support it. Keep timeout values externalized and testable.

## Scheduled Work

Schedulers require explicit ownership, idempotency, overlap behavior, time zone, failure visibility, and shutdown behavior. In multi-instance runtimes, define whether work may run on every instance or requires leader/distributed coordination. Do not add a distributed lock library without an accepted multi-instance requirement.

Inject `Clock` for business time. Keep cron/time-zone configuration typed and validated.

## Verification Focus

Useful runtime evidence includes:

- configuration binding with defaults and invalid values
- context startup for conditional beans and component scanning
- local/test startup without unrelated production infrastructure
- required dependency fail-fast or capability-specific unavailable behavior
- profile/property precedence where changed
- idempotent runner/scheduler behavior
- graceful shutdown and bounded work completion when owned

## Unsafe Defaults

- Scattered stringly typed `@Value` settings for one subsystem.
- Production credentials or URLs committed as values.
- A new bean that contacts an external service during construction.
- Profiles used as an uncontrolled feature-flag system.
- Startup runners that are unbounded or non-idempotent.
- Deployment assumptions embedded in application configuration.
