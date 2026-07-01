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

The current MCP request/result remains the authority. Load no reference by default; load references only when the current action matches the trigger.

Protocol:
- Read `references/uix/core.md` only when the current action creates, changes, or reviews user-visible frontend work.
- After reading the current request group, map `uiQualityContract.referenceProfile.referenceIds` to the exact scenario, token, and stack reference files named below. Do not scan the whole `references/uix` tree.
- When `uiQualityContract.designTokenAssetPlan.templateId` is present, load only the matching token template file. Treat it as a merge baseline for project files, not as text to copy into Loom artifacts.
- If a referenced file is not selected by the MCP contract and is not needed by the current action, leave it unread.
- In `frontendQualitySelfCheck`, report `referenceIdsChecked` and concrete evidence from changed files; do not paste reference prose or template bodies.

MCP-selected references:
- `uix.core` -> `references/uix/core.md`.
- `uix.anti-patterns` -> `references/uix/anti-patterns.md`.
- `uix.tokens.color-system`, `uix.tokens.typography`, `uix.tokens.spacing`, `uix.tokens.layout-grid`, `uix.tokens.motion`, `uix.tokens.radius-elevation` -> the matching file under `references/uix/tokens/`.
- `uix.scenarios.admin-dashboard`, `uix.scenarios.data-console`, `uix.scenarios.fintech-workstation`, `uix.scenarios.fintech-consumer-app`, `uix.scenarios.consumer-app`, `uix.scenarios.mobile-responsive`, `uix.scenarios.mobile-native`, `uix.scenarios.marketing-site`, `uix.scenarios.corporate-site`, `uix.scenarios.docs-site`, `uix.scenarios.developer-tool`, `uix.scenarios.immersive-3d` -> the matching file under `references/uix/scenarios/`.
- `uix.stacks.react`, `uix.stacks.vue`, `uix.stacks.plain-html`, `uix.stacks.native-mobile`, `uix.stacks.threejs`, `uix.stacks.svelte`, `uix.stacks.uniapp` -> the matching file under `references/uix/stacks/`.
- `uix.templates.tokens-css` -> `references/uix/templates/tokens.css.tpl`; `uix.templates.tokens-tailwind` -> `references/uix/templates/tokens.tailwind.tpl`.

Fallback focused references:
- These files are not substitutes for MCP-selected `referenceProfile.referenceIds`; use them only when the current action needs that focus and the MCP contract has not selected a more precise scenario/token/stack reference.
- `references/uix/interaction.md`: forms, flows, search/filter, loading, empty, error, or recovery states.
- `references/uix/system.md`: design system, tokens, components, icons, theming, motion, or localization.
- `references/uix/mobile.md`: mobile, tablet, responsive, PWA, or touch behavior.
- `references/uix/frameworks.md`: framework or component-library-specific frontend work.
- `references/uix/content.md`: UX writing, labels, empty states, errors, CTAs, onboarding, or terminology.
- `references/uix/data.md`: tables, dashboards, charts, analytics, research, or visualization-heavy screens.
- `references/uix/verification.md`: visual, interaction, accessibility, or screenshot-based verification.

Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result. Do not load separate delivery reference files.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block schemas, deployment stack rules, or TaskResult schemas into this skill. They belong to the current MCP request/result.

Keep chat output compact; do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks.
