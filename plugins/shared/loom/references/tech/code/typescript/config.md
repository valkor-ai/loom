# TypeScript Configuration Quality

Use this topic reference when `tech/code/typescript/config.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes `tsconfig` files, package build settings, module resolution, declaration output, project references, path aliases, framework TypeScript integration, or strictness flags.
- Use this when TypeScript configuration is blocking or shaping the implementation. Do not edit config as a shortcut around type errors in application code.
- If a task only changes normal TypeScript source and existing config works, leave configuration alone.

## Implementation Focus

- Inspect `package.json`, framework tooling, package manager scripts, and existing `tsconfig` layering before changing module settings. Apps using Vite or modern bundlers usually need bundler-oriented resolution; Node libraries and CLIs may need `NodeNext` semantics.
- Do not downgrade `strict`, `strictNullChecks`, `noImplicitAny`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, or similar safety flags to pass a build. Fix the changed code or narrow the config change to the intended package.
- When enabling a stricter flag, update the affected code in the same task or keep the change scoped so unrelated packages are not forced into broad repair work.
- Add path aliases only when the repository already supports alias resolution across TypeScript, bundler, test runner, linting, and runtime. Do not add an alias to shorten one or two imports.
- Use project references only for multi-package or layered builds that produce separate artifacts. Configure `composite`, `rootDir`, `outDir`, and declaration output consistently or avoid references.
- Generate declarations for libraries, SDKs, shared packages, or plugin APIs. Application-only packages normally do not need declaration output unless the repo already expects it.
- Keep `include` and `exclude` honest. Do not exclude broken source files, tests, or generated contract files merely to hide diagnostics.
- Use config inheritance deliberately: shared base config for common rules, package/framework config for environment-specific `lib`, `jsx`, `types`, and emit settings.
- Preserve existing `skipLibCheck` policy unless the task explicitly owns dependency type hygiene. Never use it to hide errors from local source or generated local declarations.
- If `isolatedModules` is required by the transpiler, avoid constructs that cannot be safely transpiled one file at a time, such as namespace-heavy patterns or unsafe const enum assumptions.

## Verification Focus

- Run the exact config that changed, for example `tsc -p tsconfig.json --noEmit`, `tsc -b`, or the package script that invokes TypeScript.
- Run the framework build or test command when module resolution, aliases, JSX, or emitted output can affect bundling/runtime behavior.
- If declaration output changed, run the package build and confirm generated declarations reference valid public paths.
- Confirm no changed config silently removes source coverage by excluding files that were previously typechecked.

## Evidence Notes

- Record `typescript.config` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/typescript/config.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the config decision: strictness, module resolution, alias ownership, project references, declaration output, framework layering, or include/exclude scope.
