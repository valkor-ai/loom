# UIX Stack: Native Mobile

Use for React Native, Flutter, SwiftUI/UIKit, Jetpack Compose, Kotlin/Android, and native-like mobile apps.

## Structure

- Follow platform navigation and state conventions.
- Separate screens, reusable components, domain hooks/services, and navigation configuration.
- Respect safe areas, keyboard avoidance, and platform back behavior.

## Screen Composition

```text
NavigationContainer
  Tab or Stack Navigator
    Screen
      SafeArea
      Header
      Scroll/List Content
      Sticky Action or Sheet
```

## Implementation Rules

- Touch targets must be comfortable and spaced.
- Use platform text, color, spacing, and elevation conventions unless a design system exists.
- Forms need input types, validation, loading/submitting, and error recovery.
- Lists need empty/loading/error states and stable item identity.
- Sensitive financial or destructive actions need explicit confirmation/review.
- Translate web token intent into the platform theme system. Do not create CSS token files for a native-only target.
- Keep safe-area, keyboard avoidance, and permission states part of the screen implementation, not only review notes.

## Cross-Platform Notes

- React Native: keep presentational components separate from screen orchestration when workflows grow.
- Flutter: keep widgets focused and use theme tokens consistently.
- Native iOS/Android: use platform controls unless custom controls are justified.
- UniApp or mini-app targets should also load the UniApp stack reference when selected.

## Verification

- Use simulator/device or framework preview when available.
- Check safe areas, keyboard behavior, scroll, and touch targets.
- Record platform/device or preview constraints in evidence when full verification is unavailable.

## Platform Implementation Boundary

UIX owns the visible screen composition and platform behavior. Keep framework
configuration, package selection, networking, persistence, and native build
settings in the project's engineering references and existing code conventions.

```text
platform navigation -> screen shell -> task region -> native input/list
-> local validation -> async action -> platform feedback -> next route
```

- A screen owns its header/back affordance, safe-area container, scroll region, and primary action placement.
- A feature component owns the visible representation of loading, empty, error, disabled, permission, and success states; do not leave these as invisible service outcomes.
- Keep platform adapters behind a narrow interface so iOS/Android differences do not duplicate the business surface.
- Use native controls for semantics, focus, keyboard, accessibility, and destructive confirmation before introducing a custom control.

## Screen State And Restoration

```ts
type ScreenState<T> =
  | { kind: 'loading' }
  | { kind: 'ready'; value: T }
  | { kind: 'empty'; action?: 'create' | 'retry' }
  | { kind: 'error'; message: string; canRetry: boolean };
```

Keep draft input, selected identity, and navigation return context recoverable
after backgrounding, rotation, permission prompts, or a failed request. A
successful mutation must update the visible object and route to the next useful
screen rather than only showing a transient notification.
