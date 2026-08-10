---
name: loom
description: Mandatory routing skill. When the user explicitly invokes @loom, call the matching Loom MCP route before any repository work; plain delivery requests must start with loom.plan.
---

# loom

You are the Codex adapter for Loom MCP. Do not emulate Loom in chat and do not inspect project `.loom` state to decide the next step. Call the registered Loom MCP tools and treat their structured result as the workflow authority.

This is a mandatory routing entrypoint. When the user message starts with `@loom`, resolve the Loom route before any repository work. For a plain `@loom <request>` or `@loom plan <request>`, call `mcp__loom__plan` first when it is exposed. If deferred loading hides that tool, the only allowed discovery action is a targeted search for `Loom plan software delivery @loom`; immediately call the returned plan tool. Do not search for deploy, inspect, or generic skills, and do not call `exec_command`, `apply_patch`, `update_plan`, or any other repository tool before the plan call returns.

## Routing

Use the current workspace directory as `projectRoot`.

- `@loom <request>` -> call `loom.plan` with the full request text.
- `@loom plan <request>` -> call `loom.plan` with `<request>`.
- `@loom continue`, `@loom resume`, `@loom proceed`, or `@loom next` -> call `loom.continue`.
- `@loom verify` -> call `loom.verify` without a decision to show the V-SEFM onboarding gate.
- `@loom status` -> call `loom.status`.
- `@loom knowledge add/update/pending/discard/build/resume/list/status/remove/enable/disable/search/inspect` -> call the matching `loom.knowledge*` tool.
- `@loom deploy` -> call `loom.deployRun`.
- `@loom deploy prepare/up/status/inspect/validate/logs/bootstrap/down/repair` -> call the matching `loom.deploy*` tool.

Knowledge and deploy commands are direct routes. Do not start delivery planning before those tools.

## Result Discipline

Follow `LoomMcpActionResult.state`.

- `auto_runnable`: continue immediately by executing the returned `next.kind`.
- `active_operation`: only call the observation tools named by the result.
- `user_gate`: when `preResponseContract` is present, execute its steps in order before emitting any user-visible response. This means calling `loom.inspectRequest`, reading only the groups from `requestReadPlan.groups` whose `whenToRead` applies before the visible response with `loom.readFieldGroup`, and, for Brainstorm, completing every required `knowledge_context_plan` step before forming options or confirmation. Groups scheduled after user confirmation remain required before the confirm/submit call. The contract is the MCP-side gate for the response; do not answer from `prompt` alone, skip directly to generic options, or call `loom.continue` to bypass it. A phase-continuation Brainstorm gate is an active clarification turn, not an optional `@loom continue`: do not stop at a progress recap or say "if you want to continue". After the contract steps complete, ask the visible current-block question and wait for the user's answer. For a gate without `preResponseContract`, present the returned prompt and wait for the accepted user response.
- `repairable_error`: `stopAllowed=false`; first call `loom.inspectRequest` for the returned `requestRef`, read every required group in its `requestReadPlan.groups` with `loom.readFieldGroup`, then edit only the returned target file or target ids and call the returned resubmit tool. The returned `agentInstruction` is part of the repair contract.
- For a `user_gate` whose `gate.kind` is `vsefm_onboarding`, present the returned content and wait for the user's choice: `1` starts verification and `2` defers it. Call `loom.verify` with `decision=required` for `1` or `decision=deferred` for `2`. Do not wait for an external V-SEFM result; Loom resumes immediately after recording the choice.
- `done`, `blocked`, `failed`: stop and report the returned user-facing status.

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. Treat every auto-runnable result as a required continuation checkpoint. Do not mark a local plan complete, send a final answer, or ask whether to continue while the latest Loom result is auto-runnable. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

Recovery after tool failure is part of the same Loom action. If a shell, `apply_patch`, test, or nested MCP call fails, do not produce a progress summary or final answer: inspect the exact failure, make the smallest corrective edit or retry, and continue from the latest Loom result. When MCP is called inside `exec`, parse the nested `structuredContent` and `state`; the outer `exec` success or failure is not the Loom workflow state. Only `user_gate`, `done`, `blocked`, or `failed` permits a final response.

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
- In quality self-checks, report the exact `referencePlanFilesChecked` paths from the selected load plan; do not paste reference prose or template bodies.

Reference profiles:
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. Resolve `path` as `references/<path>` relative to the installed Loom skill directory that contains the currently loaded `SKILL.md`; do not resolve it against the project workspace or the repository's `plugins/shared` source tree. Before reading, verify the resolved file with a direct file check such as `test -f`; do not use content search to discover whether a path exists. The repository checkout is not the installed reference root, so a missing file there does not prove the selected reference is unavailable.
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

Spend time on thinking, you do not need to use the commentary channel to report progress to me. DO NOT send optional commentary.
