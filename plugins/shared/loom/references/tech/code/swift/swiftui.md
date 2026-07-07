# Swift SwiftUI Quality

This file applies SwiftUI implementation guidance to task-owned UI changes.

## When To Use

- The task changes SwiftUI views, view models, navigation, state management, environment values, modifiers, custom layouts, async loading, refresh behavior, accessibility, or UI tests.
- Use this when SwiftUI state ownership, rendering performance, task lifecycle, or platform UI behavior affects correctness.
- If the project is UIKit/AppKit-only and no SwiftUI surface is touched, do not introduce SwiftUI.

## Implementation Focus

- Choose the right state owner: `@State` for local value state, `@Binding` for parent-owned values, `@StateObject`/`@Observable` for view-owned reference models, `@ObservedObject` for injected models, and environment only for broadly shared dependencies.
- Keep business logic out of `body`. Views should compose UI and call view-model/service actions; domain decisions belong outside rendering code.
- Break large views into meaningful subviews or modifiers when that clarifies state and layout. Do not split every small fragment into a type that obscures the screen flow.
- Use `task(id:)`, `refreshable`, and lifecycle modifiers so async work starts, cancels, and restarts with the right identity.
- Mark UI state mutation with `@MainActor` through the view model or action boundary. Avoid updating observable state from arbitrary background tasks.
- Use environment values for cross-cutting context such as theme, locale, or dependencies; avoid prop-drilling across many layers when environment is the local convention.
- Use preference keys and custom layouts only when regular layout containers cannot express the requirement. Keep geometry readers scoped so they do not dominate layout.
- For lists/grids, provide stable identities, avoid expensive work in row `body`, and page/lazy-load data when the expected collection is large.
- Treat previews as design aids, not verification. A preview that compiles is not proof that state, async, navigation, or accessibility works.

## Verification Focus

- Build the target platform and run available SwiftUI/UI tests or snapshot tests when the changed surface is user-facing.
- Verify loading, empty, error, success, disabled, and navigation states touched by the task.
- For async UI work, test or manually verify cancellation/reload behavior and that UI state updates stay on the main actor.
- Check accessibility labels/roles for new controls when the repository has UI accessibility standards.

## Evidence Focus

- In the evidence summary, name the SwiftUI decision: state owner, view-model split, composition, async task lifecycle, MainActor boundary, environment value, list performance, state coverage, or UI proof.
