# C# Core Quality

Use this topic reference when `tech/code/csharp/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes C# application, library, domain, service, API, worker, CLI, or shared contract code.
- Use this for baseline .NET correctness: nullability, async contracts, dependency injection, result/error modeling, configuration, and public API shape.
- If the task only changes generated files, static assets, or non-C# code, do not expand scope because this reference is available.

## Implementation Focus

- Respect the target framework and language version in `.csproj` before using C# 11/12 features such as required members, list patterns, primary constructors, or collection expressions.
- Keep nullable reference types meaningful. Do not silence warnings with `!`, `#nullable disable`, or broad suppressions when validation or better types would solve the issue.
- Use `required`, init-only properties, records, or readonly structs for immutable request/domain/value shapes when they match lifecycle semantics. Do not make mutable models just to satisfy serializers without checking serializer support.
- Keep async all the way through I/O paths. Avoid `.Result`, `.Wait()`, sync-over-async wrappers, and fire-and-forget tasks outside deliberate background service ownership.
- Accept and forward `CancellationToken` through HTTP, EF, file, queue, and remote-service operations when callers can cancel or the framework supplies a token.
- Use DI constructors for services and adapters. Do not resolve scoped dependencies from the root container or store request-scoped services in singletons.
- Model expected business failures consistently with the repository style: result type, typed exception, `ProblemDetails`, or validation result. Do not mix unrelated error styles in the same flow.
- Use strongly typed options for configuration and validate required settings on startup. Avoid stringly typed config reads scattered through business code.
- Keep DTOs, domain entities, persistence entities, and UI models separate when they have different validation, serialization, or lifecycle rules.
- Add XML docs or clear comments for public APIs consumed outside the assembly when the repository exposes library surface; avoid comments that simply repeat method names.

## Verification Focus

- Run `dotnet build` and the configured `dotnet test` command for changed projects.
- Treat nullable warnings and analyzer warnings from changed code as defects unless the repository explicitly allows them.
- Add tests for domain branches, null/invalid input handling, async cancellation or error propagation where touched.
- Confirm no new blocking async calls, broad null-forgiving operators, or service lifetime violations were introduced.

## Evidence Notes

- Record `csharp.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/csharp/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the C# decision made: nullability, async/cancellation, DI lifetime, result/error style, options validation, DTO/domain split, or public API surface.
