# Frontend UIX Core

Load this file when a loom request includes `frontend_experience`, `frontendExperienceRequirement`, frontend review signals, or user-visible UI work. Loom request artifacts remain the source of truth; this reference only sharpens visual and interaction execution.

## Brainstorm

- Capture the product surface in user language: main users, jobs-to-be-done, primary flows, navigation model, visual direction, interaction density, responsive targets, accessibility expectations, explicit must-not shapes, and whether success needs a business UI, verification UI, or no UI.
- Confirm unacceptable outcomes explicitly, such as static mockups, decorative dashboards, hidden critical actions, unreadable dense views, or flows that work only on one viewport.
- Preserve the confirmed target in the Brainstorm candidate. If the user skips UI, record the skip reason instead of inventing frontend work.

## Architecture And Planning

- Translate UIX into acceptance criteria for layout hierarchy, grid/spacing rhythm, typography/readability, color/contrast, responsive behavior, loading/empty/error states, action feedback, form/search/navigation behavior, and touch/pointer targets.
- Keep UIX requirements tied to user workflows. A visual system is not complete if the required task cannot be completed, recovered, or understood.
- Prefer existing project design tokens, component libraries, routing patterns, and state-management conventions over introducing a new visual system.

## Execution

- Implement the full state model for the requested surface: default, loading, empty, success, partial/failure, validation, retry/recovery, disabled, hover/focus/active, and mobile/desktop variants when relevant.
- Do not ship a static happy-path mock when the contract expects an interactive workflow.
- Make primary actions, destructive actions, navigation exits, and feedback states visible in the flow where users need them.
- Keep forms, filters, tables, cards, charts, and navigation usable under realistic content length, missing data, and API failure.

## Review

- Classify UIX issues as product defects when they break required workflows, accessibility, state coverage, responsive usability, or confirmed visual direction.
- Treat minor alignment, copy, or polish issues as warnings or manual-review items according to the returned severity policy.
- In TaskResult or ReviewResult evidence, cite changed screens/components, covered states, responsive or accessibility checks, screenshot/Playwright refs, and remaining manual-review risks.
