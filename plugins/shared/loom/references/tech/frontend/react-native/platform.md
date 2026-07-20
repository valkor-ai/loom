# React Native Platform Behavior

Apply this reference only when the task owns iOS/Android-specific behavior, safe areas, keyboard/status-bar/system navigation, permissions, device APIs, native modules, gestures, or lifecycle differences.

## Choose The Smallest Split

Keep shared behavior shared. Use `Platform.select` or a narrow runtime branch for small values and `.ios`/`.android` modules for substantial implementations with distinct dependencies or behavior.

Do not scatter `Platform.OS` checks throughout feature logic. Put platform policy behind a component/hook/adapter with one typed contract so common workflow code remains testable.

Confirm platform file resolution in Metro, TypeScript, tests, and native builds. A shared fallback should exist when desktop/web is a supported React Native target.

## Safe Areas And System Chrome

Use the repository safe-area provider/insets and apply each edge at the correct layout owner. Avoid double padding when navigation headers/tab bars already consume an inset.

Coordinate status bar style/background, Android navigation bar, translucent system bars, modals, and orientation with the active surface. Hardcoded inset heights fail across notches, islands, tablets, rotation, and immersive modes.

Keep primary actions and dismiss controls clear of system gestures/home indicators at large font sizes and compact heights.

## Keyboard And Forms

Choose keyboard avoidance/scroll behavior based on navigation header, tabs, modal presentation, and focused control position. A copied `keyboardVerticalOffset` is not portable.

Keep the focused input and validation message visible, preserve submit access, define tap-to-dismiss and `keyboardShouldPersistTaps`, and avoid nested scroll containers fighting for gestures.

Test hardware and software keyboards where relevant, including multiline, autofill/password manager, return-key sequencing, and orientation changes.

## Back, Gestures, And Overlays

Android hardware back should dismiss the topmost owned overlay or confirm unsaved work, then delegate to navigation. Return the correct handled state and remove listeners when focus changes.

Coordinate edge-swipe/back gestures, drawers, bottom sheets, Reanimated/Gesture Handler roots, and scroll gestures. Do not disable platform navigation globally to solve a local conflict.

Honor reduced-motion settings and avoid animation completion as the only way critical state advances.

## Permissions And Device Capabilities

Model undetermined, granted, denied, blocked/permanently denied, restricted, unavailable, and interrupted states according to the installed permission API. Explain why before prompting when product context requires it.

Request only at the user action that needs access, provide a safe fallback, and link to settings only when the platform state supports it. Never loop permission prompts.

Validate device capability separately from permission: camera, biometrics, notifications, location services, files, Bluetooth, and sensors may be unavailable or disabled despite granted permission.

## Native Modules And Lifecycle

Use the repository's Expo module/config-plugin or native-linking path. Confirm SDK/RN/platform version compatibility, Pod/Gradle configuration, required manifest/plist entries, and rebuild requirements.

Handle module initialization failure and unsupported environments without crashing the entire screen. Clean up listeners/resources and account for app active/background/inactive transitions.

Do not invoke native APIs during render. Sequence async results so a response from a prior screen/account/request cannot update current UI.

## Platform Presentation

Use platform-appropriate shadows/elevation, typography, pickers, feedback, date/time behavior, file access, and share intents while preserving the product design system. Avoid forced visual sameness that breaks native expectations.

Account for locale, dynamic type/font scaling, RTL, contrast, screen reader, switch control, and touch exploration on affected controls.

## Verification

- Run affected iOS/Android compile or Expo development-client checks for native/config changes.
- Exercise safe-area/system chrome at representative devices, rotation, large text, and modal/tab/header combinations.
- Verify keyboard focus/visibility/submit/dismiss behavior and back/gesture ownership.
- Test every owned permission/capability state and app lifecycle transition.
- Confirm platform modules resolve correctly and listeners/resources are released after leaving the surface.

## Delivery Evidence

Name the platform split, device/system API, lifecycle/permission states, and actual platform/runtime evidence. One simulator success does not prove both platforms, physical-device capability, production signing, or unavailable-state behavior; record those limits accurately.

## Unsafe Defaults

- Platform reference loaded for every React Native task.
- Hardcoded safe-area or keyboard offsets copied from an example.
- Platform checks spread through business logic.
- Permission prompt issued on mount or repeated after denial.
- Native module import assumed to complete installation/configuration.
- Global back/gesture behavior changed for a local screen.
- Platform parity claimed without matching evidence.
