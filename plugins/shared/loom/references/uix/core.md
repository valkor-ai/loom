# Frontend UIX Core

Use this file for any user-visible frontend work: pages, screens, panels, navigation, forms, tables, charts, shared components, visible states, or responsive behavior. These references turn a selected product surface into concrete implementation decisions.

This reference set is a project-owned rewrite. It absorbs proven UI craft categories such as scenario fit, token systems, state coverage, stack execution, anti-pattern detection, and rendered verification without copying another skill's text or requiring a second skill to be loaded.

## Reference Compression Rule

UIX references are not meant to be tiny summaries, and they are not meant to become a full copied design-code library. Keep the parts that change implementation behavior:

- Keep scenario-specific layout skeletons, component anatomy, responsive breakpoints, state patterns, accessibility requirements, and verification expectations.
- Keep compact code-like examples when they explain structure that agents commonly get wrong: app shell grids, sidebar/topbar behavior, table/detail composition, mobile collapse, sticky action regions, and scoped loading/error/empty states.
- Keep examples semantic. Use token names such as `--surface`, `--border`, `--space-4`, and `--radius-md` instead of brand-specific final values.
- Drop exhaustive component CSS, full template libraries, repeated variants, large brand palettes, decorative examples, and framework-specific boilerplate that belongs in stack references.
- Drop examples that only demonstrate syntax and do not improve product quality.
- When a cross-cutting UI idea appears in multiple files, keep it once at the right layer: token asset rules in templates/tokens, scenario anatomy in scenarios, rendered proof in verification, and stack-specific file organization in stacks.

The target size is "operational reference": enough concrete structure for an agent to build a production surface, still small enough to be loaded only when the work actually changes that UI concern.

## Required UI Baseline

Every production UI surface must establish these decisions before implementation:

- Product role: who uses the screen, what job it completes, and which business object is being inspected or changed.
- Scenario: the selected product scenario and the implementation stack already present in the project.
- Density: workbench, balanced, comfortable, or immersive. Density must match repeat-use behavior, not taste.
- Layout shell: navigation, content region, detail/side panel, action region, and responsive collapse behavior.
- Semantic tokens: color roles, type scale, spacing rhythm, radius/elevation policy, focus ring, error/success/warning/info states, and motion policy.
- State model: loading, success, error, empty, validation, disabled, business-blocking, long-content, and responsive states that are in scope.
- Evidence plan: rendered inspection, workflow path, viewport coverage, accessibility signals, and remaining manual gaps.

## UI Work Boundary

Apply the UI baseline whenever work:

- Creates a new visible page, route, screen, panel, modal, drawer, table, form, chart, dashboard, app shell, navigation, or workflow.
- Changes layout, visual design, state handling, validation display, accessibility behavior, responsive behavior, or frontend component foundations.
- Wires data into a user-facing surface where loading/error/empty/business-blocking states are visible.

Do not apply UI rules to backend-only work, storage-only work, deployment-only work, or non-visible refactors unless their output changes a user-visible surface.

## Implementation

- Build the usable workflow first: users must be able to complete the confirmed task, understand outcome, recover from failure, and continue.
- Separate application chrome from content. Navigation and top-level actions should remain stable while tables, forms, details, and feedback change.
- Implement state coverage next to the component that needs it. A global spinner or generic toast is not enough for forms, tables, destructive actions, or business blocks.
- Keep command examples, verification notes, local ports, framework names, and delivery progress out of the product screen unless the product itself is a developer/runtime tool.
- Use existing project conventions first: component library, router, data fetching, CSS approach, icons, tokens, and lint/test setup. Add new primitives only when the repo lacks a safe equivalent.
- Apply the declared token asset plan before page-level styling: reuse or extend existing token/theme files first; create new token files only when no compatible asset exists. Do not create a second token system beside an existing one.
- Use icons for compact tool actions when the icon meaning is standard. Pair icon-only controls with accessible labels and tooltips when needed.

## Review

Classify UI defects as product defects when they affect:

- Completion of the required workflow.
- Visual hierarchy, information density, readable layout, or state clarity.
- Accessibility, keyboard focus, touch targets, contrast, or semantics.
- Responsive behavior at required viewports.
- Product-boundary leakage or demo-only filler.

Evidence should name changed screens/components, checked states, checked viewports, token asset files changed/reused, rendered evidence when available, and known manual-review gaps.
