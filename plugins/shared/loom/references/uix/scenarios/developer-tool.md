# UIX Scenario: Developer Tool

Use for IDE-like tools, API explorers, SDK consoles, build/deploy tools, logs, debugging, automation, and runtime dashboards where technical content is part of the product.

## Baseline

- Technical terms are allowed only when they are user-facing product concepts.
- First viewport exposes the tool workspace, not delivery notes.
- Density is usually `workbench_dense`.
- Monospace, code blocks, logs, command snippets, and structured metadata need careful hierarchy.

## Workspace Layout

```css
.devtool-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  background: var(--surface);
}

.devtool-workspace {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 420px);
  grid-template-rows: auto minmax(0, 1fr);
}

.devtool-output {
  min-width: 0;
  overflow: auto;
  font-family: var(--font-mono);
}
```

## Required Patterns

- Workspace shell with navigation, command/action region, output/result region, and detail/error panel.
- Copy buttons, status indicators, logs, retry actions, and clear failure classification.
- Keyboard-friendly controls and visible focus.
- Code/log formatting with wrapping or horizontal scroll.
- Empty states that help the developer start.

## Interaction

- Copy/download/destructive runtime controls must be visually distinct.
- Logs need filtering, search, and timestamp/source context when volume is high.
- Errors should separate user configuration problems, environment failures, and tool failures.
- Keyboard shortcuts may be shown only when implemented and useful.

## Avoid

- Using developer jargon in a non-developer product.
- Dumping raw JSON or logs without structure when the user needs decisions.
- Hiding destructive runtime actions beside routine copy/download controls.
