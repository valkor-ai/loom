# Go Project Structure Quality

Use this topic reference when `tech/code/go/structure.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Go module layout, `go.mod`, `go.work`, `cmd`, `internal`, package boundaries, build tags, generated code, configuration packages, version injection, or release/build wiring.
- Use this when file placement and import boundaries affect maintainability or build correctness.
- If the task only changes logic inside an existing package, preserve the existing structure.

## Implementation Focus

- Follow the current repository layout before introducing a textbook layout. Do not create `cmd`, `internal`, or `pkg` solely because a template recommends it.
- Use `cmd/<name>` for executable entry points when the repository has multiple binaries or a clear CLI/server entry point. Keep business logic out of `main` packages.
- Use `internal` to enforce private application boundaries. Do not place public library APIs under `internal`, and do not use `pkg` as a dumping ground for miscellaneous helpers.
- Keep packages cohesive by domain or capability. Avoid package names like `utils`, `common`, or `helpers` unless the repository already uses them and the content is genuinely cross-cutting.
- Run `go mod tidy` only when dependency changes require it. Do not churn `go.mod` or `go.sum` for unrelated edits.
- In multi-module repositories, respect `go.work` and module ownership. Do not add imports across modules that bypass published/shared package boundaries.
- Use build tags for platform, integration, generated, or optional feature code. Keep tag expressions documented by file placement or test command so hidden code paths are not forgotten.
- Keep generated code clearly marked and regenerate it through existing commands. Do not hand-edit generated files unless the repository treats them as checked-in source.
- Keep runtime configuration loading in a dedicated boundary rather than scattered through handlers or domain packages.
- If version/build metadata is added, inject it through existing linker flags or build pipeline conventions; do not hard-code release values into source.

## Verification Focus

- Run `go list ./...` or `go test ./...` to catch import cycles, broken build tags, and module resolution errors.
- Run package or binary build commands when changing `cmd`, build tags, generated code, or module files.
- Verify `go.mod` and `go.sum` only changed for dependencies actually needed by this task.
- For platform or tag-specific files, run the relevant tagged test/build command or record why it could not be run.

## Evidence Notes

- Record `go.structure` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/go/structure.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the structure decision: entry point layout, internal boundary, package cohesion, module ownership, build tag, generated code, config boundary, or version injection.
