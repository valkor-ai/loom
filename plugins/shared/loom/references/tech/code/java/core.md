# Java Core Implementation Quality

Use this topic reference when `tech/code/java/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file is not a Java style guide; it tells the agent how to make Java implementation decisions inside a Loom task.

## When To Use

- The task changes Java domain, service, DTO, validation, exception, configuration, or controller-adjacent code.
- The project is regular Spring MVC, Spring Boot service code, or Java backend logic that is not specifically reactive, persistence-only, or security-only.
- Use repository conventions first. Introduce records, sealed classes, builders, Lombok, MapStruct, or package layering only when the existing project already supports them or the task explicitly owns that foundation.

## Implementation Focus

- Keep controller code transport-oriented. Controllers should parse path/query/body, trigger validation, call a service/use case, and map the result. Put business state transitions, eligibility checks, and cross-entity invariants in service/domain code.
- Do not expose JPA entities directly through API responses. Use request/response DTOs or records at transport boundaries, especially when entities have lazy associations, internal flags, audit fields, or persistence-only identifiers.
- Put input validation at the earliest owned boundary. Use Jakarta Bean Validation for simple shape constraints, and explicit service/domain validation for business rules that require repository reads or state checks.
- Model business failures with the project's existing error contract. If none exists and this task owns API behavior, use a small exception hierarchy plus `ProblemDetail`/error response mapping; do not throw generic `RuntimeException` with user-facing messages.
- Use Java 21 features only when the build tool and source compatibility already allow them. Prefer records for immutable DTOs and sealed hierarchies for closed domain states, but do not rewrite mutable framework-bound entities into records.
- Externalize runtime values through `application.yml`, environment variables, or typed configuration properties. Do not hardcode URLs, ports, credentials, feature flags, date cutoffs, or filesystem paths in services.
- Keep mapping logic explicit enough to inspect. For small DTOs, hand-written mapping is fine; for repeated mapping, follow the repository's mapper convention rather than introducing a new mapper library.
- Preserve transaction ownership. A service method that changes business state should own the transaction boundary or call an existing transactional use case; do not spread writes across controller code.
- Prefer constructor injection and final dependencies. Do not add field injection or static service lookups.
- When adding IDs or status strings, use domain-specific types/enums where the project already uses them; avoid free-form strings for state machines or business categories.

## Verification Focus

- Run the repository's Java build/test command: `./gradlew test`, `./gradlew check`, `./mvnw test`, or `./mvnw verify` depending on the project.
- Add service/domain tests for new business rules, including at least one business-blocking or validation failure path.
- If controller-visible behavior changes, add a controller/API test or runtime probe that proves status code, response body, and error shape.
- If configuration is added, verify the default local value and one override path when feasible.
- If the change uses Java 21 language features, the compile step must prove the configured source/target supports them.

## Evidence Notes

- Record `java.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/java/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Java boundary that was kept clean, such as `controller -> service -> repository`, DTO/entity separation, validation ownership, or exception mapping.
