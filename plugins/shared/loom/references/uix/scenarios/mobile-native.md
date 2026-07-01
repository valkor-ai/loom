# Mobile Native UIX

Use for native or cross-platform mobile applications: iOS, Android, React Native, Expo, Flutter, or native-shell app work.

## Baseline

- Respect platform navigation, safe areas, gestures, keyboard behavior, permission flows, and offline/interrupted states.
- Use platform-appropriate controls and touch targets instead of shrinking desktop controls.
- Preserve state across app lifecycle interruptions when the workflow is risky or long.

## Required Patterns

- Screens need navigation title, back/close behavior, loading/error/empty states, and accessible labels.
- Inputs must account for virtual keyboard and validation timing.
- Sheets, pickers, tabs, and gestures need escape/recovery paths and reduced-motion behavior where supported.
