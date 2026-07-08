# Kotlin Multiplatform Quality

## When To Use

- The task changes Kotlin Multiplatform source sets, common code, platform-specific code, `expect`/`actual`, shared clients, serialization, native interop, Gradle KMP setup, or multiplatform tests.
- Use this when code must compile or behave across JVM, Android, iOS, JS, Native, or shared modules.
- If the project is single-platform Kotlin, do not add KMP structure because this reference is available.

## Implementation Focus

- Keep platform-neutral business logic in `commonMain`. Do not import Android, JVM, iOS, JS, filesystem, or platform UI APIs from common code.
- Use `expect`/`actual` only for real platform differences such as time, filesystem, crypto, database drivers, device APIs, or platform clients. Do not split code by platform for convenience.
- Keep source set hierarchy aligned with actual targets. Do not add intermediate source sets or target dependencies unless multiple targets share implementation.
- Use shared DTOs and serialization only when all target platforms support the chosen library and configuration.
- Keep Ktor clients or other shared clients configured with platform engines in platform source sets and common request/response contracts in common code.
- Isolate native interop in platform modules and expose a small common abstraction. Do not leak Objective-C/Swift, Android, or JVM types through common APIs.
- Avoid platform-specific threading assumptions in common code. Coroutine dispatchers and lifecycle scopes should be provided by platform owners when needed.
- Keep dependency additions scoped to source sets. Do not add Android-only or JVM-only dependencies to `commonMain`.
- If publishing a KMP library, keep artifact coordinates, metadata, and version source aligned with existing release conventions.
- Treat legacy Kotlin/Native memory-model workarounds carefully; do not add obsolete freezing patterns unless the project target requires them.

## Verification Focus

- Run the common test target and every platform compile/test target touched by the task.
- If not all platform targets can run locally, run available compile tasks and record the skipped target reason.
- Add tests for common behavior and platform-specific actual implementations when they contain logic.
- Confirm no platform APIs or dependencies leaked into common source sets.

## Evidence Focus

- In the evidence summary, name the KMP decision: common/platform split, expect/actual boundary, source set hierarchy, shared serialization/client, native interop isolation, dependency scope, or platform verification.
