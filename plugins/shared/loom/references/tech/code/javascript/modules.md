# JavaScript Module Quality

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

### Resolution And Publication

Treat `package.json` `type`, `exports`, `imports`, file extensions, and bundler aliases as one resolution contract. For a package with multiple consumers, define which conditions (`import`, `require`, `node`, `browser`, or the repository's supported condition) resolve to which artifact, and keep declaration/source maps aligned where they are owned.

Do not expose a source directory merely to make a test import pass. Add a public export only when the symbol is a supported API, and verify deep-import failures for paths that must remain private. Import maps and aliases are browser/build configuration; they do not change Node's package resolution unless the runtime explicitly supports them.

## Verification Focus

- Run build or bundle commands that exercise the changed import graph.
- Add or run an import smoke test for public package entry points, CLI entry modules, or dynamically imported modules.
- Verify both Node and browser targets when the module is consumed in both environments.
- Confirm no unintended circular dependency, missing extension, broken `exports` path, or default/named export mismatch was introduced.
- Verify each declared package entry under its supported runtime condition and run an import smoke test from the package boundary, not only from an internal relative path.

## Evidence Focus

- In the evidence summary, name the module decision: ESM/CJS convention, public exports, dynamic import, interop adapter, side-effect boundary, or circular dependency removal.
