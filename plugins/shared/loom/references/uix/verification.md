# UIX Verification

Load this file before frontend review, visual inspection, accessibility checks, or screenshot-based evidence gathering.

## Rendered Inspection

- Inspect the rendered application when a local preview is available. Prefer Browser or Playwright screenshots over source-only claims.
- Exercise the required workflow from entry to completion. Component-level inspection is not enough when the task is a business flow.
- Verify the first viewport: it must show the actual product surface for the selected scenario.
- Check at least desktop and mobile-responsive widths for web surfaces when the product contract includes responsive behavior.
- For 3D/canvas/media surfaces, verify the scene or media is nonblank, correctly framed, and still interactive after initial render.

## State Coverage

Check every state that is in scope for the screen:

- Loading: stable layout, skeleton or scoped progress, no page jump.
- Empty: business explanation and next action, not a developer placeholder.
- Success: confirmation near the changed object, updated data, and clear continuation.
- Validation: field-level errors, disabled/submitting state, and preserved input.
- Error: recoverable message, retry or correction path, no stack traces.
- Business-blocking: separated from technical errors and tied to the blocking rule.
- Long content: names, labels, table cells, cards, and buttons remain usable.
- Permission/disabled: reason is visible when the user can act later or needs another route.

## Visual Checks

- Visual hierarchy matches the selected scenario and density.
- Spacing follows a repeatable scale; components do not float with arbitrary gaps.
- Typography scale fits the surface: compact inside workbench panels, expressive only in true hero/editorial contexts.
- Color roles are semantic and consistent across normal, hover, active, focus, disabled, success, warning, error, and info states.
- Radius/elevation communicate interaction depth instead of decoration.
- Layout uses stable dimensions for fixed-format elements such as boards, tables, icon buttons, counters, tiles, and toolbars.
- No text overlaps, clips, or overflows its container at checked breakpoints.

## Interaction Checks

- Keyboard focus order follows reading/task order.
- Icon-only controls have accessible labels and tooltips when meaning is not universal.
- Form submission cannot double-submit and does not lose user input on failure.
- Destructive actions require confirmation, undo, or a clear recovery path based on severity.
- Filters, search, pagination, tabs, and drawers preserve context and make current state visible.
- Mobile interactions do not depend on hover and have adequate touch targets.

## Evidence Requirements

TaskResult or ReviewResult evidence should include:

- `referenceIdsChecked`: all UIX ids that were loaded for the task.
- Changed screens/components and the user workflow checked.
- States covered and states not applicable.
- Viewports or devices checked.
- Screenshot, Playwright, browser, or manual inspection evidence when available.
- Accessibility checks performed or explicit environment blockers.
- Known gaps with business impact, not vague polish notes.

If a rendered check cannot run because of missing dependencies, network, auth, credentials, or environment limits, record that as verification blocked. Do not mark it as a product defect unless the UI itself caused the failure.
