# Kotlin Compose Quality

## When To Use

- The task changes Jetpack Compose UI, Compose Multiplatform UI, ViewModels, state holders, navigation, Material theme usage, effects, lists, animation, or UI tests.
- Use this when declarative UI state, recomposition, lifecycle, accessibility, or mobile/desktop UI behavior affects correctness.
- If UI is not Compose, use the relevant frontend/UI references instead.

## Implementation Focus

- Hoist state to the lowest stable owner that needs to coordinate it. Keep composables mostly stateless when parent/ViewModel ownership is needed, and use local `remember` only for local UI state.
- Expose screen state as an explicit model, preferably sealed or data-state based, covering loading, empty, error, success, editing, saving, and disabled states relevant to the workflow.
- Use lifecycle-aware collection such as `collectAsStateWithLifecycle` on Android when collecting flows in composables.
- Keep side effects in `LaunchedEffect`, `DisposableEffect`, or other Compose effect APIs with correct keys. Do not trigger network calls directly from the composable body.
- Use stable item keys in lazy lists when rows can change, update, or be reordered. Avoid unbounded eager rendering of large collections.
- Use `rememberSaveable` only for small serializable UI state that should survive recreation; do not store repositories, clients, or large objects in saved state.
- Keep Material theme, typography, spacing, and component style consistent with the app. Do not create one-off colors and layouts for each screen.
- Dispose listeners, callbacks, sensors, interop handles, and other external resources through `DisposableEffect` or lifecycle owners.
- Avoid `!!` in UI rendering. Make null/absent states explicit so the screen can render a proper loading, empty, or unavailable state.
- Treat previews as design aids, not verification. Runtime state, navigation, and validation still need tests or app smoke coverage.

## Decision Rules

- Define a screen state that distinguishes loading, empty, content, validation failure, save-in-progress, and recoverable error when the workflow has those states. Do not represent mutually exclusive states with independent flags.
- Hoist state to the narrowest owner that coordinates more than one composable. Keep transient input local, business state in a ViewModel/state holder, and pass events down as callbacks or stable interfaces.
- Key `LaunchedEffect` by the identity that invalidates the work. A changing query, route id, or authenticated user should cancel the old load; `Unit` is only correct for a true screen-lifetime effect.
- Use `rememberSaveable` for small user-editable values, not repositories, clients, large lists, or values that must be reloaded from the source of truth.
- Keep navigation arguments typed and validated at the boundary. Do not let a composable silently treat a missing argument as a valid empty identifier.
- Apply theme tokens and shared components before introducing screen-local colors, spacing, or typography. A preview that looks correct with one state is not evidence that interaction states are complete.

## Verification Focus

- Run the configured Compose/Android/desktop build or test task for changed UI modules.
- Test state rendering for loading, empty, error, success, validation, disabled, and saving states touched by the task.
- For ViewModel/Flow-backed screens, test state transitions and cancellation using coroutine test support.
- For lists and effects, verify stable keys and cleanup behavior when inputs change or the screen is disposed.
- Exercise the screen through the real navigation/state owner for at least one loading-to-content and one failure/retry path when those paths are part of the task.

## Evidence Focus

- In the evidence summary, name the Compose decision: state hoisting, screen state model, lifecycle collection, side effect key, lazy list keys, theme consistency, cleanup, or UI state verification.
