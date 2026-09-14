---
name: loom
description: Mandatory routing skill. When the user explicitly invokes @loom, call the matching Loom MCP route before any repository work; plain delivery requests must start with loom.plan.
---

# loom

Use Loom MCP as the workflow authority. Do not inspect `.loom` directly, invent a workflow in chat, or touch repository files before the route below returns.

This is the mandatory routing entrypoint. When a user invokes `@loom`, resolve the Loom route before any repository work. For `@loom <request>` or `@loom plan <request>`, call `mcp__loom__plan` when it is available. If deferred loading hides that tool, make one targeted search for `Loom plan software delivery @loom`, then call the returned plan tool. Do not search for deploy, inspect, or generic skills, and do not call `exec_command`, `apply_patch`, or other repository tools before the plan call returns.

## Route

Use the current workspace as `projectRoot`.

- `@loom <request>`: call `loom.plan` with `{ projectRoot, requestText: "<full request>" }`.
- `@loom plan <request>`: same call with `<request>`.
- `@loom continue|resume|proceed|next`: call `loom.continue`.
- `@loom verify`, `@loom status`, `@loom knowledge ...`, and `@loom deploy ...`: call the matching Loom tool directly.
- If the plan tool is deferred, make one targeted search for `Loom plan software delivery @loom`, then call the returned plan tool. Do not do other discovery first.
- For a plan-conflict gate, present its choices exactly and resolve it with `loom.planConflictResolve`; never call `loom.plan` again.

## Follow Results

- `auto_runnable`: perform the returned `next` action immediately.
- `active_operation`: use only the named observation tools.
- `user_gate`: follow `preResponseContract` in order before replying. For Brainstorm, read the applicable groups, run every required knowledge step, present the current question, then wait for the user. A phase continuation is an active clarification, not an optional next step.
- `repairable_error`: inspect the request, read every required group, change only the returned target, and use its resubmit tool. Do not ask the user to repeat a confirmation.
- V-SEFM onboarding: wait for the user's choice, then call `loom.verify` with `decision=required` or `decision=deferred`.
- `done`, `blocked`, or `failed`: stop and report it. Do not stop while a result is auto-runnable or `stopAllowed=false`.

## Read, Write, Verify

When a result has `requestRef`, use `loom.inspectRequest` and only the declared `loom.readFieldGroup` groups. `requestReadPlan.groups` is the only read contract. The request is the source of schemas, read boundaries, and submit parameters; do not request individual fields or infer inputs from old artifacts.

Write only returned `writeTargets`, then submit with the returned tool. For execution, respect edit boundaries, run the required checks, write the TaskResult, and submit it before saying work is complete. If an operation fails, inspect and repair it in the same Loom action.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the build publishes, blocks, or reaches a real user gate.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting completion.

## Reference Loading

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- Read reference files only from `referenceLoadPlan` arrays in the current MCP request/result.
- Treat selected group fields as semantic labels for scope and evidence, not as path mappings.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In quality self-checks, report the exact `referencePlanFilesChecked` paths from the selected load plan; do not paste reference prose or template bodies.

Reference profiles:
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. Resolve `path` under the installed Loom skill's `references/` directory. Do not resolve it against the project workspace or source checkout. Before reading, verify the resolved file directly.
- Load exactly the listed paths for the current action. Do not derive paths from group names, scan reference directories, or load external language, API, architecture, or UI skills.
- Treat token template paths as merge baselines for project files, not as text to copy into Loom artifacts.

Reference discipline:
- Do not load unselected references to compensate for weak planning; ask Loom to repair the contract only when the selected `referenceLoadPlan` is insufficient.
- In TaskPlan, Execution, Review, and Repair requests without a selected load plan, use the provided quality refs, requirements, evidence, and review signals without reading raw references.
- Do not paste tech reference text into artifacts, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Keep user-facing responses compact. Do not paste raw artifacts, request payloads, or logs unless asked.
