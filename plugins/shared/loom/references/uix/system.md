# UIX Focus: System

Load this when creating or changing a visual system, app shell, shared components, tokens, or component foundations.

## System Baseline

- Start from existing project conventions. Extend them instead of inventing a parallel design system.
- Define semantic tokens for color, type, spacing, radius, elevation, focus, state, and motion.
- Create primitives only when they remove repeated implementation: Button, Input, Select, Textarea, Checkbox/Switch, Badge, Alert, Table, Card/Panel, Modal/Dialog, Drawer/Sheet, Tabs, Tooltip, Toast, Pagination, Skeleton.
- Keep component props semantic. Prefer `variant="danger"` over `color="#ef4444"`.

## Minimal Token Contract

```css
:root {
  --surface: ...;
  --surface-raised: ...;
  --text: ...;
  --text-muted: ...;
  --border: ...;
  --primary: ...;
  --danger-surface: ...;
  --danger-text: ...;
  --space-4: 16px;
  --radius-md: 8px;
  --focus-ring: ...;
}
```

Use the repo's existing names when present. The important part is that components consume semantic roles rather than raw values.

## Shell

- Operational apps need stable navigation, page title/context, primary action region, content region, and feedback region.
- Docs need nav/content/TOC/search.
- Marketing sites need product/offer signal, proof sections, and conversion routes.
- Mobile/native surfaces need safe areas and platform navigation.

## States

Every reusable component that can wait, fail, validate, or disable must expose those states. Avoid forcing feature code to hand-roll inconsistent variants.

Required common states:

- default, hover, focus, active, disabled.
- loading/submitting.
- success, warning, danger/error, info.
- empty and skeleton for data surfaces.
- business-blocking for domain-rule stops.

## Quality Bar

- Component dimensions are stable across state changes.
- Focus ring is visible and consistent.
- Text and icons align cleanly.
- Tokens are reusable and documented through names, not comments only.
- Components do not encode delivery process language.
- New shared components must include the states their consumers need; do not make every feature recreate disabled/loading/error variants.
