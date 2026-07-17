---
description: Route Loom delivery, knowledge, and deploy commands through MCP.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
---

# loom

You are executing `/loom $ARGUMENTS` now.

Call the matching Loom MCP tool for the current project directory before doing any other work.

- `status` -> `loom.status`
- `continue`, `resume`, `proceed`, `next`, or empty -> `loom.continue`
- `knowledge ...` -> matching `loom.knowledge*` tool
- `deploy` -> `loom.deployRun`
- `deploy ...` -> matching `loom.deploy*` tool
- `plan <request>` -> `loom.plan` with `<request>`
- Any other request text -> `loom.plan` with the full request text

After the tool returns, follow `LoomMcpActionResult.state`: continue immediately for `auto_runnable`; do not report progress, mark a local plan complete, send a final answer, or stop while `stopAllowed=false`; for `user_gate` with `preResponseContract`, execute its steps in order before emitting any user-visible response: call `loom.inspectRequest`, read only groups whose `whenToRead` applies before the visible response from `requestReadPlan.groups` with `loom.readFieldGroup`, and run every Brainstorm `knowledge_context_plan` step before forming options or confirmation. Groups scheduled after user confirmation remain required before the confirm/submit call. Do not answer from `prompt` alone, skip to generic options, or call `/loom continue` to bypass the contract. A phase-continuation Brainstorm gate is an active clarification turn, not an optional `/loom continue`; do not stop at a progress recap or say "if you want to continue". Complete the reads and knowledge calls, then ask the visible current-block question and wait for the user's answer. For a gate without `preResponseContract`, present the returned prompt and wait for the accepted response. Repair only returned targets for `repairable_error`; stop only for `done`, `blocked`, or `failed`.

## Result Discipline

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. Do not mark a local plan complete, send a final answer, or ask whether to continue while the latest Loom result is auto-runnable. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

For `active_operation`, call only the observation tools named by the result. For `repairable_error`, repair only the returned file or target ids, then call the returned resubmit tool.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not search `.loom`, do not use ad hoc JSON selectors, and do not infer request shape or submit parameters from old artifacts.

Read only the field groups needed for the current action. Do not request individual field paths; `loom.readFieldGroup` is the request read API.

## Writing And Submit

Write artifacts only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, inspect `next.requestRef`, read the declared groups, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting progress.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

## Reference Loading

The current MCP request/result remains the authority. Optional references are installed under `../references/loom/`. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- Read reference files only from `referenceLoadPlan` arrays in the current MCP request/result.
- Treat any selected group fields as semantic labels for scope and evidence, not as path mappings.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In quality self-checks, report the exact `referencePlanFilesChecked` paths from the selected load plan; do not paste reference prose or template bodies.

Reference profiles:
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. Resolve `path` relative to `../references/loom/` from this command/plugin location, not relative to the project workspace.
- Load exactly the listed paths for the current action. Do not derive paths from group names, scan reference directories, or load external language/API/architecture/UI skills.
- Treat token template paths as merge baselines for project files, not as text to copy into Loom artifacts.

Reference discipline:
- Do not load unselected references to compensate for weak implementation planning; ask Loom to repair the contract only when the selected `referenceLoadPlan` is insufficient for the task.
- In TaskPlan, Execution, Review, and Repair requests without a selected load plan, use the provided quality refs, requirements, evidence, and review signals without reading raw references.
- Do not paste tech reference text into Architecture, TaskPlan, TaskResult, ReviewResult, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block contracts, deployment stack rules, architecture section contracts, or TaskResult contracts into this command. They belong to the current MCP request/result.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
