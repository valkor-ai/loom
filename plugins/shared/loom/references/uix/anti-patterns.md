# UIX Anti-Patterns

Load this file when generating, refining, or reviewing user-visible UI. These are production defects, not taste preferences, when they appear in a product surface.

## Product Boundary Failures

- Runtime commands, local ports, dependency commands, stack explanations, verification instructions, delivery progress, internal workflow names, tool names, generated artifact ids, validator terms, and internal status values must not appear in normal product UI.
- Operational products must not open with a marketing hero, feature-summary wall, footer-heavy page, or "how this was built" explanation.
- Brand intro blocks, footer-like explainer sections, and "system capability" cards are product-boundary failures when the user asked for a workbench, admin console, data surface, or internal product.
- Developer/runtime products may show technical terms only when the user task is actually about those terms.
- Empty states may explain the business state and next action; they must not explain the implementation process.

## Demo-Looking UI

- Avoid one-file app shells that mix routing, data fetching, form state, modal state, table state, and all styling once the screen has real workflow complexity.
- Avoid decorative card grids that describe capabilities instead of providing the actual working surface.
- Avoid repeated "icon + title + paragraph" tiles when users need data, forms, filters, actions, or status.
- Avoid oversized hero typography inside dashboards, staff consoles, forms, tables, and side panels.
- Avoid giant logo/brand blocks in the app shell. Navigation identity should be compact unless the scenario is marketing/corporate.
- Avoid ornamental gradients, blobs, glass panels, background noise, stock-like illustrations, and giant brand blocks when the user must scan or operate data.
- Avoid arbitrary color sprawl, unrelated radius values, and inconsistent spacing. Use semantic tokens or the existing design system.

## Layout Failures

- Do not put cards inside cards for normal page sections. Use full-width bands, panels, tables, drawers, or repeated item cards as appropriate.
- Do not use a generic centered container for every page. Workbench, data, docs, mobile, marketing, and immersive surfaces need different layout baselines.
- Do not let text overlap, clip, wrap into broken controls, or resize surrounding layout when states change.
- Do not hide primary task controls below decorative content.
- Do not collapse desktop data tables into unusable mobile tables without a card/list/detail fallback.

## Interaction Failures

- Do not hide critical actions or data behind hover-only controls.
- Do not use modals as the default answer for every secondary action. Prefer inline edit, side panel, drawer, split view, undo toast, or dedicated step flow when they preserve task context.
- Do not submit forms without visible validation, disabled/submitting state, success feedback, and recoverable error feedback.
- Do not show tables without scoped loading, empty, error, pagination/overflow, sorting/filtering behavior when those states are in scope.
- Do not use toasts as the only place where business-blocking feedback appears. The related field, row, panel, or form must also show the block.
- Do not animate layout properties such as width, height, top, or left for frequent UI transitions. Prefer transform and opacity, and respect reduced motion.

## Visual Slop Checks

- No default purple-blue gradient theme unless the domain and existing brand genuinely require it.
- No single-hue palette where every surface is a tinted version of one color.
- No dark slate dashboard by reflex for every operational product.
- No beige/cream/tan page by reflex for every "premium" product.
- No gradient text as routine emphasis.
- No left-border color stripe as the main hierarchy device for every card or alert.
- No glassmorphism as the default visual language.
- No endless equal-size card grids for unrelated content.
- No default Inter/Roboto/Arial-only typography when the repo does not already use it; choose a suitable stack or follow the existing system.

## Accessibility Red Lines

- Text contrast must meet AA targets for normal and large text.
- Focus must be visible and not color-only.
- Keyboard access must reach all primary controls.
- Touch targets must be large enough on mobile and tablet.
- Color must not be the only carrier of status or risk.
- Headings, buttons, links, labels, and form errors must use correct semantics.
- Motion must respect `prefers-reduced-motion`.

## Review Rule

If product-boundary leakage or demo-only filler appears in a production surface, the UI quality result cannot be treated as satisfied. Fix the UI or record the remaining issue as a known gap with a concrete reason.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `anti.product_boundary.no_internal_process` | Changed UI source and rendered copy keep internal workflow, runtime, build, and progress language out of product surfaces. | Product UI contains delivery notes, stack explanations, runtime commands, generated artifact ids, future-stage copy, or feature-description filler instead of business UI. |
