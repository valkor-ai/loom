# C# ASP.NET Core Quality

Use this topic reference when `tech/code/csharp/aspnet.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes ASP.NET Core Minimal APIs, controllers, route groups, middleware, authentication, authorization, validation filters, health checks, caching, rate limiting, or host configuration.
- Use this when HTTP behavior, request lifecycle, middleware order, DI, or API error shape affects correctness.
- If the task only changes internal domain code with no HTTP surface impact, use core or persistence references instead.

## Implementation Focus

- Follow the existing endpoint style: Minimal API route groups, controllers, or vertical slices. Do not mix controller and Minimal API conventions in one area without a repository precedent.
- Keep routes resource-oriented and stable. Use route groups for shared prefixes, tags, auth policies, filters, and version/path conventions already present in the app.
- Validate request bodies, route values, query params, and headers at the HTTP boundary. Return the repository's standard validation/problem response rather than leaking exceptions or raw validator details.
- Forward `CancellationToken` from endpoints into services, EF queries, HTTP clients, and file/queue operations.
- Keep middleware order deliberate: exception handling early, routing/auth/cors/rate limiting/caching in the order expected by ASP.NET Core and the existing app.
- Use the options pattern for JWT, CORS, downstream service URLs, limits, and feature flags. Validate required options at startup when a missing value would break requests.
- Use authorization policies for business permissions. Do not rely on UI hiding or controller code branches as the only access control.
- Keep global exception handling mapped to `ProblemDetails` or the app's equivalent. Do not expose stack traces or internal messages outside development.
- Apply output caching, response compression, or rate limiting only when the endpoint semantics support it. Do not cache user-specific, unsafe, or mutation responses by accident.
- Health checks should reflect runtime dependencies the app actually needs and should not perform heavy business work.

## Verification Focus

- Add or run `WebApplicationFactory`, `TestServer`, or endpoint integration tests for changed routes.
- Test success status, validation failure, not found, auth/forbidden, and exception/problem response branches touched by the task.
- Run `dotnet test` and a startup/config smoke when middleware, options, auth, or health checks changed.
- Confirm generated OpenAPI metadata or endpoint names remain consistent when the repository uses them.

## Evidence Notes

- Record `csharp.aspnet` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/csharp/aspnet.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the ASP.NET decision: endpoint style, validation boundary, middleware order, auth policy, options validation, problem response, caching/rate limiting, or health check.
