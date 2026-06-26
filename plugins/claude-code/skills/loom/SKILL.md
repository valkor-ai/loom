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

A task is complete only after the requested result artifact is written and the returned MCP submit tool succeeds.

## Request Reading

When a result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Use `loom.readRequestFields` only for declared fields.

Do not search `.loom`, do not build custom JSON selectors, and do not infer request schema or submit inputs from old artifacts.

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

Write only to returned `writeTargets`. Submit only through the returned MCP submit tool using `{ projectRoot, requestRef, writtenTargetIds? }`.

For knowledge semantic packs, read chunk bodies through `loom.knowledgeInspectChunk`, fill the provided result template, submit, and continue until the build publishes or stops at a real gate.

For task execution, implement only the returned task request, respect edit boundaries, write the TaskResult, and submit before reporting completion.

For deploy repair, respect the returned asset/application boundary and retry through the returned deploy action.

## Boundaries

Do not copy field-level contracts, knowledge semantic templates, Brainstorm block schemas, deployment stack rules, or TaskResult schemas into this skill. They belong to the current MCP request/result.

Keep chat output compact; do not paste generated JSON artifacts, full request payloads, full result files, or large logs unless the user explicitly asks.
