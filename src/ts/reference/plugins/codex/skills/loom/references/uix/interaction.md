# Interaction UIX

Load this file for complex flows, forms, search, loading, feedback, state machines, gestures, onboarding, or error recovery.

## State Model

- Model complex UI as states, events, transitions, guards, and actions before implementation.
- Cover idle, editing, validating, loading, success, empty, partial failure, error, retrying, disabled, and complete states when relevant.
- Every visible state needs a way out. Avoid dead ends such as an error screen with no retry, back, or alternate path.
- Prevent impossible states, such as showing loading and final error at the same time, or enabling submit while validation is unresolved.

## Errors And Recovery

- Prefer prevention first: constraints, smart defaults, inline validation, confirmation for destructive actions, autosave for risky inputs.
- Place field errors near the field, page errors near the failed region, and global errors only when scope is truly global.
- Error copy must say what happened, why if useful, and the next action. Do not expose raw stack traces or vague "Something went wrong" copy.
- Preserve user input on failure, offer retry for transient failures, and provide undo for reversible destructive actions.

## Loading And Feedback

- Show something immediately. Use skeletons for known layout, subtle indicators for short waits, progress for measurable long work, and background options for very long work.
- Avoid layout shift when content loads. Keep scroll position stable during refresh.
- Feedback should appear in the same flow as the action: saving, success, retry, undo, destructive confirmation, optimistic rollback, or permission failure.
- Respect reduced-motion for shimmer, slide, scale, and looping effects.

## Forms, Search, And Navigation

- Forms need labels, validation timing, error association, keyboard order, disabled/loading states, and submit/retry paths.
- Search needs query entry, suggestions or history when useful, loading, results, no results, spelling/refinement hints, and clear filters.
- Navigation should match product structure and user tasks; active state, back behavior, breadcrumbs, tabs, drawers, and deep links must be predictable.
- Onboarding should get users to value quickly, one concept at a time, with skip or resume paths when appropriate.

## Evidence

- Record flow/state coverage in TaskResult for frontend work.
- Cite screenshots, Playwright traces, state diagrams, or manual checks when complex interactions cannot be fully automated.
