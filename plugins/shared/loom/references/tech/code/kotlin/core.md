# Kotlin Core Quality

Use this topic reference when `tech/code/kotlin/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Kotlin application, library, domain, service, Android, server, or shared module code.
- Use this for baseline Kotlin correctness: null safety, data/state modeling, idioms, extension boundaries, public API shape, and interop-safe design.
- If the task only changes generated files, build metadata, or non-Kotlin code, do not expand scope because this reference is available.

## Implementation Focus

- Use Kotlin null safety as a design tool. Avoid `!!` except for documented contract violations that should fail fast; prefer `requireNotNull`, safe calls, or explicit validation.
- Model finite workflow, UI, or operation states with sealed classes/interfaces and exhaustive `when`. Avoid scattered nullable fields or booleans that permit impossible combinations.
- Use data classes for immutable data carriers, but keep mutable state and behavior in services, view models, or domain objects when lifecycle matters.
- Keep extension functions close to the type/domain they clarify. Do not add broad global extensions on common types like `String`, `List`, or `Any` unless the repository already owns that convention.
- Use scope functions intentionally: `apply` for configuration, `let` for nullable transform, `also` for side effects, `run/with` for scoped computation. Avoid chains that hide business logic.
- Use inline/value classes for strongly typed identifiers or constrained primitives only when they prevent real mix-ups and validation is centralized.
- Keep Java interop explicit where relevant: nullability annotations, platform types, checked exceptions, SAM adapters, and serialization/JPA/framework requirements.
- Use `require`, `check`, and domain results consistently according to caller expectations. Do not throw generic exceptions for normal validation failures if the app uses typed errors.
- For libraries, follow explicit API mode if configured: public declarations need deliberate visibility, return types, and KDoc where they form an external contract.
- Avoid magic companion objects, singletons, or top-level mutable state for dependencies that should be injected or lifecycle-owned.

## Verification Focus

- Run the configured Gradle build/test command for changed modules.
- Run `ktlint`, `detekt`, or repository lint tasks when configured.
- Add tests for null branches, sealed state transitions, validation failures, Java interop boundaries, and extension behavior touched by the task.
- Confirm no new undocumented `!!`, impossible state combinations, or lifecycle-free mutable singletons were introduced.

## Evidence Notes

- Record `kotlin.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/kotlin/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Kotlin decision made: null safety, sealed state, data/domain split, extension boundary, scope function use, value class, Java interop, explicit API, or lifecycle ownership.
