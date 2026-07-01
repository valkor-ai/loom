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

The current MCP request/result remains the authority. Optional references are installed under `../references/loom/`. Load no reference by default; load references only when the current action matches the trigger.

Protocol:
- Read `../references/loom/uix/core.md` only when the current action creates, changes, or reviews user-visible frontend work.
- After reading the current request group, map `uiQualityContract.referenceProfile.referenceIds` to the exact scenario, token, and stack reference files named below. Do not scan the whole `../references/loom/uix` tree.
- When `uiQualityContract.designTokenAssetPlan.templateId` is present, load only the matching token template file. Treat it as a merge baseline for project files, not as text to copy into Loom artifacts.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In `frontendQualitySelfCheck`, report `referenceIdsChecked` and concrete evidence from changed files; do not paste reference prose or template bodies.

MCP-selected references:
- `uix.core` -> `../references/loom/uix/core.md`.
- `uix.anti-patterns` -> `../references/loom/uix/anti-patterns.md`.
- `uix.tokens.color-system`, `uix.tokens.typography`, `uix.tokens.spacing`, `uix.tokens.layout-grid`, `uix.tokens.motion`, `uix.tokens.radius-elevation` -> the matching file under `../references/loom/uix/tokens/`.
- `uix.scenarios.admin-dashboard`, `uix.scenarios.data-console`, `uix.scenarios.fintech-workstation`, `uix.scenarios.fintech-consumer-app`, `uix.scenarios.consumer-app`, `uix.scenarios.mobile-responsive`, `uix.scenarios.mobile-native`, `uix.scenarios.marketing-site`, `uix.scenarios.corporate-site`, `uix.scenarios.docs-site`, `uix.scenarios.developer-tool`, `uix.scenarios.immersive-3d` -> the matching file under `../references/loom/uix/scenarios/`.
- `uix.stacks.react`, `uix.stacks.vue`, `uix.stacks.plain-html`, `uix.stacks.native-mobile`, `uix.stacks.threejs`, `uix.stacks.svelte`, `uix.stacks.uniapp` -> the matching file under `../references/loom/uix/stacks/`.
- `uix.templates.tokens-css` -> `../references/loom/uix/templates/tokens.css.tpl`; `uix.templates.tokens-tailwind` -> `../references/loom/uix/templates/tokens.tailwind.tpl`.

Fallback focused references:
- These files are not substitutes for MCP-selected `referenceProfile.referenceIds`; use them only when the current action needs that focus and the MCP contract has not selected a more precise scenario/token/stack reference.
- `../references/loom/uix/interaction.md`: forms, flows, search/filter, loading, empty, error, or recovery states.
- `../references/loom/uix/system.md`: design system, tokens, components, icons, theming, motion, or localization.
- `../references/loom/uix/mobile.md`: mobile, tablet, responsive, PWA, or touch behavior.
- `../references/loom/uix/frameworks.md`: framework or component-library-specific frontend work.
- `../references/loom/uix/content.md`: UX writing, labels, empty states, errors, CTAs, onboarding, or terminology.
- `../references/loom/uix/data.md`: tables, dashboards, charts, analytics, research, or visualization-heavy screens.
- `../references/loom/uix/verification.md`: visual, interaction, accessibility, or screenshot-based verification.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block contracts, deployment stack rules, architecture section contracts, or TaskResult contracts into this command. They belong to the current MCP request/result.

Keep user-visible output compact. Do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks to inspect them.
