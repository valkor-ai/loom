# React UIX Stack

Use for React, Vite React, Next.js, Remix, and similar React-based projects.

## Rules

- Preserve existing routing, component, data-fetching, form, and styling conventions.
- Split growing UI by responsibility: shell/layout, feature screens, forms, tables, dialogs/drawers, API client/state helpers, and shared primitives.
- Keep effects and async state explicit. Loading, abort/retry, stale data, and error states should be represented in component state or the project's data library.
- For Next or Remix, respect server/client boundaries and route-level loading/error behavior.
- Prefer accessible primitives for dialogs, menus, tabs, selects, tooltips, and popovers.
