# Flutter Structure Quality

This file applies Flutter project-structure discipline to task-owned `lib/`, feature modules, shared widgets, services, repositories, route configuration, themes, assets, and generated code boundaries.

## When To Use

- The task creates or changes Flutter project layout, feature folders, app entry points, `pubspec.yaml`, themes, constants, shared widgets, repositories, storage/API services, route setup, or code generation.
- Use this when file placement and module boundaries affect maintainability, route ownership, testability, or repeated feature delivery.
- Keep widget implementation details in the widget reference and state-management details in the Riverpod or Bloc reference.

## Implementation Focus

- Preserve the repository's structure. If it already uses feature-first folders, keep new feature code under the matching feature. If it uses layer-first or package/module boundaries, follow that convention.
- Keep app bootstrapping small: `main.dart` should initialize bindings, environment/config, storage, generated dependencies, and then run the root app/provider scope.
- Separate `core/` concerns such as theme, constants, errors, extensions, validators, logging, and reusable infrastructure from feature-specific workflow code.
- Keep feature folders cohesive. A feature that owns domain behavior should separate data/repository, domain/use-case, presentation/screens/widgets, and state providers/blocs according to existing style.
- Keep shared widgets truly reusable. Shared buttons, inputs, cards, list cells, and dialogs should not import feature repositories or domain-specific services.
- Treat `pubspec.yaml` changes as product-impacting. New dependencies, assets, fonts, platform permissions, and code-generation packages need matching usage and verification.
- Keep route config close to app/navigation ownership. Feature screens should not independently create competing router instances.
- Keep generated files and annotations consistent. If the repository uses `build_runner`, update generated outputs or note why generation is not required.
- Keep asset paths, localization files, theme files, and platform config references valid across all target platforms.

## Verification Focus

- Run Flutter analysis/tests after structure changes; run dependency resolution when `pubspec.yaml` changes.
- Verify new imports do not create circular dependencies between core, shared, feature data, feature domain, feature presentation, and state layers.
- Verify assets, fonts, localization, generated code, and route files resolve from the changed structure.
- Verify feature code can be tested without booting unrelated app routes or native services.
- Verify dependency additions are used intentionally and do not duplicate an existing repository library.

## Evidence Focus

- In the evidence summary, name the structure decision: feature boundary, app bootstrap change, shared widget boundary, repository/service placement, route config placement, dependency addition, generated output, or asset/config proof.
