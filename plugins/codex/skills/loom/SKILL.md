---
name: loom
description: Mandatory routing skill. When the user explicitly invokes @loom, call the matching Loom MCP route before any repository work; plain delivery requests must start with loom.plan.
---

# loom

Use Loom MCP as the workflow authority. Do not inspect `.loom` directly, invent a workflow in chat, or touch repository files before the route below returns.

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

When a result has `requestRef`, use `loom.inspectRequest` and only the declared `loom.readFieldGroup` groups. The request is the source of schemas, read boundaries, and submit parameters.

Write only returned `writeTargets`, then submit with the returned tool. For execution, respect edit boundaries, run the required checks, write the TaskResult, and submit it before saying work is complete. If an operation fails, inspect and repair it in the same Loom action.

Load references only when the current request selects a `referenceLoadPlan`. Resolve its paths under the installed Loom skill's `references/` directory, load exactly those files, and use them as guidance rather than copying their prose into artifacts.

Keep user-facing responses compact. Do not paste raw artifacts, request payloads, or logs unless asked.
