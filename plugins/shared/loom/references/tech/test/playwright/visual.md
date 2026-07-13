# Playwright Rendered And Visual Verification

Rendered verification proves that the assigned product surface is visible, usable, and stable at required viewports. It is broader than pixel comparison and narrower than a subjective redesign review.

## Two Distinct Practices

Use rendered inspection to check layout, hierarchy, content, states, interaction, and viewport fit. Use visual regression snapshots only when the repository has a reviewed baseline workflow or the task explicitly owns stable visual output.

A screenshot artifact does not prove quality by itself. The check must state what was observed.

## Viewport Procedure

For each profile viewport:

1. Enter the task-owned route or workflow with deterministic data.
2. Wait for the meaningful ready state.
3. Confirm the actual product surface is present in the first viewport when required by its scenario.
4. Inspect fixed headers, sidebars, toolbars, dialogs, tables, forms, and action regions for overlap or clipping.
5. Exercise the assigned primary action and state feedback.
6. Check long labels, representative data density, and scroll behavior.
7. Capture a screenshot only after the state is stable.

```typescript
await page.setViewportSize({ width: 1440, height: 900 });
await page.goto('/analytics/revenue');
await expect(page.getByRole('heading', { name: 'Revenue analytics' })).toBeVisible();
await expect(page.getByRole('region', { name: 'Revenue trend' })).toBeVisible();
await expect(page.getByRole('button', { name: 'Export report' })).toBeVisible();
await page.screenshot({ path: testInfo.outputPath('revenue-desktop.png'), fullPage: true });
```

## Layout Assertions

Prefer observable product assertions over hard-coded pixel coordinates. Use measurements only for a real geometry contract:

```typescript
const toolbar = page.getByRole('toolbar', { name: 'Report filters' });
const chart = page.getByRole('region', { name: 'Revenue trend' });
await expect(toolbar).toBeVisible();
await expect(chart).toBeVisible();

const toolbarBox = await toolbar.boundingBox();
const chartBox = await chart.boundingBox();
expect(toolbarBox && chartBox && toolbarBox.y + toolbarBox.height <= chartBox.y).toBeTruthy();
```

Do not assert every padding value. Token consistency belongs to implementation and UI quality review; browser geometry checks target overlap, occlusion, unusable size, and required fixed relationships.

## Mobile And Responsive Behavior

- Verify that primary actions remain reachable without hover.
- Check navigation transformation, data-view fallback, dialogs/sheets, sticky actions, keyboard-safe forms, and horizontal overflow.
- A desktop table squeezed below its minimum usable width is a failure even when no element technically overflows.
- Verify text wraps or truncates intentionally and controls retain adequate target size.
- Check orientation or tablet layouts only when the product contract includes them.

## Visual Snapshots

Use `toHaveScreenshot()` for a stable component/page whose visual baseline is reviewed:

```typescript
await expect(page.getByRole('region', { name: 'Pricing summary' }))
  .toHaveScreenshot('pricing-summary-annual.png', {
    animations: 'disabled',
    caret: 'hide',
  });
```

Baseline rules:

- Pin fonts, browser version, viewport, color scheme, locale, timezone, and deterministic data.
- Mask only genuinely variable values; do not mask the area that changed.
- Keep snapshot scope as small as the visual contract allows.
- Review baseline updates as product changes, not automatic test repairs.
- Do not raise diff tolerance until a defect disappears.

## Dynamic Content

Control timestamps, random ids, animations, carousels, maps, remote images, and live charts through accepted test support. Prefer deterministic fixture values over broad masks. Wait for fonts and critical media when their rendering affects layout.

- Freeze or inject values through existing application/test seams; do not patch rendered text after the app loads.
- Disable motion for capture while retaining the normal interactive path in functional checks.
- Keep one representative long/empty/error value when those states affect geometry.

## Canvas, WebGL, Charts, And Media

- Prove the canvas or media has non-zero dimensions and nonblank output.
- Check the assigned control or interaction changes the scene/state.
- Use screenshot or pixel sampling for blank-render detection, not source existence.
- Verify fallback/error state when assets fail if that state is in scope.
- Keep semantic labels or accessible summaries for charts/media where the product requires them.

## State Captures

Capture the state that proves the check: loading only when loading behavior is assigned; validation after the invalid action; business blocking with its reason; success after data refresh. Avoid producing many screenshots with no stated purpose.

Name artifacts by surface, state, and viewport so a reviewer can identify them without opening every file. Do not reuse one screenshot ref for several states that were not actually rendered.

## Environment Blockers

Missing browser binaries, unavailable fonts/assets, credentials, inaccessible preview, or unsupported GPU may block rendered evidence. Record the attempted check and concrete blocker. Source inspection can be fallback evidence, but it cannot be reported as a successful rendered check.

Separate host-specific rendering differences from product layout defects by confirming the selected browser revision, font availability, device scale, and color scheme before changing application styles or snapshot tolerance.

## Evidence Summary

Name the viewport, route/workflow, state, observed layout/interaction outcome, and artifact ref. Keep the image binary outside prose. A successful build is supporting evidence; it does not replace rendered verification.

For visual regression, also identify the baseline name and whether the comparison passed first attempt, passed after retry, or remains blocked/failed.
