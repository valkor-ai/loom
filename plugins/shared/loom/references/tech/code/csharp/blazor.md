# Blazor Component And Hosting Delivery

## When To Use

Use this reference only for a selected Blazor stack when the task owns components, routing, forms, interactive render modes, circuit/WASM state, JS interop, SignalR, streaming, virtualization, or Blazor lifecycle behavior.

## Implementation Focus

### Hosting And Render Mode

Identify Server, WebAssembly, Web App static SSR, interactive server, interactive WASM, or auto render mode and which components/routes cross modes. State, DI lifetime, API access, secrets, latency, reconnect, and browser capabilities differ.

Server-only services/secrets cannot enter WASM. Prerendered components may execute initialization twice; make data loading/idempotent side effects and persistent component state explicit.

Do not choose a render mode per component without considering serialization, package availability, download size, auth, and fallback.

### Component Contracts

Use parameters for values, `EventCallback<T>` for parent intent, and stable IDs in callbacks. `[EditorRequired]` improves tooling but does not enforce runtime presence.

Avoid mutating parameters/cascading objects directly. Keep editable form/view models separate from API/domain/EF records and define rebase/reset after parameter changes.

Keep cascading values narrow, stable, and `IsFixed` only when identity never changes. Large mutable app-state cascades can rerender broad trees and leak circuit/user state.

### Lifecycle And Async Work

Use initialization for stable first load, parameters-set for route/parameter-dependent load, and after-render only for DOM/JS availability. Guard first render without hiding required subsequent parameter behavior.

Cancel/order async work on parameter/navigation/disposal changes so old results cannot overwrite the current record. Call `InvokeAsync` when external callbacks update component state.

Avoid unnecessary `StateHasChanged`, render loops, and side effects during render. `ShouldRender` optimizations must not suppress validation/auth/data changes.

### Forms And Validation

Use one `EditContext`/form owner with field and business validation, invalid intermediate values, duplicate-submit prevention, backend errors, and returned readback.

Do not rely on disabled controls/hidden UI for authorization. Preserve valid input after rejection and focus/associate validation summaries/messages accessibly.

Map anti-forgery/auth/session behavior according to hosting mode and accepted API boundary.

### JS Interop

Import JS modules and create `DotNetObjectReference`/JS object handles at the owning component/service and dispose asynchronously. Handle navigation/disconnect (`JSDisconnectedException`) and avoid calls before interactive rendering.

Keep interop payloads bounded/serializable; use streaming APIs for large data. Validate browser-provided data and never expose privileged server methods through broad invokable callbacks.

### Server Circuits, WASM, And State

Blazor Server scoped services are circuit-scoped, not one HTTP request. Do not place another user/circuit state in singleton/static fields; bound per-circuit memory and handle reconnect/disconnect.

WASM client state/config/storage is public and untrusted. Keep authorization and secrets on server APIs and define offline/network failure behavior.

For persistent/prerender state, key by component/user/route and prevent cross-user reuse.

### Routing, Auth, And Errors

Validate route/query params and render not-found/forbidden/unavailable states. `AuthorizeView` and route UI are presentation; backend/endpoint must enforce access.

Use error boundaries at coherent regions with recovery/navigation/logging. They do not replace expected validation/business failures and must not reveal raw exceptions.

### Lists And Streaming

Use paging or `Virtualize` for large collections with stable item keys/provider cancellation/total semantics. Do not render unbounded server data into a circuit/browser.

Streaming rendering/skeletons need deterministic final state and no duplicate data load/side effect during prerender/interactivity transition.

## Verification Focus

- Build/publish the actual render modes and run focused component/integration tests.
- Test parameters/route changes, prerender double execution, loading/empty/error/form/readback, auth deny, reconnect/disconnect, and disposal.
- Verify JS module/object reference cleanup and unavailable/disconnected behavior.
- Exercise circuit/user isolation and WASM API authorization boundaries.
- Test Virtualize/provider cancellation/identity and representative rendered states/accessibility.

## Evidence Focus

Name hosting/render mode, component/form/lifecycle owner, state/interop isolation, and rendered/integration assertion. A component unit test or successful server render alone does not prove prerender, circuit, WASM, JS, or authorization behavior.

## Unsafe Defaults

- Blazor guidance loaded for non-Blazor .NET tasks.
- Server/WASM/SSR render modes treated as interchangeable.
- `[EditorRequired]` or hidden UI treated as runtime validation/authorization.
- Scoped state assumed to be per HTTP request in Blazor Server.
- JS handles/callbacks retained after navigation/disconnect.
- Unbounded collections rendered without paging/virtualization.
