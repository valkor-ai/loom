# React Native Core Quality

This file applies React Native and Expo implementation discipline to task-owned mobile screens, components, hooks, services, and cross-platform workflows.

## When To Use

- The task creates or changes React Native screens, Expo app surfaces, shared mobile components, native-aware hooks, forms, lists, navigation-connected screens, or mobile state orchestration.
- Use this for React Native rendering behavior, mobile accessibility, safe area and keyboard concerns, native module boundaries, styling, and platform-aware workflow delivery.
- If the task is pure web React, use the React references instead. React Native references are primary for iOS/Android surfaces and should not be treated as web React pages.

## Implementation Focus

- Preserve the repository's app framework: Expo Router, React Navigation, bare React Native, Expo managed workflow, styling approach, state library, and test runner.
- Use React Native primitives and platform conventions. Do not render web-only elements, CSS assumptions, browser APIs, or DOM event patterns inside native screens.
- Keep screens as workflow orchestration. Extract reusable UI components, feature components, hooks, services, formatters, validators, and native adapters when a screen becomes hard to inspect.
- Use `StyleSheet.create`, token/constants files, or the repository's styling system for reusable styles. Avoid broad inline style object creation in list rows or frequently rerendered components.
- Handle safe areas, keyboard overlap, status bar, hardware back behavior, touch targets, accessibility labels, and screen reader roles for task-owned interactive surfaces.
- Use stable IDs for list rows, selected records, navigation params, and mutation targets. Do not rely on visible row order or array indexes for actions.
- Keep API clients, secure storage, native modules, and permission checks behind small service/hook boundaries so screens do not own infrastructure details.
- Keep product UI free of delivery notes, runtime commands, framework explanations, verification instructions, and desktop-only wording.
- Treat simulator-only success as partial evidence when the task touches platform behavior. Name which platform behavior was proven and which remains unproven.

## Verification Focus

- Run the repository's TypeScript, lint, Metro/Expo, unit, and focused screen/component tests when available.
- Verify loading, empty, ready, validation error, business-blocking error, offline/error, submitting, success, disabled, keyboard-open, and navigation states touched by the task.
- Verify iOS and Android behavior when safe area, keyboard, hardware back, status bar, native modules, permissions, or platform-specific styling is in scope.
- For mobile forms, verify keyboard avoidance, submit disablement, input capitalization/autocomplete choices, backend errors, and resubmit behavior.
- For native module changes, verify module installation/config compatibility and rebuild requirements according to the repository workflow.

## Evidence Focus

- In the evidence summary, name the mobile decision: Expo/React Navigation boundary, native primitive use, safe-area/keyboard handling, platform split, list identity, native module boundary, or platform verification coverage.
