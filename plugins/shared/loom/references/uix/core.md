# Frontend UIX Core

Load this file whenever a Loom request includes user-visible frontend work, `frontend_experience`, `frontendExperienceRequirement`, `uiQualityContract`, frontend review signals, or a task that creates/modifies screens. Loom MCP artifacts are the source of truth; these references turn that contract into concrete implementation decisions.

This reference set is a Loom-owned rewrite. It absorbs proven UI craft categories such as scenario fit, token systems, state coverage, stack execution, anti-pattern detection, and rendered verification without copying another skill's text or requiring a second skill to be loaded.

## Routing Contract

When the request includes `uiQualityContract.referenceProfile.referenceIds`, load only the listed references in addition to this core file.

| Reference id family | File location | Purpose |
| --- | --- | --- |
| `uix.anti-patterns` | `references/uix/anti-patterns.md` | Blocks demo-looking UI and product-boundary failures. |
| `uix.tokens.*` | `references/uix/tokens/` | Defines semantic color, typography, spacing, grid, radius, elevation, and motion decisions. |
| `uix.scenarios.*` | `references/uix/scenarios/` | Defines surface layout, density, navigation, component, and state expectations for a product scenario. |
| `uix.stacks.*` | `references/uix/stacks/` | Defines framework-specific implementation patterns without changing the product contract. |

Do not load unrelated UI skills. Do not copy reference text into task results. Cite reference ids in `frontendQualitySelfCheck.referenceIdsChecked`, then provide concrete evidence from the implemented UI.

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

- Convert the confirmed UI target into `uiQualityContract`: scenario, quality level, surface policy, layout baseline, density, semantic token policy, selected reference ids, forbidden user-visible content, required UI states, and business UI rules.
- Keep the contract field-level and operational. Do not store large prose blobs or duplicate the full frontend model inside the UI contract.
- The selected scenario reference must explain page structure. Token references must explain how the visual system is made. Stack references must explain how to implement it in the detected framework.
- Frontend sections must reject product-boundary leakage: runtime commands, stack explanations, Loom/MCP terms, verification instructions, and delivery progress do not belong in the product UI.

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
- Use icons for compact tool actions when the icon meaning is standard. Pair icon-only controls with accessible labels and tooltips when needed.

## Review

Classify UI defects as product defects when they affect:

- Completion of the required workflow.
- Visual hierarchy, information density, readable layout, or state clarity.
- Accessibility, keyboard focus, touch targets, contrast, or semantics.
- Responsive behavior at required viewports.
- Product-boundary leakage or demo-only filler.

Task and review evidence should name changed screens/components, checked states, checked viewports, reference ids used, screenshot or Playwright evidence when available, and known manual-review gaps.
