# JavaScript Module Quality

Use this topic reference when `tech/code/javascript/modules.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes imports, exports, package entry points, `package.json` `type` or `exports`, dynamic imports, bundling boundaries, CommonJS/ESM interop, or module layout.
- Use this when module shape affects runtime loading, tree shaking, package consumers, test setup, or browser/Node compatibility.
- If the task only changes logic inside an existing module and import/export shape is unchanged, this reference should not expand scope.

## Implementation Focus

- Determine the package convention from `package.json`, file extensions, bundler config, and existing imports. Do not convert a package between CommonJS and ESM as an incidental fix.
- In ESM code, use explicit file extensions where the runtime requires them. In bundler-managed apps, follow the bundler and repository import style rather than mixing conventions.
- Keep CommonJS interop in a small adapter boundary, such as a `.cjs` wrapper or `createRequire` helper. Do not scatter `require` calls through ESM modules.
- Prefer named exports for libraries and shared utility modules so consumers and bundlers can select stable APIs. Keep default exports when the framework or repository convention expects them.
- Use `package.json` `exports` to expose intentional public entry points. Do not expose internal folders or unstable build artifacts unless the package already treats them as public API.
- Use dynamic import for lazy features, optional heavy dependencies, or environment-specific code. Validate dynamic import keys; do not interpolate arbitrary user input into module paths.
- Avoid circular dependencies by moving shared contracts/constants to a neutral module or injecting dependencies through a factory. Do not patch cycles with late mutation unless the project already uses that pattern.
- Keep side-effect imports limited to bootstrap, polyfills, global styles, or explicit registration modules. A normal utility module should not mutate global state at import time.
- When changing module boundaries, preserve test and build runner compatibility; Jest/Vitest/Node/bundlers often resolve ESM and CommonJS differently.

## Verification Focus

- Run build or bundle commands that exercise the changed import graph.
- Add or run an import smoke test for public package entry points, CLI entry modules, or dynamically imported modules.
- Verify both Node and browser targets when the module is consumed in both environments.
- Confirm no unintended circular dependency, missing extension, broken `exports` path, or default/named export mismatch was introduced.

## Evidence Notes

- Record `javascript.modules` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/javascript/modules.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the module decision: ESM/CJS convention, public exports, dynamic import, interop adapter, side-effect boundary, or circular dependency removal.
