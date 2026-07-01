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

## Responsive Rules

- Reduce page padding before reducing readable content quality.
- Collapse multi-column forms to one column on narrow screens.
- Keep sticky bars and bottom actions away from safe-area edges on mobile.
- Avoid horizontal scroll for the page; allow it only inside data tables or code blocks when needed.

## Self-Check

- No arbitrary one-off margins used to "nudge" many components.
- Layout remains stable across loading/error/success state changes.
- Dense surfaces still have enough breathing room to scan repeated actions.
