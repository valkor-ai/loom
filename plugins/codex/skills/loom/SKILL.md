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

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not search `.loom`, do not use ad hoc JSON selectors, and do not infer schema or submit parameters from old artifacts.

Read only the field groups needed for the current action. Do not request individual field paths; `loom.readFieldGroup` is the request read API.

## Writing And Submit

Write artifacts only to the returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting progress.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

## Reference Loading

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- `architectureQualitySeed.techReferenceProfile.groups`, `apiQualitySeed.techReferenceProfile.groups`, or another explicit `techReferenceProfile.groups` selector selects tech references.
- `uiQualityContract.referenceProfile.groups` selects UIX core, focus, scenario, token, stack, and template references when the current action creates, changes, or reviews user-visible frontend work.
- `uiQualityContract.designTokenAssetPlan.templateId` selects one token template item from `referenceProfile.groups.templates`. Treat it as a merge baseline for project files, not as text to copy into Loom artifacts.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In `frontendQualitySelfCheck`, report `referenceGroupsChecked` and concrete evidence from changed files; do not paste reference prose or template bodies.

MCP-selected references:

Reference profiles:
- Tech references are selected only by `techReferenceProfile.groups`; map selected group/items to the exact files below. Do not scan the whole `references/tech` tree and do not load external architecture or API skills.
- UIX references are selected only by `uiQualityContract.referenceProfile.groups`; map selected group/items to exact files below. Do not scan the whole `references/uix` tree.

Tech architecture reference map:
- `techReferenceProfile.groups.arch` item `core` -> `references/tech/arch/core.md`.
- `techReferenceProfile.groups.arch` item `patterns` -> `references/tech/arch/patterns.md`.
- `techReferenceProfile.groups.arch` item `system` -> `references/tech/arch/system.md`.
- `techReferenceProfile.groups.arch` item `data` -> `references/tech/arch/data.md`.
- `techReferenceProfile.groups.arch` item `nfr` -> `references/tech/arch/nfr.md`.
- `techReferenceProfile.groups.arch` item `adr` -> `references/tech/arch/adr.md`.
- `techReferenceProfile.groups.arch` item `failure` -> `references/tech/arch/failure.md`.

Tech API reference map:
- `techReferenceProfile.groups.api` item `core` -> `references/tech/api/core.md`.
- `techReferenceProfile.groups.api` item `resource` -> `references/tech/api/resource.md`.
- `techReferenceProfile.groups.api` item `errors` -> `references/tech/api/errors.md`.
- `techReferenceProfile.groups.api` item `pagination` -> `references/tech/api/pagination.md`.
- `techReferenceProfile.groups.api` item `contract` -> `references/tech/api/contract.md`.
- `techReferenceProfile.groups.api` item `security` -> `references/tech/api/security.md`.
- `techReferenceProfile.groups.api` item `evolution` -> `references/tech/api/evolution.md`.
- `techReferenceProfile.groups.api` item `operations` -> `references/tech/api/operations.md`.

UIX reference map:
- `groups.core`: `core` -> `references/uix/core.md`; `anti-patterns` -> `references/uix/anti-patterns.md`; `system`, `interaction`, `content`, `verification` -> matching top-level files under `references/uix/`.
- `groups.focus`: `data`, `mobile`, `frameworks` -> matching top-level files under `references/uix/`.
- `groups.tokens`: `color-system`, `typography`, `spacing`, `layout-grid`, `motion`, `radius-elevation` -> matching files under `references/uix/tokens/`.
- `groups.scenarios`: scenario items such as `admin-dashboard`, `data-console`, `docs-site` -> matching files under `references/uix/scenarios/`.
- `groups.stacks`: stack items such as `react`, `vue`, `plain-html`, `native-mobile`, `threejs`, `svelte`, `uniapp` -> matching files under `references/uix/stacks/`.
- `groups.templates`: `tokens-css` -> `references/uix/templates/tokens.css.tpl`; `tokens-tailwind` -> `references/uix/templates/tokens.tailwind.tpl`.

Reference discipline:
- Focus references are contract-selected group/items, not fallback reading. Load a focus file only when its item appears in `referenceProfile.groups.focus`.
- If the contract selects companion scenario items such as `data-console` and `admin-dashboard`, read both and apply the more specific rule to each surface.
- Do not load unselected UIX files to compensate for weak implementation planning; ask Loom to repair the contract only when selected references are insufficient for the task.
- For tech references, load only selected group/items from `techReferenceProfile.groups`; never expand `arch` into `api`, or `api` into `arch`, `stack`, `code`, or `test` references unless the MCP contract selects them. In TaskPlan, Execution, and Review requests without a selected tech profile, use the provided quality refs, requirements, evidence, and review signals without reading raw tech references.
- Do not paste tech reference text into Architecture, TaskPlan, TaskResult, ReviewResult, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level schemas into the plugin. Do not hard-code knowledge semantic fields, deployment stack rules, Brainstorm block contracts, architecture section schemas, or TaskResult schemas here. Those contracts come from the current MCP request.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
