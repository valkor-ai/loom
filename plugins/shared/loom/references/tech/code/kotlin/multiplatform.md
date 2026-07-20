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
- Treat Ktor client and Ktor server as different ownership boundaries. A shared client owns request/response, engine, timeout, serialization, and platform networking decisions; it must not inherit server routing, server plugin, or server authentication setup.
- Isolate native interop in platform modules and expose a small common abstraction. Do not leak Objective-C/Swift, Android, or JVM types through common APIs.
- Avoid platform-specific threading assumptions in common code. Coroutine dispatchers and lifecycle scopes should be provided by platform owners when needed.
- Keep dependency additions scoped to source sets. Do not add Android-only or JVM-only dependencies to `commonMain`.
- If publishing a KMP library, keep artifact coordinates, metadata, and version source aligned with existing release conventions.
- Treat legacy Kotlin/Native memory-model workarounds carefully; do not add obsolete freezing patterns unless the project target requires them.

## Decision Rules

- Add a platform target only when the product or existing build matrix owns it. Do not broaden KMP configuration merely because the plugin makes another target easy to declare.
- Keep common code limited to APIs available to every selected target. Put HTTP engines, file systems, secure storage, clocks, UI toolkits, and native interop behind a small common abstraction with platform-owned implementations.
- Use `expect`/`actual` for a genuine platform capability, not for business branching. If behavior is shared and only construction differs, inject the dependency instead of duplicating the algorithm.
- Keep dependency declarations in the narrowest source set that can compile them. A common serialization or client dependency must be supported by every target in the target matrix.
- Treat `commonTest` as proof of shared behavior and platform tests as proof of actual implementations. A successful JVM test does not prove an iOS, Native, JS, or Android actual implementation compiles or behaves correctly.
- Keep framework and binary coordinates aligned with the repository's version catalog and release policy; do not copy version numbers from an external example.

## Verification Focus

- Run the common test target and every platform compile/test target touched by the task.
- If not all platform targets can run locally, run available compile tasks and record the skipped target reason.
- Add tests for common behavior and platform-specific actual implementations when they contain logic.
- Confirm no platform APIs or dependencies leaked into common source sets.
- Record the target matrix exercised, including compile-only targets and targets unavailable in the current environment, so the evidence does not overclaim portability.

## Evidence Focus

- In the evidence summary, name the KMP decision: common/platform split, expect/actual boundary, source set hierarchy, shared serialization/client, native interop isolation, dependency scope, or platform verification.
