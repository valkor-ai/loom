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

After the tool returns, follow `LoomMcpActionResult.state`: continue immediately for `auto_runnable`; do not report progress or stop while `stopAllowed=false`; for `user_gate` with `requestRef`, inspect the request, read required `requestReadPlan.groups`, and run Brainstorm `knowledge_context_plan` steps before asking; repair only returned targets for `repairable_error`; stop only for `done`, `blocked`, or `failed`.

## Result Discipline

Do not stop at a recap while `state=auto_runnable` or `stopAllowed=false`. A task execution is complete only after the requested result artifact is written and its MCP submit tool succeeds.

For `active_operation`, call only the observation tools named by the result. For `repairable_error`, repair only the returned file or target ids, then call the returned resubmit tool.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Do not search `.loom`, do not use ad hoc JSON selectors, and do not infer request shape or submit parameters from old artifacts.

Read only the field groups needed for the current action. Do not request individual field paths; `loom.readFieldGroup` is the request read API.

## Writing And Submit

Write artifacts only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting progress.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

## Reference Loading

The current MCP request/result remains the authority. Optional references are installed under `../references/loom/`. Load no reference by default; load references only when the current action selects a reference profile.

Protocol:
- After reading the current request group, choose references only from the profiles selected by that request.
- `architectureQualitySeed.techReferenceProfile.groups`, `apiQualitySeed.techReferenceProfile.groups`, or another explicit `techReferenceProfile.groups` selector selects tech references.
- `uiQualityContract.referenceProfile.groups` selects UIX core, focus, scenario, token, stack, and template references when the current action creates, changes, or reviews user-visible frontend work.
- `uiQualityContract.designTokenAssetPlan.templateId` selects one token template item from `referenceProfile.groups.templates`. Treat it as a merge baseline for project files, not as text to copy into Loom artifacts.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In `frontendQualitySelfCheck`, report `referenceGroupsChecked` and concrete evidence from changed files; do not paste reference prose or template bodies.

MCP-selected references:

Reference profiles:
- Tech references are selected only by `techReferenceProfile.groups`; map selected group/items to the exact files below. Do not scan the whole `../references/loom/tech` tree and do not load external architecture or API skills.
- UIX references are selected only by `uiQualityContract.referenceProfile.groups`; map selected group/items to exact files below. Do not scan the whole `../references/loom/uix` tree.

Tech architecture reference map:
- `techReferenceProfile.groups.arch` item `core` -> `../references/loom/tech/arch/core.md`.
- `techReferenceProfile.groups.arch` item `patterns` -> `../references/loom/tech/arch/patterns.md`.
- `techReferenceProfile.groups.arch` item `system` -> `../references/loom/tech/arch/system.md`.
- `techReferenceProfile.groups.arch` item `data` -> `../references/loom/tech/arch/data.md`.
- `techReferenceProfile.groups.arch` item `nfr` -> `../references/loom/tech/arch/nfr.md`.
- `techReferenceProfile.groups.arch` item `adr` -> `../references/loom/tech/arch/adr.md`.
- `techReferenceProfile.groups.arch` item `failure` -> `../references/loom/tech/arch/failure.md`.

Tech API reference map:
- `techReferenceProfile.groups.api` item `core` -> `../references/loom/tech/api/core.md`.
- `techReferenceProfile.groups.api` item `resource` -> `../references/loom/tech/api/resource.md`.
- `techReferenceProfile.groups.api` item `errors` -> `../references/loom/tech/api/errors.md`.
- `techReferenceProfile.groups.api` item `pagination` -> `../references/loom/tech/api/pagination.md`.
- `techReferenceProfile.groups.api` item `contract` -> `../references/loom/tech/api/contract.md`.
- `techReferenceProfile.groups.api` item `security` -> `../references/loom/tech/api/security.md`.
- `techReferenceProfile.groups.api` item `evolution` -> `../references/loom/tech/api/evolution.md`.
- `techReferenceProfile.groups.api` item `operations` -> `../references/loom/tech/api/operations.md`.

UIX reference map:
- `groups.core`: `core` -> `../references/loom/uix/core.md`; `anti-patterns` -> `../references/loom/uix/anti-patterns.md`; `system`, `interaction`, `content`, `verification` -> matching top-level files under `../references/loom/uix/`.
- `groups.focus`: `data`, `mobile`, `frameworks` -> matching top-level files under `../references/loom/uix/`.
- `groups.tokens`: `color-system`, `typography`, `spacing`, `layout-grid`, `motion`, `radius-elevation` -> matching files under `../references/loom/uix/tokens/`.
- `groups.scenarios`: scenario items such as `admin-dashboard`, `data-console`, `docs-site` -> matching files under `../references/loom/uix/scenarios/`.
- `groups.stacks`: stack items such as `react`, `vue`, `plain-html`, `native-mobile`, `threejs`, `svelte`, `uniapp` -> matching files under `../references/loom/uix/stacks/`.
- `groups.templates`: `tokens-css` -> `../references/loom/uix/templates/tokens.css.tpl`; `tokens-tailwind` -> `../references/loom/uix/templates/tokens.tailwind.tpl`.

Reference discipline:
- Focus references are contract-selected group/items, not fallback reading. Load a focus file only when its item appears in `referenceProfile.groups.focus`.
- If the contract selects companion scenario items such as `data-console` and `admin-dashboard`, read both and apply the more specific rule to each surface.
- Do not load unselected UIX files to compensate for weak implementation planning; ask Loom to repair the contract only when selected references are insufficient for the task.
- For tech references, load only selected group/items from `techReferenceProfile.groups`; never expand `arch` into `api`, or `api` into `arch`, `stack`, `code`, or `test` references unless the MCP contract selects them. In TaskPlan, Execution, and Review requests without a selected tech profile, use the provided quality refs, requirements, evidence, and review signals without reading raw tech references.
- Do not paste tech reference text into Architecture, TaskPlan, TaskResult, ReviewResult, source files, or user-facing UI. Use references to produce concrete decisions, interface contracts, NFRs, risks, and evidence.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block contracts, deployment stack rules, architecture section contracts, or TaskResult contracts into this command. They belong to the current MCP request/result.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
