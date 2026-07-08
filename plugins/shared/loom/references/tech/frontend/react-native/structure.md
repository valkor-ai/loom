# React Native Structure Quality

This file applies React Native project-structure discipline to task-owned Expo or native app folders, route groups, components, hooks, services, stores, constants, assets, and configuration.

## When To Use

- The task creates or changes mobile app structure, Expo Router folders, shared component directories, hooks, services, stores, configuration, TypeScript path aliases, or app metadata.
- Use this when file placement and module boundaries affect maintainability, navigation ownership, native build behavior, or repeated feature delivery.
- Keep route behavior in the navigation reference and platform behavior in the platform reference.

## Implementation Focus

- Respect the existing structure before adding new folders. If the app already separates `app/`, `components/`, `features/`, `hooks/`, `services/`, `stores/`, `constants/`, `types/`, or `assets/`, place task code inside that boundary.
- In Expo Router apps, keep route files in `app/` and reusable implementation outside routes. Do not put shared feature logic directly inside route group folders unless it is route-specific.
- Separate reusable UI primitives from feature-specific components. A button/input/card primitive should not import purchase/order/customer domain services.
- Keep hooks focused: data fetching hooks own query orchestration, storage hooks own persistence, permission hooks own native permission state, and UI hooks own view behavior.
- Keep services small and typed around API, auth, analytics, native modules, or storage. Do not hide UI state transitions inside generic service methods.
- Use constants for colors, spacing, breakpoints, storage keys, feature flags, and platform dimensions when the repository has an equivalent token system.
- Keep app configuration changes deliberate. Changes to `app.json`, bundle/package identifiers, URL schemes, permissions, icons, splash assets, or plugins affect release behavior and need evidence.
- Use TypeScript path aliases only when the repository already supports them or the task introduces all required config consistently across TypeScript, Babel/Metro, tests, and tooling.
- Keep Reanimated Babel plugin ordering and native module setup aligned with repository requirements when those libraries are present.

## Verification Focus

- Run TypeScript/build tooling that proves new path aliases, imports, app config, and route files resolve.
- Run Expo/React Native doctor or repository equivalent when the task touches Expo config, native modules, or SDK compatibility.
- Verify no route file imports a feature through fragile relative paths when the repository has stable aliases.
- Verify assets referenced by config and screens exist at the expected paths and load on the target platform.
- Verify structure changes do not create circular imports between components, hooks, services, and stores.

## Evidence Focus

- In the evidence summary, name the structure decision: route-vs-feature placement, UI primitive boundary, hook/service/store split, config change, path alias consistency, asset placement, or native module setup proof.
