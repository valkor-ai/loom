# Loom Code Reference Common Rules

This file is selected for every `language_implementation_quality` requirement. It defines how code references are used in Loom; language/topic files define only the implementation details for their selected subject.

## Position In Loom

- These references are task-scoped implementation aids, not standalone skills and not technology selection documents.
- TechnicalBaseline remains the source of stack facts. Code references only refine how confirmed languages/frameworks should be implemented.
- Treat the current task's selected code references as the loading boundary; do not browse sibling language, framework, or database topics unless they are selected for that task.

## Repository Adaptation

- Start from the existing module layout, naming, formatter, dependency injection style, error contract, and test conventions.
- Extend a local abstraction when it already exists; do not introduce a parallel architecture for a single task.
- Add dependencies only when the task-owned behavior needs them and the repository has a clear dependency management path.
- Keep public contract changes aligned with API, UI, persistence, runtime, and verification artifacts.

## Delivery Rules

- Translate selected references into concrete edits. Do not paste reference prose into source files, delivery evidence, review findings, or user-facing UI.
- Keep domain behavior in domain/service code and keep transport/UI glue thin.
- Make invalid states hard to express where the language or framework supports that without excessive ceremony.
- Remove dead scaffolding after implementation; do not leave demo-only placeholders in production_code_implementation tasks.

## Verification Rules

- Run the smallest existing compile/type/lint/test command that proves the changed files.
- Add or update tests for new business branches, validation failures, persistence behavior, async lifecycle, or public API behavior touched by the task.
- Do not claim commands that were not run. Put blocked or unavailable verification in known gaps with the reason.
- Summarize command outcomes; do not paste large logs into delivery evidence.

## Evidence Rules

- Use the current result contract and validator messages for exact evidence field names and required values.
- Evidence summaries should state how changed files followed both repository style and the selected topic references.

## Cross-Cutting Ownership

- Load `tech/code/observability.md` only when the task owns a structured observability, request-tracing, async-processing, external-boundary, resilience, or sensitive-error concern. A word such as `log`, `logging`, `monitoring`, or `tracing` in task prose is not an ownership signal.
- The observability reference owns cross-stack event behavior, task-owned diagnostic boundaries, and deterministic language fallback when no framework overlay applies. A selected framework `logging.md` owns framework provider wiring and configuration mechanics, including async appenders, file output, rotation, compression, and retention. Deploy owns container topology and does not generate those settings.

## Common Anti-Patterns

- Loading sibling language/topic files outside the task-selected reference set.
- Reselecting the technology stack after TechnicalBaseline is accepted.
- Adding framework boilerplate without task-owned behavior.
- Silencing compiler, type, lint, or test failures to make delivery appear complete.
- Hardcoding secrets, ports, URLs, database paths, or environment-specific values.
