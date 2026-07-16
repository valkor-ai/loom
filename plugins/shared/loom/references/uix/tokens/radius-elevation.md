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
  --shadow-border: 0 0 0 1px var(--border);
  --shadow-raised: 0 1px 2px rgb(16 24 40 / 0.08);
  --shadow-popover: 0 8px 24px rgb(16 24 40 / 0.14);
  --scrim: rgb(15 23 42 / 0.45);
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

## Component Combinations

| Component | Radius direction | Elevation direction |
| --- | --- | --- |
| Dense table/list | none or small | border and row contrast |
| Form field/control | small or medium | normally none; focus uses ring |
| Workbench panel | medium | border or subtle raised surface |
| Dropdown/popover | medium | border plus popover shadow |
| Drawer/sheet | large only when the product style supports it | scrim plus directional elevation |
| Modal/dialog | medium | scrim plus modal elevation |
| Toast | medium | elevated and time-bounded |

Do not use elevation to compensate for weak layout hierarchy. First establish
region ownership, spacing, and surface contrast; then add a layer effect only when
the component is actually above another interactive surface.

## Layering Rules

- A card inside a card is usually a layout mistake. Use sections, tables, rows, or panels instead.
- Modals and drawers must have a clear close route and focus behavior.
- Floating controls must not cover table rows, form submit buttons, chart legends, or mobile safe areas.
- Elevation must communicate interaction depth, not decoration.
- Use scrims for modals/sheets when background interaction is blocked. Do not use blurred glass as the default surface style.
- Keep the active layer's focus and scroll boundary visible. A shadow that visually
  separates a drawer but leaves its close action unreachable is not a usable layer.

## Implementation

- Define radius and shadow tokens once.
- Keep border color tied to color tokens.
- Avoid random shadow values per component.
- Use z-index tokens for dropdown, sticky, fixed, modal, popover, tooltip, and notification layers.
- Keep the z-index scale shared by shell, data surfaces, overlays, and notifications;
  document an intentional exception in the existing system asset instead of a page file.

## Self-Check

- Radius and elevation are consistent across controls.
- Layered surfaces remain readable on light and dark backgrounds.
- Component combinations use the declared radius/elevation role instead of a local
  shadow or radius value that creates a new visual dialect.
- Modals, drawers, and popovers do not create hidden scroll traps.
- Nested cards are absent from normal page sections unless there is a clear repeated item structure.
