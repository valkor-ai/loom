# UIX Token: Spacing

Load this file when creating layout primitives, components, forms, tables, dashboards, mobile screens, or responsive behavior.

## Scale

Use a consistent spacing scale. Suggested baseline:

| Token | px | Use |
| --- | ---: | --- |
| `space-1` | 4 | Icon/text micro gaps, compact separators. |
| `space-2` | 8 | Button internal gap, table cell minor gap. |
| `space-3` | 12 | Form row gap, nav item gap. |
| `space-4` | 16 | Component padding, mobile page padding. |
| `space-5` | 20 | Dense section gap. |
| `space-6` | 24 | Standard section padding, card/table block gap. |
| `space-8` | 32 | Page section separation. |
| `space-10` | 40 | Large section separation. |
| `space-12` | 48 | Editorial/marketing rhythm. |

Extend the scale only when the project already has a larger rhythm or the selected
surface needs it. Keep the base increments divisible by 4 for product UI. A new
value needs a semantic reason such as page section, shell gutter, control height,
or media composition; it must not exist only to nudge one element into place.

## Semantic Rhythm

Use separate semantic aliases for repeated relationships:

| Relationship | Examples |
| --- | --- |
| Inline | icon/label, badge/text, input prefix/suffix |
| Control | button padding, field padding, table cell padding |
| Component | form fields, toolbar items, card content |
| Region | panel sections, list/detail gap, page heading to content |
| Shell | page gutter, sidebar/content gap, mobile safe-area padding |

The same numeric value may serve multiple aliases, but components should consume
the semantic alias so a density change can be made without editing every component.

## CSS Token Skeleton

```css
:root {
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;
  --gap-toolbar: var(--space-3);
  --gap-panel: var(--space-4);
  --pad-page-dense: var(--space-4);
  --pad-page-default: var(--space-6);
  --pad-control-x: var(--space-3);
  --pad-control-y: var(--space-2);
  --pad-cell-x: var(--space-3);
}
```

Use these tokens in layout primitives and component padding before introducing one-off values.

## Density Rules

- `workbench_dense`: 8-16px component gaps, 36-44px row/control height, compact page padding.
- `balanced`: 12-24px component gaps, 40-48px controls, comfortable form/table spacing.
- `comfortable`: 16-32px gaps, larger touch targets, mobile-friendly spacing.
- `immersive`: spacing follows scene/media composition and must not crowd controls.

## Component Rules

- Tables: consistent cell padding, stable row height, visible overflow behavior.
- Forms: group related fields; do not create one long unstructured column unless the form is short.
- Toolbars: align icon buttons, filters, and primary actions on a predictable rhythm.
- Panels/drawers: keep internal padding consistent and preserve scroll boundaries.
- Empty/error/loading states: occupy the same layout region as the data they replace.
- Brand or page identity regions in workbench UI should be compact. Do not spend vertical space on non-functional intro copy when users need the working surface.

## Responsive Rules

- Reduce page padding before reducing readable content quality.
- Collapse multi-column forms to one column on narrow screens.
- Keep sticky bars and bottom actions away from safe-area edges on mobile.
- Avoid horizontal scroll for the page; allow it only inside data tables or code blocks when needed.
- Mobile page padding usually starts at `space-4`; dense desktop workbenches can also use `space-4` when information volume is high.
- For wide screens, increase outer region separation before increasing every control
  gap. For narrow screens, reduce page padding before shrinking readable text or
  touch targets.
- Keep action groups visually closer to the object they affect than to unrelated
  page chrome. Spacing is part of workflow hierarchy, not decoration.

## Self-Check

- No arbitrary one-off margins used to "nudge" many components.
- Layout remains stable across loading/error/success state changes.
- Dense surfaces still have enough breathing room to scan repeated actions.
- Repeated components use the same gap/padding tokens instead of per-component raw values.
- A spacing audit can explain every off-scale value as an asset dimension, browser
  constraint, platform metric, or an explicitly accepted exception.
