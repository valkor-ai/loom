# UIX Token: Radius And Elevation

Load this file when styling cards, panels, inputs, buttons, modals, drawers, menus, popovers, charts, or layered surfaces.

## Radius

Use radius as a system:

- `radius-none`: tables, sharp enterprise surfaces, code/log panels when appropriate.
- `radius-sm`: 4px for dense inputs, table chips, compact controls.
- `radius-md`: 6-8px for buttons, cards, fields, panels.
- `radius-lg`: 10-12px for modals, drawers, mobile cards.
- Larger radii only when the brand or mobile-native platform expects it.

Cards should usually be 8px or less unless an existing design system says otherwise.

## CSS Token Skeleton

```css
:root {
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --shadow-raised: 0 1px 2px rgb(16 24 40 / 0.08);
  --shadow-popover: 0 8px 24px rgb(16 24 40 / 0.14);
  --z-dropdown: 10;
  --z-sticky: 20;
  --z-modal: 50;
  --z-tooltip: 70;
}
```

## Elevation

Prefer borders and surface contrast for normal workbench hierarchy. Use shadow/elevation for true layering:

- Base surface: no shadow.
- Raised panel/card: subtle border or very soft shadow.
- Sticky topbar/sidebar: border plus surface.
- Dropdown/popover/menu: shadow + border.
- Modal/drawer: scrim plus clear elevation.
- Toast/notification: elevated, but never blocks core workflow longer than necessary.

## Layering Rules

- A card inside a card is usually a layout mistake. Use sections, tables, rows, or panels instead.
- Modals and drawers must have a clear close route and focus behavior.
- Floating controls must not cover table rows, form submit buttons, chart legends, or mobile safe areas.
- Elevation must communicate interaction depth, not decoration.

## Implementation

- Define radius and shadow tokens once.
- Keep border color tied to color tokens.
- Avoid random shadow values per component.
- Use z-index tokens for dropdown, sticky, fixed, modal, popover, tooltip, and notification layers.

## Self-Check

- Radius and elevation are consistent across controls.
- Layered surfaces remain readable on light and dark backgrounds.
- Modals, drawers, and popovers do not create hidden scroll traps.
