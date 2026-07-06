# Kotlin Compose Quality

Use this topic reference when `tech/code/kotlin/compose.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

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

## Verification Focus

- Run the configured Compose/Android/desktop build or test task for changed UI modules.
- Test state rendering for loading, empty, error, success, validation, disabled, and saving states touched by the task.
- For ViewModel/Flow-backed screens, test state transitions and cancellation using coroutine test support.
- For lists and effects, verify stable keys and cleanup behavior when inputs change or the screen is disposed.

## Evidence Notes

- Record `kotlin.compose` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/kotlin/compose.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Compose decision: state hoisting, screen state model, lifecycle collection, side effect key, lazy list keys, theme consistency, cleanup, or UI state verification.
