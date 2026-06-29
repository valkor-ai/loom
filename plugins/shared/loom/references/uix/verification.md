# UIX Verification

Load this file before frontend review, visual inspection, accessibility checks, or screenshot-based evidence gathering.

## Rendered Checks

- Inspect the rendered application when a local preview is available. Prefer Browser or Playwright screenshots for real layout evidence.
- Check the required workflow from entry to completion, not only isolated components.
- Exercise default, loading, empty, success, validation, error, retry, disabled, hover/focus/active, and long-content states when they are in scope.

## Visual Checks

- Verify visual hierarchy, composition balance, spacing consistency, typography scale, readable line length, color/contrast, brand/token consistency, and responsive breakpoints.
- Watch for overlap, clipped text, horizontal scroll, cramped controls, unstable layout shifts, and controls that resize when state changes.
- Confirm media, charts, canvases, maps, and icons render nonblank and remain framed correctly.

## Interaction Checks

- Check keyboard navigation and focus visibility for core controls.
- Check touch target size and scroll behavior for mobile or responsive surfaces.
- Confirm feedback appears in the same flow as the action: saving, errors, retry, undo, destructive confirmation, and async progress.

## Evidence

- Include screenshot paths, Playwright reports, viewport sizes, commands run, and known manual-review gaps in TaskResult or ReviewResult evidence.
- If a check cannot run because of environment, dependency, network, auth, or credential limits, classify that separately from product defects.
