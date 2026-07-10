# UIX Focus: System

Load this when creating or changing a visual system, app shell, shared components, tokens, or component foundations.

## System Baseline

- Start from existing project conventions. Extend them instead of inventing a parallel design system.
- Define semantic tokens for color, type, spacing, radius, elevation, focus, state, and motion.
- Create primitives only when they remove repeated implementation: Button, Input, Select, Textarea, Checkbox/Switch, Badge, Alert, Table, Card/Panel, Modal/Dialog, Drawer/Sheet, Tabs, Tooltip, Toast, Pagination, Skeleton.
- Keep component props semantic. Prefer `variant="danger"` over `color="#ef4444"`.

## Minimal Token Set

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

## Token Asset Workflow

1. Inspect existing style assets before creating files: Tailwind config, global CSS, variables/theme files, component-library theme, native theme, or design-token package.
2. If a compatible asset exists, extend it in place. If none exists, create the token file selected for the current implementation.
3. Import or register the token asset once at the app root. Do not make every page import its own token file.
4. Convert page-local raw values into semantic aliases when the task creates shared or repeated UI. One-off raw values are acceptable only for asset dimensions or media crop details.
5. Record token asset files and the UI files that consume them in implementation evidence.

Useful alias groups for product UI:

```css
:root {
  --surface: var(--color-surface);
  --surface-raised: var(--color-surface-elevated);
  --surface-muted: var(--color-surface-tinted);
  --text: var(--color-on-surface);
  --text-muted: var(--color-on-surface-muted);
  --border: var(--color-border);
  --border-strong: var(--color-border-strong);
  --focus-ring: 0 0 0 3px color-mix(in oklch, var(--color-primary) 28%, transparent);
  --control-height-sm: 32px;
  --control-height-md: 40px;
  --row-height-compact: 40px;
  --row-height-default: 48px;
  --shell-sidebar-width: 240px;
  --shell-detail-width: 380px;
}
```

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

## Shared Primitive Bar

For a production internal product, the first UI pass should usually establish these primitives or reuse the repo equivalents:

```text
Button / IconButton / Input / Select / Textarea / Checkbox
Badge / StatusPill / Alert / EmptyState / Skeleton
Table / Pagination / Toolbar / Drawer or DetailPanel / Dialog
FormField / FieldError / Toast or InlineNotice
```

Do not create decorative primitives before the workflow has the controls above. A business UI that lacks field errors, disabled states, row states, and detail actions is still demo-level even if it looks styled.

## Quality Gate Index

| Gate | Pass signal | Fail signal |
| --- | --- | --- |
| `token.single_source_consumed` | UI consumes one project token/theme source through the app style entry or component system, and evidence cites both token asset files and token consumer files. | New page-local token system competes with existing styles, token file is created but not imported/consumed, or raw values remain scattered across repeated UI. |
