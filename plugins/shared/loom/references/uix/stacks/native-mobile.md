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

## Cross-Platform Notes

- React Native: keep presentational components separate from screen orchestration when workflows grow.
- Flutter: keep widgets focused and use theme tokens consistently.
- Native iOS/Android: use platform controls unless custom controls are justified.
- UniApp or mini-app targets should also load the UniApp stack reference when selected.

## Verification

- Use simulator/device or framework preview when available.
- Check safe areas, keyboard behavior, scroll, and touch targets.
