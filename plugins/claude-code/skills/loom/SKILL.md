---
name: loom
description: Use when the user invokes /loom to route software delivery, knowledge, or deploy work through the Loom MCP server.
---

# loom

You are the Claude Code adapter for Loom MCP. The `/loom` command chooses the first Loom MCP tool. After that, the current `LoomMcpActionResult` is the workflow authority.

Do not replace Loom with Claude Plan Mode. Do not inspect project `.loom` state to decide the next step. Use Loom MCP tools and resources.

## Result Discipline

- `auto_runnable`: keep going immediately by executing the returned `next.kind`.
- `active_operation`: only use the observation tools named by the result.
- `user_gate`: when `preResponseContract` is present, execute its steps in order before emitting any user-visible response. This means calling `loom.inspectRequest`, reading only the groups from `requestReadPlan.groups` whose `whenToRead` applies before the visible response with `loom.readFieldGroup`, and, for Brainstorm, completing every required `knowledge_context_plan` step before forming options or confirmation. Groups scheduled after user confirmation remain required before the confirm/submit call. The contract is the MCP-side gate for the response; do not answer from `prompt` alone, skip directly to generic options, or call `/loom continue` to bypass it. A phase-continuation Brainstorm gate is an active clarification turn, not an optional `/loom continue`: do not stop at a progress recap or say "if you want to continue". After the contract steps complete, ask the visible current-block question and wait for the user's answer. For a gate without `preResponseContract`, present the returned prompt and wait for the accepted user response.
- `repairable_error`: `stopAllowed=false`; first call `loom.inspectRequest` for the returned `requestRef` and read every required group in `requestReadPlan.groups` with `loom.readFieldGroup`, then repair only the returned target and resubmit with the returned tool. The returned `agentInstruction` is part of the repair contract.
- `done`, `blocked`, `failed`: report the returned status and stop.

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. Treat every auto-runnable result as a required continuation checkpoint. Do not mark a local plan complete, send a final answer, or ask whether to continue while the latest Loom result is auto-runnable. A task is complete only after the requested result artifact is written and the returned MCP submit tool succeeds.

Recovery after tool failure is part of the same Loom action. If a shell, patch, test, or nested MCP call fails, inspect the exact failure and retry the smallest corrective step; do not produce a progress summary or final answer. When MCP is called through a wrapper, parse the nested structured result and its `state`; the wrapper's success or failure is not the Loom workflow state. Only `user_gate`, `done`, `blocked`, or `failed` permits a final response.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not request individual field paths.

Do not search `.loom`, do not build custom JSON selectors, and do not infer request schema or submit inputs from old artifacts.

## Writing And Submit

Write only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the build publishes, blocks, or reaches a real user gate.

For task execution, inspect `next.requestRef`, read the declared groups, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit before reporting completion.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting completion.

For deploy repair, respect the returned asset/application boundary and retry through the returned deploy action.

## Reference Loading

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- Read reference files only from `referenceLoadPlan` arrays in the current MCP request/result.
- Treat any selected group fields as semantic labels for scope and evidence, not as path mappings.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In quality self-checks, report the exact `referencePlanFilesChecked` paths from the selected load plan; do not paste reference prose or template bodies.

Reference profiles:
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. In this Claude Code skill, resolve `path` as `references/<path>` relative to this `SKILL.md` directory, not relative to the project workspace.
- Load exactly the listed paths for the current action. Do not derive paths from group names, scan reference directories, or load external language/API/architecture/UI skills.
- Treat token template paths as merge baselines for project files, not as text to copy into Loom artifacts.

Reference discipline:
- Do not load unselected references to compensate for weak implementation planning; ask Loom to repair the contract only when the selected `referenceLoadPlan` is insufficient for the task.
- In TaskPlan, Execution, Review, and Repair requests without a selected load plan, use the provided quality refs, requirements, evidence, and review signals without reading raw references.
- Do not paste tech reference text into Architecture, TaskPlan, TaskResult, ReviewResult, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block schemas, deployment stack rules, or TaskResult schemas into this skill. They belong to the current MCP request/result.

Keep chat output compact; do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks.
