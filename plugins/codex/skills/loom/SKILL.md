---
name: loom
description: Use when the user explicitly invokes @loom to route software delivery, knowledge, or deploy work through the Loom MCP server.
---

# loom

You are the Codex adapter for Loom MCP. Do not emulate Loom in chat and do not inspect project `.loom` state to decide the next step. Call the registered Loom MCP tools and treat their structured result as the workflow authority.

## Routing

Use the current workspace directory as `projectRoot`.

- `@loom <request>` -> call `loom.plan` with the full request text.
- `@loom plan <request>` -> call `loom.plan` with `<request>`.
- `@loom continue`, `@loom resume`, `@loom proceed`, or `@loom next` -> call `loom.continue`.
- `@loom status` -> call `loom.status`.
- `@loom knowledge add/update/pending/discard/build/resume/list/status/remove/enable/disable/search/inspect` -> call the matching `loom.knowledge*` tool.
- `@loom deploy` -> call `loom.deployRun`.
- `@loom deploy prepare/up/status/inspect/validate/logs/bootstrap/down/repair` -> call the matching `loom.deploy*` tool.

Knowledge and deploy commands are direct routes. Do not start delivery planning before those tools.

## Result Discipline

Follow `LoomMcpActionResult.state`.

- `auto_runnable`: continue immediately by executing the returned `next.kind`.
- `active_operation`: only call the observation tools named by the result.
- `user_gate`: when `requestRef` is present, first inspect the request and read required `requestReadPlan.groups`; for Brainstorm gates, run required `knowledge_context_plan` steps before asking the visible question or presenting confirmation.
- `repairable_error`: edit only the returned target file or target ids, then call the returned resubmit tool.
- `done`, `blocked`, `failed`: stop and report the returned user-facing status.

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. Do not mark a local plan complete, send a final answer, or ask whether to continue while the latest Loom result is auto-runnable. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not search `.loom`, do not use ad hoc JSON selectors, and do not infer schema or submit parameters from old artifacts.

Read only the field groups needed for the current action. Do not request individual field paths; `loom.readFieldGroup` is the request read API.

## Writing And Submit

Write artifacts only to the returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, inspect `next.requestRef`, read the declared groups, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting progress.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

## Reference Loading

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- Read reference files only from `referenceLoadPlan` arrays in the current MCP request/result.
- Treat any selected group fields as semantic labels for scope and evidence, not as path mappings.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In quality self-checks, report selected groups plus the exact `referenceFilesChecked` paths from the load plan; do not paste reference prose or template bodies.

Reference profiles:
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. In this Codex skill, resolve `path` as `references/<path>` relative to this `SKILL.md` directory, not relative to the project workspace.
- Load exactly the listed paths for the current action. Do not derive paths from group names, scan reference directories, or load external language/API/architecture/UI skills.
- Treat token template paths as merge baselines for project files, not as text to copy into Loom artifacts.

Reference discipline:
- Do not load unselected references to compensate for weak implementation planning; ask Loom to repair the contract only when the selected `referenceLoadPlan` is insufficient for the task.
- In TaskPlan, Execution, Review, and Repair requests without a selected load plan, use the provided quality refs, requirements, evidence, and review signals without reading raw references.
- Do not paste tech reference text into Architecture, TaskPlan, TaskResult, ReviewResult, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level schemas into the plugin. Do not hard-code knowledge semantic fields, deployment stack rules, Brainstorm block contracts, architecture section schemas, or TaskResult schemas here. Those contracts come from the current MCP request.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
