# UIX Scenario: Mobile Native

Use for iOS, Android, React Native, Flutter, Swift, Kotlin, or native-like mobile app surfaces.

## Baseline

- Respect platform navigation, safe areas, touch targets, and system conventions.
- Density is `comfortable`.
- Screens support one clear user task and preserve navigation context.
- Native UI must not look like a desktop web table squeezed into a phone.

## Screen Anatomy

```text
Root navigation
  -> tab/stack/shell
    -> screen header
    -> scrollable content
    -> sticky or contextual action area
    -> sheet/dialog only for focused secondary work
```

## Required Patterns

- Stack/tab navigation matching platform expectations.
- Safe-area-aware headers, bottom bars, sheets, and actions.
- Large enough touch targets and reachable primary actions.
- Offline/loading/error/permission states when relevant.
- Form inputs with mobile keyboards, validation, and preserved values.

## Component Guidance

- Lists use native list/card patterns with clear row identity and status.
- Forms use grouped sections, visible labels, and input-specific keyboards.
- Bottom sheets are for short choices or confirmations; complex flows get full screens.
- Destructive or financial actions need confirmation, review, or undo based on severity.
- Empty states should offer the next native action.

## Native State Handling

- Loading should use platform-appropriate progress indicators or skeleton/list placeholders.
- Permission states should explain the missing permission and route to recovery when possible.
- Offline states should separate unavailable network from empty data.
- Keyboard-aware layouts must keep active fields and submit actions reachable.

```text
screen -> loading/empty/error/content
action -> pending/success/failure
navigation -> back/cancel/restore context
```

## Verification

- Use simulator/device or framework preview when available.
- Check safe areas, keyboard behavior, scroll, and touch targets.
- Check platform back behavior and focus/voiceover labels when possible.
- Check dark/light mode only when the app supports both.

## Avoid

- Web-only hover interactions.
- Tiny table cells, cramped toolbars, and desktop sidebars.
- Ignoring platform back behavior or safe areas.
- Hiding critical action state in transient toast only.

## Platform Resolution

Resolve platform behavior before styling the screen. The same product action can
need different navigation, permission, keyboard, and feedback behavior on iOS,
Android, or a cross-platform runtime.

| Concern | Required decision |
| --- | --- |
| Navigation | Platform back gesture/button, deep link, tab/stack ownership, and restoration after relaunch. |
| Safe area | Insets for status bars, notches, home indicators, sheets, and keyboard. |
| Input | Keyboard type, focus order, scroll-to-focused-field, autofill, and dismissal behavior. |
| Permission | Pre-permission explanation, denied state, retry/settings route, and feature fallback. |
| Feedback | Native or platform-consistent pending, success, error, and destructive confirmation behavior. |
| Touch | Minimum target size, gesture conflict resolution, and reachable primary action. |

```text
screen shell -> platform header/back -> task content -> validation/permission
-> bottom or inline action -> success route or recoverable failure
```

Do not hide a platform limitation in a generic error. Explain what the user can
do next and keep already entered data when recovery is possible.
