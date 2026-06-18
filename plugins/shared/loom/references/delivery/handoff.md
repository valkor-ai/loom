# Handoff Discipline

Use this reference before final responses, blocked reports, handoff notes, or continuation summaries that need durable evidence.

## Preserve continuation state

- Reference existing Loom refs, result files, logs, commits, preview evidence, and verification evidence instead of restating large artifacts in chat.
- Keep the next action tied to the latest `instruction` or CLI route.
- Do not claim completion until the required result file exists and the submit command has succeeded.

## Report compactly

- Summarize what changed, what was verified, and what remains.
- Include exact blockers only when the next action needs user input, environment access, or a non-repairable decision.
- Avoid pasting generated JSON, full diffs, full logs, or full `.loom` artifacts unless the user explicitly asks.

## Prepare the next agent

- Name the source of truth for the next step: `continue`, a specific repair request, a result artifact, or a blocker.
- Mention residual risks as concrete checks, not vague warnings.
- If work is incomplete, record the partial state in the Loom result path before asking the user to continue later.
