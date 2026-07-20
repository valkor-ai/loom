# TypeScript Configuration Quality

## When To Use

- Load only when the task changes `tsconfig` files, package build settings, module resolution, declarations, project references, aliases, framework integration, or compiler strictness.
- Do not edit configuration as a shortcut around application type errors; fix the source or narrow the config change to the owned package.
- Inspect `package.json`, package-manager scripts, bundler, test runner, and existing config inheritance before changing compiler options.

## Decision Rules

- Preserve `strict`, `strictNullChecks`, `noImplicitAny`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, and related safety flags. A build fix must not weaken them silently.
- Match module resolution to the runtime: bundler-oriented resolution for bundler applications, and `NodeNext`-compatible settings for Node libraries and CLIs.
- Add a path alias only when TypeScript, bundler, test runner, linting, and runtime resolve the same mapping. One shortened import is not sufficient justification.
- Use project references only for real package or layer boundaries that emit separate artifacts; align `composite`, `rootDir`, `outDir`, and declarations.
- Generate declarations for libraries, SDKs, shared packages, and plugin APIs. Application-only packages do not need them unless the repository consumes them.
- Keep `include` and `exclude` honest. Never exclude source, tests, or generated contracts to hide diagnostics.
- Preserve the repository's `skipLibCheck` policy and never use it to hide local source or local declaration failures.

## Implementation Focus

- Put common compiler rules in the existing base config and keep environment-specific `lib`, `jsx`, `types`, and emit options in the package config.
- If a stricter flag is enabled, repair its affected source in the same task or scope the flag to the intended package.
- For `isolatedModules`, avoid namespace-heavy or unsafe one-file transpilation patterns and verify the actual framework build.
- Use incremental compilation or build metadata only when the repository has a reproducible cache location and clean-build path.

## Failure Modes

- Do not change `module`, `moduleResolution`, JSX, or aliases without checking the bundler, test runner, and runtime together.
- Do not make a local package pass by excluding its tests or source files from a shared config.
- Do not commit generated declaration or build output when the repository treats it as a derived artifact.

## Verification Focus

- Run the exact changed config with `tsc -p` or `tsc -b`, then run the framework build when aliases, JSX, module resolution, or emitted output can affect runtime.
- If declaration output changes, inspect generated public paths and run a clean package build.
- Confirm no changed config silently removes files from typechecking.

## Evidence Focus

- Record the configuration decision: strictness, module resolution, alias ownership, project references, declaration output, framework layering, or file scope.
