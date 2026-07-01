# UIX Scenario: Mobile Native

Use for iOS, Android, React Native, Flutter, Swift, Kotlin, or native-like mobile app surfaces.

## Baseline

- Respect platform navigation, safe areas, touch targets, and system conventions.
- Density is `comfortable`.
- Screens should support one clear user task and preserve navigation context.
- Native UI must not look like a desktop web table squeezed into a phone.

## Required Patterns

- Stack/tab navigation matching platform expectations.
- Safe-area-aware headers, bottom bars, sheets, and actions.
- Large enough touch targets and reachable primary actions.
- Offline/loading/error/permission states when relevant.
- Form inputs with mobile keyboards, validation, and preserved values.

## Layout

- Use lists, cards, grouped forms, sheets, and drill-down details.
- Use platform typography and spacing conventions unless the app already has tokens.
- Keep destructive or financial actions separated from routine navigation.

## Avoid

- Web-only hover interactions.
- Tiny table cells, cramped toolbars, and desktop sidebars.
- Ignoring platform back behavior or safe areas.
