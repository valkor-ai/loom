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
- `user_gate`: when `requestRef` is present, first inspect the request and read required `requestReadPlan.groups`; for Brainstorm gates, run required `knowledge_context_plan` steps before asking the visible question or presenting confirmation.
- `repairable_error`: repair only the returned target and resubmit with the returned tool.
- `done`, `blocked`, `failed`: report the returned status and stop.

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. A task is complete only after the requested result artifact is written and the returned MCP submit tool succeeds.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not request individual field paths.

Do not search `.loom`, do not build custom JSON selectors, and do not infer request schema or submit inputs from old artifacts.

## Writing And Submit

Write only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the build publishes, blocks, or reaches a real user gate.

For task execution, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit before reporting completion.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting completion.

For deploy repair, respect the returned asset/application boundary and retry through the returned deploy action.

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

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block schemas, deployment stack rules, or TaskResult schemas into this skill. They belong to the current MCP request/result.

Keep chat output compact; do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks.
