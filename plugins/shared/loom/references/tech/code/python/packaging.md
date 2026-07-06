# Python Packaging Quality

Use this topic reference when `tech/code/python/packaging.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes `pyproject.toml`, package layout, dependency declarations, lock files, CLI entry points, build metadata, type package markers, import paths, or distribution settings.
- Use this when packaging decisions affect installation, runtime importability, dependency resolution, or release artifacts.
- If the task only changes Python source inside an established package, preserve packaging files unless the source change requires them.

## Implementation Focus

- Follow the repository's package manager and build backend. Do not switch between Poetry, Hatch, setuptools, uv, pip-tools, or plain requirements as an incidental change.
- Keep application and library dependency rules distinct. Applications can use lock files and pinned runtime dependencies; libraries should usually use compatible version ranges and avoid over-pinning transitive behavior.
- Use dependency groups or extras according to the existing project style. Do not put test, lint, docs, or dev-only tools in runtime dependencies.
- Preserve `src/` layout or flat layout based on the repository. Do not move packages just to match a template.
- Add `py.typed` only for packages that intentionally expose typed public APIs. Ensure it is included in package data when distribution is relevant.
- Keep CLI entry points in packaging metadata when commands must be installed by users. Direct script paths are acceptable only when the repository already uses them for local tooling.
- Keep package version source of truth clear. Do not duplicate version constants across source, metadata, and release scripts without an existing synchronization pattern.
- Update lock files only when dependency changes require it. Do not churn lock files for source-only changes.
- Include non-Python package data deliberately through the existing backend's package-data mechanism. Do not rely on files being present because they exist in the repo.
- Keep import paths stable for consumers; moving modules needs compatibility exports or explicit migration when the package is public.

## Verification Focus

- Run an import smoke test for changed package/module paths.
- Run the repository's build command when packaging metadata, package data, entry points, or dependencies changed.
- Validate CLI entry points by invoking the installed or local command when added or changed.
- Confirm runtime dependencies, optional/dev dependencies, lock files, and package data changed only for task-relevant reasons.

## Evidence Notes

- Record `python.packaging` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/python/packaging.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the packaging decision: build backend, dependency scope, package layout, typed marker, CLI entry point, version source, lock update, package data, or import compatibility.
