# UIX Token: Color System

Load this file for any frontend work that creates or changes visual styling. Use existing project tokens first. When a project has no usable color system, create semantic roles before styling components.

## Required Roles

Define roles, not raw colors:

- `surface`, `surface-muted`, `surface-raised`, `surface-inset`.
- `text`, `text-muted`, `text-subtle`, `text-inverse`.
- `border`, `border-strong`, `divider`.
- `primary`, `primary-hover`, `primary-active`, `primary-contrast`.
- `secondary` and `accent` only when the product needs a second meaning.
- `success`, `warning`, `danger`, `info`, each with background, border, text, and icon roles.
- `focus-ring`, `selection`, `scrim`.

## CSS Token Skeleton

Use project naming when it already exists. For a new web surface, this shape is acceptable:

```css
:root {
  --surface: #ffffff;
  --surface-muted: #f7f8fa;
  --surface-raised: #ffffff;
  --text: #111827;
  --text-muted: #4b5563;
  --border: #d9dee7;
  --primary: #1f6feb;
  --primary-hover: #185abc;
  --primary-contrast: #ffffff;
  --success-surface: #ecfdf3;
  --success-text: #067647;
  --warning-surface: #fffaeb;
  --warning-text: #b54708;
  --danger-surface: #fef3f2;
  --danger-text: #b42318;
  --focus-ring: #2563eb;
}
```

The exact values may change, but components should consume roles rather than hard-coded values.

## Selection Rules

- Choose a palette for the scenario, not the category stereotype. Finance is not automatically dark blue/gold; dashboards are not automatically dark slate.
- Workbench and admin tools should favor quiet surfaces, clear borders, and restrained accents.
- Marketing and immersive surfaces may use stronger color, but text contrast and readability still win.
- Data-heavy products need separate semantic status colors; do not overload the primary brand color for every state.
- Do not use more than one dominant hue family unless the product has an explicit brand system or data encoding need.
- Pick a color commitment level before values: restrained accent, committed brand surface, full multi-role palette, or immersive scene palette. Most operational products should be restrained.
- Neutral colors should be slightly tuned toward the product hue when a custom palette is introduced, but must stay readable and calm.

## Scenario Palette Guidance

| Scenario | Typical direction | Watch out for |
| --- | --- | --- |
| Admin/workbench/data | Low-chroma surfaces, one clear primary, semantic status roles. | Dark-slate or purple-gradient reflex. |
| Finance/workstation | Trustworthy contrast, strong status/risk separation, tabular numeric clarity. | Blue/gold cliche and low-contrast warning states. |
| Consumer/mobile | Warmth and clarity with accessible action color. | Decorative gradients behind forms. |
| Docs/developer | High text contrast, code/callout roles, subtle navigation state. | Marketing palette overwhelming reference reading. |
| Marketing/corporate | More expressive brand color and media overlay roles. | Gradient text, unreadable hero overlays. |
| 3D/immersive | Scene-aware foreground/control overlays. | UI controls disappearing into the scene. |

## Contrast And Status

- Normal text must meet AA contrast on its surface.
- Do not communicate status by color alone. Pair color with label, icon, shape, or placement.
- Error and business-blocking states must be visually distinct from warning and neutral empty states.
- Disabled state should preserve readable labels; reduce emphasis without making controls illegible.
- Hover and active colors must remain inside the semantic role family.

## Implementation

- Prefer CSS variables, design-token objects, Tailwind theme extension, or the project's existing token format.
- Keep token names stable even if palette values change.
- Avoid one-off component colors unless they represent a new semantic role that will be reused.
- Check dark mode only if the product supports it; do not invent a dark mode when it is out of scope.
- If using OKLCH, keep fallback behavior in mind for the project's browser support. If the stack already uses hex/HSL tokens, preserve format unless changing it is justified.

## Self-Check

- No raw hex/HSL/OKLCH sprawl inside many components.
- No generic purple gradient unless required by brand.
- No category-reflex palette.
- Every visible state uses a semantic role.
- Text, borders, icons, charts, and focus rings remain legible on the selected surfaces.
- `frontendQualitySelfCheck.designTokenEvidence` names the token asset and at least one consumer file that uses these roles.
