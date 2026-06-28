---
name: loom
description: Use when the user explicitly invokes @loom to route software delivery, knowledge, or deploy work through the Loom MCP server.
---

# loom

You are the Codex adapter for Loom MCP. Do not emulate Loom in chat and do not inspect project `.loom` state to decide the next step. Call the registered Loom MCP tools and treat their structured result as the workflow authority.

## Routing

Use the current workspace directory as `projectRoot`.

- `@loom <request>` or `@loom plan <request>` -> call `loom.plan`.
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

Do not stop at a recap while `state=auto_runnable`. If `continuationPolicy.mustContinue=true` or `continuationPolicy.progressReportAllowed=false`, do not emit a progress summary; keep following the returned action until one of the returned stop conditions is reached. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not search `.loom`, do not use ad hoc JSON selectors, and do not infer schema or submit parameters from old artifacts.

Read only the field groups needed for the current action. Use `loom.readRequestFields` only for declared fields inside the request read plan.

## Optional References

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action matches the trigger.

UIX references:
- `references/uix/core.md`: writing or reviewing user-visible frontend artifacts.
- `references/uix/interaction.md`: forms, flows, search/filter, loading, empty, error, or recovery states.
- `references/uix/system.md`: design system, tokens, components, icons, theming, motion, or localization.
- `references/uix/mobile.md`: mobile, tablet, responsive, PWA, or touch behavior.
- `references/uix/frameworks.md`: framework or component-library-specific frontend work.
- `references/uix/content.md`: UX writing, labels, empty states, errors, CTAs, onboarding, or terminology.
- `references/uix/data.md`: tables, dashboards, charts, analytics, research, or visualization-heavy screens.
- `references/uix/verification.md`: visual, interaction, accessibility, or screenshot-based verification.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Writing And Submit

Write artifacts only to the returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

## Boundaries

Do not copy field-level schemas into the plugin. Do not hard-code knowledge semantic fields, deployment stack rules, Brainstorm block contracts, architecture section schemas, or TaskResult schemas here. Those contracts come from the current MCP request.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
