# Kotlin Ktor Server Quality

Use this topic reference when `tech/code/kotlin/ktor.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Ktor routing, plugins, serialization, authentication, CORS, StatusPages, WebSockets, database integration, request validation, or server tests.
- Use this when HTTP behavior, plugin setup, request lifecycle, or server runtime contracts affect correctness.
- If Kotlin code is not a Ktor server, use core/coroutine/persistence-related references instead.

## Implementation Focus

- Keep application setup modular using the repository's existing `configureX` or route module pattern. Do not put all routing, plugin, and service code into `main`.
- Keep serialization contracts explicit. Use the existing JSON settings and DTO annotations; do not silently enable lenient or unknown-key behavior unless it matches API compatibility needs.
- Validate path/query parameters and request bodies at the route boundary. Map validation, not found, auth, and business errors into the app's standard response shape.
- Install `StatusPages` or use the existing error pipeline to prevent raw exceptions from becoming inconsistent responses.
- Configure authentication from validated configuration. Do not hard-code JWT secrets, issuers, audiences, or token expiry in route code.
- Scope protected routes with `authenticate` and keep authorization checks in server-side code, not only client logic.
- Configure CORS for actual allowed origins in production paths. `anyHost` is acceptable only for local/dev profiles that are clearly isolated.
- Keep database work off the event loop and in the repository/service boundary. Use suspended transaction patterns or dispatcher boundaries according to the selected persistence library.
- For WebSockets, own session registration/removal, ping/timeout settings, frame size expectations, and cleanup on disconnect.
- Keep route DTOs separate from persistence rows or domain internals when serialization shape, security, or validation differs.

## Verification Focus

- Use `testApplication` or the repository's Ktor test setup for changed routes/plugins.
- Test success, bad request, not found, auth failure, forbidden, and error response branches touched by the task.
- Run the Gradle test/build task for the server module.
- For WebSockets or long-lived sessions, test connect/disconnect cleanup or at least smoke the lifecycle path.

## Evidence Notes

- Record `kotlin.ktor` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/kotlin/ktor.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Ktor decision: route module, serialization, validation boundary, StatusPages mapping, auth config, CORS, database boundary, WebSocket lifecycle, or DTO separation.
