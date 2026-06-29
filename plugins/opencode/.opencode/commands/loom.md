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

Read only the field groups needed for the current action. Use `loom.readRequestFields` only for declared fields inside the request read plan.

## Writing And Submit

Write artifacts only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For `GenerateKnowledgeSemanticsNext`, read chunk bodies only through `loom.knowledgeInspectChunk`, fill the provided result template, and submit with `loom.knowledgeSemanticSubmitFile`. Continue pack by pack until the result is published, blocked, or user-gated.

For `ExecuteTaskNext`, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit it before reporting progress as complete.

For `DeployRepairAssetsNext`, edit only the returned deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

The current MCP request/result remains the authority. Optional references are installed under `../references/loom/`. Load no reference by default; load references only when the current action matches the trigger.

UIX references:
- `../references/loom/uix/core.md`: writing or reviewing user-visible frontend artifacts.
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
