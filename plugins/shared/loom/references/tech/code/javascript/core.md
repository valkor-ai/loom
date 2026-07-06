# JavaScript Core Quality

Use this topic reference when `tech/code/javascript/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes JavaScript source in apps, libraries, Node services, browser modules, build scripts, or shared runtime utilities.
- Use this for baseline modern JavaScript correctness: syntax target, data handling, public contracts, side-effect boundaries, and runtime safety.
- If the task is TypeScript-first, use TypeScript references instead; do not duplicate JavaScript guidance unless plain `.js`, `.mjs`, or `.cjs` files are part of the task.

## Implementation Focus

- Match the repository's runtime target before using newer language features. Do not use syntax or built-ins that the configured Node version, browser target, bundler, or test environment cannot run.
- Follow the package module convention. Keep ESM code in ESM files and CommonJS code in CommonJS files; isolate interop instead of mixing `import` and `require` in the same module.
- Use `const` by default and `let` only for intentional reassignment. Do not introduce `var`.
- Use optional chaining only for genuinely optional paths, and use nullish coalescing when `0`, `false`, or empty string are valid values. Do not replace meaningful falsy data with defaults via `||`.
- Keep external inputs validated at runtime: HTTP payloads, form values, storage data, environment variables, CLI args, and messages from workers or iframes.
- Add JSDoc for public functions, exported modules, and complex data shapes when the repository does not have TypeScript types for the contract.
- Keep pure transformation logic separate from side effects such as fetch, filesystem, DOM mutation, timers, logging, and process control.
- Do not mutate function parameters unless the local project convention uses controlled mutation for performance or framework APIs. Prefer returning new objects for business state changes.
- Use `Object.hasOwn` or safe ownership checks for untrusted objects. Avoid calling methods directly from data objects that may not inherit from `Object.prototype`.
- Avoid proposal-stage features unless the existing toolchain already transpiles them and the task owns compatibility risk.

## Verification Focus

- Run the repository's lint/build/test command that covers the changed JavaScript runtime.
- Smoke-test changed entry modules with the actual runtime when possible, especially build scripts, CLIs, Node services, and browser bootstrap files.
- Add tests for data validation, defaulting behavior, and side-effect boundaries that changed.
- Confirm no new unhandled promise rejections, unsupported syntax for the target runtime, or module-system mixing was introduced.

## Evidence Notes

- Record `javascript.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/javascript/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the JavaScript decision made: runtime target, module convention, defaulting behavior, validation boundary, JSDoc contract, or side-effect separation.
