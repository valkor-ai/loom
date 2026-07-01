# UIX Token: Typography

Load this file when creating or changing headings, tables, forms, dashboards, marketing copy, docs, code views, or mobile text.

## Font Stack

- Use the existing project font system when present.
- For Chinese business software, prefer reliable CJK stacks such as `Noto Sans SC`, `PingFang SC`, `Microsoft YaHei`, and system fallbacks.
- For code, numeric tables, logs, or developer tools, include a monospace stack with tabular figures when supported.
- Avoid making Inter, Roboto, Arial, or `system-ui` the only design decision in a new product unless the existing repo already standardizes on it.
- Do not load remote fonts when the environment or product constraints make that risky; local/system fallbacks are acceptable when chosen intentionally.

## Scale

Use a small, stable type scale. Suggested web baseline:

- `xs`: 12px for metadata, table hints, compact labels.
- `sm`: 13-14px for dense table cells, secondary controls.
- `base`: 15-16px for body, form inputs, standard controls.
- `lg`: 18px for section intros or important row titles.
- `xl`: 20px for panel titles.
- `2xl`: 24px for page titles inside workbench UI.
- Larger sizes are reserved for true landing/editorial/immersive hero contexts.

## CSS Token Skeleton

```css
:root {
  --font-sans: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", monospace;
  --text-xs: 12px;
  --text-sm: 14px;
  --text-base: 16px;
  --text-lg: 18px;
  --text-xl: 20px;
  --text-2xl: 24px;
  --leading-tight: 1.2;
  --leading-normal: 1.5;
  --leading-readable: 1.65;
}
```

Use tabular numeric variants for finance, tables, metrics, and logs when the stack supports it.

## Surface Rules

- Workbench/admin/table UI: compact headings, dense but readable rows, tabular numbers, stable line height.
- Forms: labels and helper text must remain readable; error text should not shift unrelated layout.
- Docs: readable prose width, clear heading hierarchy, code block typography.
- Marketing: expressive headline scale is allowed, but supporting copy must stay readable and responsive.
- Mobile: text must not depend on desktop line length; controls and labels must wrap cleanly.

## Implementation

- Keep letter spacing at `0` for normal text. Use uppercase tracking only for small section labels when the style already supports it.
- Use line-height around 1.45-1.65 for body/prose, tighter for headings, and comfortable for table rows.
- Use font weight before color when emphasizing text in dense UI.
- Keep heading levels semantic; do not skip levels to force visual size.

## Self-Check

- Long Chinese labels, long English words, account ids, and numeric values fit without overlap.
- Page titles do not look like marketing heroes inside internal tools.
- Tables align numbers and statuses.
- Error/help text is readable at the smallest supported viewport.
