# Loom Code Reference Common Rules

This file is selected for every `language_implementation_quality` requirement. It defines how code references are used in Loom; language/topic files define only the implementation details for their selected subject.

## Position In Loom

- These references are task-scoped implementation aids, not standalone skills and not technology selection documents.
- TechnicalBaseline remains the source of stack facts. Code references only refine how confirmed languages/frameworks should be implemented.
- MCP `referenceLoadPlan` is the only loading authority. `referenceGroups` are evidence labels, not file maps.
- Load this common file plus only the topic files listed in the current task's `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

## Repository Adaptation

- Start from the existing module layout, naming, formatter, dependency injection style, error contract, and test conventions.
- Extend a local abstraction when it already exists; do not introduce a parallel architecture for a single task.
- Add dependencies only when the task-owned behavior needs them and the repository has a clear dependency management path.
- Keep public contract changes aligned with API, UI, persistence, runtime, and verification artifacts.

## Delivery Rules

- Translate selected references into concrete edits. Do not paste reference prose into source files, TaskResult, ReviewResult, or user-facing UI.
- Keep domain behavior in domain/service code and keep transport/UI glue thin.
- Make invalid states hard to express where the language or framework supports that without excessive ceremony.
- Remove dead scaffolding after implementation; do not leave demo-only placeholders in production_code_implementation tasks.

## Verification Rules

- Run the smallest existing compile/type/lint/test command that proves the changed files.
- Add or update tests for new business branches, validation failures, persistence behavior, async lifecycle, or public API behavior touched by the task.
- Do not claim commands that were not run. Put blocked or unavailable verification in known gaps with the reason.
- Summarize command outcomes; do not paste large logs into TaskResult.

## Evidence Rules

- `codeQualityEvidence.referenceGroupsChecked` must match the selected language/topic groups.
- `codeQualityEvidence.referenceFilesChecked` must include `tech/code/common.md` and every selected topic path that was read.
- Link evidence to exact `task.verificationIntents[].verificationId` values.
- Summaries should state how changed files followed both repository style and the selected topic references.

## Common Anti-Patterns

- Loading sibling language/topic files that are not listed in `referenceLoadPlan`.
- Reselecting the technology stack after TechnicalBaseline is accepted.
- Adding framework boilerplate without task-owned behavior.
- Silencing compiler, type, lint, or test failures to make delivery appear complete.
- Hardcoding secrets, ports, URLs, database paths, or environment-specific values.
