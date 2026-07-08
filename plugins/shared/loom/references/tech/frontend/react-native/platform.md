# React Native Platform Quality

This file applies platform-specific discipline to task-owned iOS and Android differences, safe areas, keyboard behavior, status bars, shadows/elevation, dimensions, permissions, and native module behavior.

## When To Use

- The task creates or changes mobile UI or behavior that may differ between iOS and Android: layout, keyboard, safe area, status bar, hardware back, permissions, native modules, gestures, shadows, files, or device APIs.
- Use this when platform correctness is part of the product workflow or when the implementation adds `.ios.tsx`, `.android.tsx`, `Platform.select`, or native configuration.
- Keep route-level behavior in the navigation reference and list virtualization in the lists reference.

## Implementation Focus

- Use platform-specific files for substantial implementation differences. Use `Platform.select` for small style/value differences such as shadow/elevation, fonts, status bar color, keyboard offset, or animation tuning.
- Handle safe areas with the repository's safe-area provider/hooks. Do not assume top/bottom padding values or ignore notches/home indicators.
- Wrap forms and long editable surfaces with keyboard-aware behavior. Use keyboard offsets that account for headers, tabs, or custom navigation chrome.
- Handle Android hardware back for unsaved changes, open modals, custom drawers, multi-step flows, and destructive confirmation. Return the correct boolean to prevent or allow default behavior.
- Keep status bar style and background coordinated with the active screen and platform. Android may need explicit background color; iOS usually does not.
- Avoid hardcoded dimensions. Use flex layout, window dimensions, measured layout, or design tokens according to the repository style.
- Keep permission prompts, native module calls, and unavailable-platform cases explicit. Unsupported features need a product-appropriate disabled or explanatory state.
- Avoid broad platform forks when a shared implementation plus small platform values is enough. Forking duplicates bug fixes and tests.
- Clean up event listeners, subscriptions, sensors, keyboard listeners, app-state listeners, and back handlers on unmount.

## Verification Focus

- Verify affected screens on iOS and Android when platform-specific code, safe area, keyboard, hardware back, status bar, permissions, or native modules are in scope.
- Verify keyboard-open form behavior, scrollability, focused input visibility, submit button accessibility, and dismissal behavior.
- Verify notches, home indicator, Android navigation bar, and status bar do not cover product controls.
- Verify platform-specific files compile and the shared import resolves to the expected file on each platform.
- Verify listener cleanup by navigating away or closing the screen after triggering platform events.

## Evidence Focus

- In the evidence summary, name the platform decision: platform file split, `Platform.select` value, safe-area handling, keyboard behavior, Android back handling, status bar handling, permission fallback, or native listener cleanup.
