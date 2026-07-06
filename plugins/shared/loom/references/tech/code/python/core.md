# Python Core Quality

Use this topic reference when `tech/code/python/core.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Python application, library, CLI, service, data processing, configuration, or shared module code.
- Use this for baseline Python correctness: version compatibility, standard library choices, resource handling, exceptions, logging, and domain data modeling.
- If the task only changes generated files, notebooks, documentation, or non-Python assets, do not expand scope because this reference is available.

## Implementation Focus

- Match the repository's supported Python version before using newer syntax or library features. Do not assume Python 3.11+ unless project metadata, tooling, or runtime states it.
- Use `pathlib.Path` for filesystem paths in new code unless the surrounding API requires strings. Keep encoding explicit for text I/O.
- Use context managers for files, network clients, database sessions, locks, temporary directories, and other resources that need cleanup.
- Use dataclasses, enums, or small domain classes for structured domain values when behavior or validation belongs with the data. Do not pass large untyped dictionaries through business logic when field names are contractual.
- Avoid mutable default arguments. Use `field(default_factory=...)` for dataclasses and initialize mutable values inside functions or constructors.
- Raise explicit domain or standard exceptions with actionable messages. Do not use bare `except`, and do not swallow exceptions unless the fallback behavior is part of the contract.
- Use logging instead of `print` in application/runtime code. Keep loggers module-scoped, avoid logging secrets, and reserve stack traces for errors that need diagnosis.
- Keep configuration loading and validation at startup or adapter boundaries. Do not read environment variables throughout domain code.
- Prefer standard library tools that match the need: `collections.abc` for protocols, `contextlib` for resource lifecycles, `itertools` for streaming transforms, `functools` for caching/decorators, and `collections` for queues/counters/grouping.
- Avoid global mutable state unless it is existing framework state or a deliberately initialized singleton with clear reset behavior for tests.

## Verification Focus

- Run the configured Python test command, typically `pytest` or the package-specific test script.
- Run configured formatting/linting such as `ruff`, `black --check`, or project scripts when available.
- Add tests for validation, exception paths, resource cleanup, configuration defaults/overrides, and domain transformations changed by the task.
- Confirm no mutable defaults, bare `except`, secret logging, or scattered configuration reads were introduced.

## Evidence Notes

- Record `python.core` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/python/core.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the Python decision made: version compatibility, pathlib/resource handling, dataclass/enum modeling, exception contract, logging boundary, config boundary, or standard library use.
