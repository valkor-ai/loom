# Frontend UIX Core

Load this file whenever a Loom request includes user-visible frontend work, `frontend_experience`, `frontendExperienceRequirement`, `uiQualityContract`, frontend review signals, or a task that creates/modifies screens. Loom MCP artifacts are the source of truth; these references turn that contract into concrete implementation decisions.

This reference set is a Loom-owned rewrite. It absorbs proven UI craft categories such as scenario fit, token systems, state coverage, stack execution, anti-pattern detection, and rendered verification without copying another skill's text or requiring a second skill to be loaded.

## Routing Contract

Load UIX references only from the current MCP request's `referenceLoadPlan` entries. Do not derive file paths from group names, scenario names, stack names, token names, or this reference file. Selected groups are evidence labels only; `referenceLoadPlan[].path` is the loading authority.

Do not load unrelated UI skills or scan the UIX reference tree. Do not copy reference text into task results. Cite loaded group/items in `frontendQualitySelfCheck.referenceGroupsChecked`, list exact loaded paths in `frontendQualitySelfCheck.referenceFilesChecked`, then provide concrete evidence from the implemented UI.

## Contract Chain

Loom UIX must work as one chain, not as optional reading:

1. Architecture writes `uiQualityContract` from the confirmed frontend target and repository evidence. It selects compact reference groups and a token asset plan; it does not copy reference prose.
2. TaskPlan copies the architecture `uiQualityContract` exactly into every task that owns visible UI surfaces, workflows, states, bindings, or frontend foundations.
3. Execution reads only the selected group/items, adapts the token template to the actual project stack/theme, and implements the business surface.
4. TaskResult fills `frontendQualitySelfCheck` with changed UI files, states, actions, token asset files, token consumer files, viewports, and known gaps.
5. Review checks the rendered product surface and the evidence; it must not accept a prose-only claim of UI quality.

If a task creates or changes a page, route, screen, panel, modal, drawer, table, form, chart, navigation, shared component, or visible state, it is UI work even when it is not the app first viewport.

## Reference Compression Rule

Loom UIX references are not meant to be tiny summaries, and they are not meant to become a full copied design-code library. Keep the parts that change implementation behavior:

- Keep scenario-specific layout skeletons, component anatomy, responsive breakpoints, state patterns, accessibility requirements, and verification expectations.
- Keep compact code-like examples when they explain structure that agents commonly get wrong: app shell grids, sidebar/topbar behavior, table/detail composition, mobile collapse, sticky action regions, and scoped loading/error/empty states.
- Keep examples semantic. Use token names such as `--surface`, `--border`, `--space-4`, and `--radius-md` instead of brand-specific final values.
- Drop exhaustive component CSS, full template libraries, repeated variants, large brand palettes, decorative examples, and framework-specific boilerplate that belongs in stack references.
- Drop examples that only demonstrate syntax and do not improve product quality.
- When a cross-cutting UI idea appears in multiple files, keep it once at the right layer: token asset rules in templates/tokens, scenario anatomy in scenarios, rendered proof in verification, and stack-specific file organization in stacks.

The target size is "operational reference": enough concrete structure for an agent to build a production surface, still small enough to be read only when the MCP contract selects that UIX reference id.

## Required UI Baseline

Every production UI surface must establish these decisions before implementation:

- Product role: who uses the screen, what job it completes, and which business object is being inspected or changed.
- Scenario: one selected scenario kind from the MCP contract, plus any stack references selected by the technical baseline.
- Density: workbench, balanced, comfortable, or immersive. Density must match repeat-use behavior, not taste.
- Layout shell: navigation, content region, detail/side panel, action region, and responsive collapse behavior.
- Semantic tokens: color roles, type scale, spacing rhythm, radius/elevation policy, focus ring, error/success/warning/info states, and motion policy.
- State model: loading, success, error, empty, validation, disabled, business-blocking, long-content, and responsive states that are in scope.
- Evidence plan: rendered inspection, workflow path, viewport coverage, accessibility signals, and remaining manual gaps.

## Brainstorm

- Capture frontend work in user language: main user roles, primary tasks, data objects, input forms, table/detail needs, navigation depth, device targets, and unacceptable UI shapes.
- Keep the first decision at product-surface level. Do not start from a framework, theme, library, or deployment note.
- If the user asks for a business application, plan the actual application screen as the first viewport, not a marketing page or explanatory landing page.
- If the user explicitly does not need UI, record the skip reason. Do not invent frontend work.

## Architecture

- Convert the confirmed UI target into `uiQualityContract`: scenario, quality level, surface policy, layout baseline, density, semantic token policy, selected reference groups, forbidden user-visible content, required UI states, and business UI rules.
- Keep the contract field-level and operational. Do not store large prose blobs or duplicate the full frontend model inside the UI contract.
- The selected scenario reference must explain page structure. Token references must explain how the visual system is made. Stack references must explain how to implement it in the detected framework.
- Frontend sections must reject product-boundary leakage: runtime commands, stack explanations, Loom/MCP terms, verification instructions, and delivery progress do not belong in the product UI.
- `designTokenAssetPlan` selects whether to reuse, extend, or create token assets. When it contains a `templateId`, read the matching template file as a baseline, adapt it to the project stack and existing theme, and record only file/evidence facts in the result.

## Task Planning

Attach UI baseline requirements to any task that:

- Creates a new visible page, route, screen, panel, modal, drawer, table, form, chart, dashboard, app shell, navigation, or workflow.
- Changes layout, visual design, state handling, validation display, accessibility behavior, responsive behavior, or frontend component foundations.
- Wires data into a user-facing surface where loading/error/empty/business-blocking states are visible.

Do not attach UI baseline requirements to backend-only tasks, schema-only tasks, deployment-only tasks, or non-visible refactors unless their output changes a user-visible surface.

## Execution

- Build the usable workflow first: users must be able to complete the confirmed task, understand outcome, recover from failure, and continue.
- Separate application chrome from content. Navigation and top-level actions should remain stable while tables, forms, details, and feedback change.
- Implement state coverage next to the component that needs it. A global spinner or generic toast is not enough for forms, tables, destructive actions, or business blocks.
- Keep command examples, verification notes, local ports, framework names, and delivery progress out of the product screen unless the product itself is a developer/runtime tool.
- Use existing project conventions first: component library, router, data fetching, CSS approach, icons, tokens, and lint/test setup. Add new primitives only when the repo lacks a safe equivalent.
- Apply `designTokenAssetPlan` before page-level styling: reuse or extend existing token/theme files first; create `targetFiles` only when no compatible token asset exists. Do not create a second token system beside an existing one.
- Use icons for compact tool actions when the icon meaning is standard. Pair icon-only controls with accessible labels and tooltips when needed.

## Review

Classify UI defects as product defects when they affect:

- Completion of the required workflow.
- Visual hierarchy, information density, readable layout, or state clarity.
- Accessibility, keyboard focus, touch targets, contrast, or semantics.
- Responsive behavior at required viewports.
- Product-boundary leakage or demo-only filler.

Task and review evidence should name changed screens/components, checked states, checked viewports, reference group/items used, token asset files changed/reused, screenshot or Playwright evidence when available, and known manual-review gaps.
