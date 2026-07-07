# React Native Navigation Quality

This file applies React Native navigation discipline to task-owned Expo Router routes, React Navigation stacks/tabs/drawers, route params, deep links, protected routes, and back behavior.

## When To Use

- The task creates or changes mobile routes, stacks, tabs, drawers, route groups, modals, dynamic routes, navigation params, protected flows, deep links, or back behavior.
- Use this for Expo Router and React Navigation decisions that affect user movement, route ownership, auth gates, modal presentation, and platform navigation expectations.
- If the task only changes a screen's internal rendering without navigation behavior, keep this file out of scope.

## Implementation Focus

- Preserve the repository's router: Expo Router file-based routes, React Navigation, or a hybrid wrapper. Do not introduce a second navigation model for one task.
- Keep route groups meaningful: auth flows, tabbed product areas, modal/detail flows, settings, onboarding, and protected surfaces should have clear ownership.
- Type route params and validate them before API calls, store dispatches, or native module operations. Missing or malformed params need explicit UI states.
- Use `router.push`, `router.replace`, links, or navigation actions according to stack semantics. Do not use replace where the user should be able to go back.
- Gate protected routes at layout or navigator boundaries when possible, not by sprinkling redirects inside every leaf screen.
- Preserve deep-link behavior for product-critical detail screens and return paths. URL schemes and linking config changes must match app config.
- Handle Android hardware back when task-owned screens have unsaved changes, open drawers/sheets, multi-step forms, or custom modal behavior.
- Keep navigation headers, tab labels, titles, icons, and accessibility labels aligned with the product workflow and design system.
- Avoid passing large mutable objects through route params. Pass stable IDs and load/derive the current record through the app's data layer.

## Verification Focus

- Verify route entry, forward navigation, back navigation, tab switching, modal presentation/dismissal, protected redirect, and deep-link entry touched by the task.
- Verify route params are parsed safely and produce not-found or blocked states when invalid.
- Verify Android hardware back and iOS swipe/back behavior when custom navigation or unsaved changes exist.
- Run navigation/screen tests where the repository has test utilities; otherwise prove behavior through Expo/Metro and manual/simulator evidence.
- Verify app config URL scheme or linking changes when deep links are introduced.

## Evidence Focus

- In the evidence summary, name the navigation decision: route group, stack/tab/modal ownership, param validation, protected layout, deep link, Android back handling, or navigation-state proof.
