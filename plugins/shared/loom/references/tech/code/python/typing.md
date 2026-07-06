# Python Typing Quality

Use this topic reference when `tech/code/python/typing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Python public APIs, dataclasses, protocols, typed dictionaries, validation boundaries, generics, callbacks, decorators, or type checker configuration.
- Use this when type annotations affect maintainability, integration contracts, or runtime validation.
- If the repository is intentionally untyped and the task only touches a small internal script, add focused annotations without forcing project-wide typing policy.

## Implementation Focus

- Annotate public functions, methods, class attributes, callbacks, and fixture factories touched by the task. Let obvious local variables infer unless annotation improves readability.
- Use `X | None` only when the supported Python version allows it. Otherwise follow the repository's existing `Optional[X]` style.
- Prefer `collections.abc` abstractions for inputs: `Sequence`, `Mapping`, `Iterable`, `Callable`, and `Iterator` where mutation is not required. Use concrete `list`/`dict` when callers may rely on mutation or concrete return shape.
- Use `Protocol` for structural seams such as storage, clock, HTTP client, repository, or notifier dependencies. Do not create inheritance-heavy abstract classes when a small protocol expresses the needed methods.
- Use `TypedDict` for dictionary-shaped external payloads only when the shape is stable and remains a dictionary at runtime. Use dataclasses, Pydantic models, or domain classes when validation and behavior are needed.
- Use `Literal` or enums for finite modes, statuses, and discriminants. Keep runtime validation aligned with the static finite set.
- Keep `Any` contained at external or legacy boundaries. Convert it to validated domain types before passing into business logic.
- Avoid broad `cast`, `type: ignore`, and untyped decorators. If a suppression is unavoidable, keep it local and include the narrow reason or error code expected by the repository style.
- Preserve callable signatures with `ParamSpec` only for decorators or wrappers where call-site type safety matters. Do not add advanced generics for one-off helpers.
- Type checking does not replace runtime validation for network, file, env, CLI, or user input boundaries.

## Verification Focus

- Run the configured type checker such as `mypy`, `pyright`, or the repository's validation script when available.
- Run tests for runtime validation, deserialization, and external payload mapping touched by typed changes.
- Confirm new annotations do not hide errors through `Any`, broad casts, or global ignore rules.
- For protocols and decorators, include usage through the typed seam so the checker exercises the intended call shape.

## Evidence Notes

- Record `python.typing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/python/typing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the typing decision: public API annotations, collections abstraction, protocol seam, TypedDict/dataclass choice, finite status, Any containment, or checker command.
