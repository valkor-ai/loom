# UIX Scenario: Admin Dashboard

Use for internal operations, staff consoles, CRM/ERP/CMS back offices, SaaS control panels, and management tools. Efficiency, scanability, and workflow completion matter more than visual spectacle.

## Baseline

- First viewport is the working console, not a landing page.
- Typical shell: sidebar navigation, topbar with page context/search/actions, main content, optional right detail drawer.
- Density is usually `workbench_dense` or `balanced`.
- Visual style is restrained: quiet surfaces, strong information hierarchy, semantic status colors, and predictable controls.

## Required Patterns

- Navigation: active section, grouped nav items, stable page title, breadcrumbs when depth is greater than one.
- Data: table/list with filters, search, sort, pagination or infinite scroll, empty/loading/error states.
- Detail: row selection opens side panel, route detail, or inline expansion without losing list context.
- Forms: grouped sections, field validation, disabled/submitting state, business-blocking feedback.
- Actions: primary action visible near the relevant region; destructive actions require confirmation or recovery.
- Feedback: success updates affected row/detail and appears near the changed object.

## Layout

- Desktop: sidebar 220-280px, topbar 52-64px, content region with min-width handling.
- Tablet: sidebar collapses to drawer or rail; filters may move into drawer.
- Mobile: convert table-heavy views to list/detail cards or full-screen detail routes; do not depend on hover.
- Use sticky table headers or action bars only when they do not hide content.

## States

- Loading: table skeleton or row skeleton, not a full-page spinner after shell loads.
- Empty: explain the business reason and next action.
- Error: show retry and preserve filters/form inputs.
- Business-blocking: message must reference the rule and affected object.

## Avoid

- Hero sections, marketing footers, feature-explainer cards, decorative metrics, or long implementation notes.
- Modal-only workflows that make users lose list context.
- Tables without overflow, pagination, or responsive fallback.
