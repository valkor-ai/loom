# Flutter Project And Feature Structure

Apply structure guidance only when the task owns application setup, frontend architecture, dependency/config/assets/code generation, or a framework migration. Ordinary widget tasks should preserve existing placement without loading a full project-layout reference.

## Preserve The Repository Shape

Identify whether the project is feature-first, layer-first, package-based, or a hybrid before creating directories. Extend the selected ownership boundaries; do not copy a reference tree over an established application.

A feature-first structure can separate presentation/state/domain/data only when those layers exist meaningfully:

```text
lib/
  app/                 # bootstrap, router, theme, global composition
  core/                # stable cross-feature platform/infrastructure primitives
  features/
    orders/
      presentation/    # screens and feature widgets
      state/           # selected providers/blocs
      domain/          # accepted entities/use cases/ports
      data/            # DTOs, repositories, adapters
```

Do not create empty ceremonial layers or place feature-specific rules in `core`, `shared`, or `utils`. Shared widgets/utilities must be genuinely reusable and must not import feature repositories/state.

## Bootstrap And Environment

Keep `main.dart` small and deterministic: initialize required framework/plugins/config, install root provider/router/theme/localization, then run the app. Separate flavors/entry points only when the product/runtime contract selects them.

Treat browser/mobile configuration as public. Keep API binding/environment values in the repository's runtime/build mechanism and validate them; never embed service credentials.

Initialization that can fail needs loading/failure/recovery behavior or clear startup failure. Do not perform database migrations, unbounded network calls, or seed business data during every app startup without an accepted lifecycle.

## Dependencies And pubspec

Before adding a package, check existing capabilities, target-platform support, SDK constraints, maintenance, transitive/native requirements, license, and bundle/build impact. Do not add Riverpod, Bloc, GoRouter, Dio, Freezed, or storage libraries because a template uses them.

Keep dependency versions compatible with the selected Flutter/Dart SDK and lockfile policy. Avoid broad `dependency_overrides` as a routine resolution.

Declare fonts/assets accurately, including case-sensitive paths and directory semantics. Verify every target used by the feature.

## Generated Code

Use the repository's `build_runner`, Riverpod generator, Freezed, JSON serialization, localization, or asset-generation commands and committed-output policy. Source annotations and generated output must agree.

Never hand-edit generated files. Watch for stale generated code, duplicate part names, conflicting builders, and missing outputs after model/provider changes.

Generated persistence/JSON models are transport/storage representations; keep domain/UI semantics separate where the accepted architecture does.

## Platform Boundaries

Place platform plugins behind narrow adapters and keep Android/iOS/web/desktop configuration synchronized: permissions, manifests/plists, URL schemes, entitlements, minimum versions, signing capabilities, and plugin initialization.

Use conditional imports/implementations that compile for every selected target. Avoid importing `dart:io` in web code or scattering platform checks through feature widgets.

Add platform permission copy and denied/restricted behavior according to product requirements, not plugin defaults.

## Routes, Theme, Localization, And Assets

Keep one router composition owner and feature-owned route fragments where supported. Screens must not create competing router instances.

Centralize theme/color/text/spacing/component extensions and localization delegates/generated strings. Feature code consumes semantic tokens/strings rather than hardcoded visual or language values.

Assets and localization keys need deterministic naming, ownership, removal, and tests/build verification to prevent stale bundle content.

## Dependency Direction

Enforce the accepted direction through imports/packages and avoid feature cycles. Data adapters may implement domain/application ports; domain code should not depend on Flutter widgets, BuildContext, state libraries, or platform plugins when those layers are separated.

State/navigation should depend on feature operations, not make repositories depend on UI. Do not use global service locators to erase project boundaries.

## Verification

- Run dependency resolution/analyze after `pubspec.yaml`, SDK, lint, or generator changes.
- Build/test selected targets after platform plugin/config/conditional import changes.
- Regenerate outputs and verify no stale/hand-edited generated files.
- Verify assets/fonts/localization/theme/router composition from the new paths.
- Inspect imports/package dependencies for cycles and feature leakage.
- Confirm feature code can be tested without booting unrelated native services/routes.

## Delivery Evidence

Identify the structure/dependency/config/generated/platform decision and the resolve/analyze/build/import assertion proving it. A directory tree or successful editor import cannot prove target builds, plugin setup, generator freshness, or dependency direction.

## Unsafe Defaults

- Reference folder tree imposed on an established project.
- Empty clean-architecture layers and feature logic placed in `core/shared/utils`.
- Packages added from examples without stack/platform/license checks.
- Generated files edited manually or left stale.
- Platform permission/config changed for only one selected target.
- Global service locator used to bypass dependency boundaries.
