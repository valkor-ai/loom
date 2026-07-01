# UIX Anti-Patterns

Load this file when generating, refining, or reviewing user-visible UI. Treat these as product-quality defects when they appear in production surfaces.

## Product Boundary Failures

- Do not place delivery progress, implementation notes, local verification commands, stack explanations, or "currently wired" commentary inside the product UI.
- Do not use a marketing hero, footer, or feature-summary section as the first screen for an operational tool, admin console, data console, developer tool, or workflow application.
- Do not expose Loom block names, internal artifact names, MCP tool names, enum values, schema terms, or request ids in user-visible UI.

## Demo-Looking UI

- Avoid one-file product shells where navigation, data fetching, form state, modals, tables, and styling are all mixed into one large component after the app grows beyond a tiny proof.
- Avoid decorative card grids that explain capabilities instead of giving users the actual working surface.
- Avoid empty visual polish: gradient blobs, glass panels, oversized hero type, stock-like illustrations, or large brand panels when the user needs a dense work surface.
- Avoid raw color sprawl, arbitrary spacing, and unrelated radius values. Use semantic tokens or the existing design system.

## Interaction Failures

- Do not hide critical actions behind hover-only controls.
- Do not make modal dialogs the default answer to every secondary action; use inline actions, drawers, sheets, or contextual panels when they preserve task context better.
- Do not submit forms without visible validation, disabled/loading state, success feedback, and recoverable error feedback.
- Do not show data tables without empty, loading, error, pagination or overflow behavior when those states are in scope.

## Review Rule

If any forbidden product-boundary content appears in the product UI, the task cannot claim `frontendQualitySelfCheck.status=satisfied`.
