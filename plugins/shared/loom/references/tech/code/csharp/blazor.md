# C# Blazor Quality

## When To Use

- The task changes Blazor Server, WebAssembly, components, forms, validation, cascading state, routing, authorization UI, JS interop, SignalR, virtualization, or component lifecycle behavior.
- Use this when UI state, component parameters, browser/runtime interop, or render lifecycle affects correctness.
- If the frontend is React/Vue/Svelte or non-Blazor, use the relevant JavaScript/TypeScript/UI references instead.

## Implementation Focus

- Keep component parameters explicit. Use `[EditorRequired]` for required parameters and `EventCallback<T>` for typed parent-child events.
- Separate editable form models from server DTOs or EF entities when validation, partial input, or UI-only fields differ from persistence/API contracts.
- Use `EditForm` with the repository's validation approach and render field-level/business feedback. Do not rely only on disabled buttons or client-side hints.
- Load data in the lifecycle method that matches the trigger: initialization, parameter changes, or first render for JS interop. Avoid repeated fetches caused by unnecessary rerenders.
- Dispose subscriptions, timers, SignalR handlers, JS modules, object references, and event callbacks through `IDisposable` or `IAsyncDisposable`.
- Keep cascading state small and stable. Do not use global cascading objects as a substitute for proper service/state ownership.
- For JS interop, isolate module import and object reference ownership inside the component or service that disposes it. Do not leave stale browser handles after navigation.
- Use `ErrorBoundary` or the app's error pattern for recoverable component failures. Avoid exposing raw exception messages to normal users.
- Use `Virtualize` or server paging for large lists. Do not render unbounded collections in components that can receive many rows.
- Preserve authentication/authorization semantics in routes and server/API enforcement; conditional UI visibility is not sufficient access control.

## Verification Focus

- Run the app build and the configured component/unit/integration tests for changed Blazor code.
- Test loading, empty, validation failure, submit success, submit failure, unauthorized/forbidden, and disposal paths touched by the task.
- Verify JS interop and SignalR paths clean up handles or subscriptions when the component is disposed.
- For large lists or virtualization, test or smoke the paging/provider behavior rather than only rendering a tiny fixture.

## Evidence Focus

- In the evidence summary, name the Blazor decision: parameter contract, form model, lifecycle fetch, validation feedback, JS interop cleanup, cascading state, auth UI, virtualization, or SignalR disposal.
