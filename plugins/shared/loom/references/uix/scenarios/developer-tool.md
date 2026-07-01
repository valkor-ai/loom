# UIX Scenario: Developer Tool

Use for IDE-like tools, API explorers, SDK consoles, build/deploy tools, logs, debugging, automation, and runtime dashboards where technical content is part of the product.

## Baseline

- Technical terms are allowed only when they are user-facing product concepts.
- First viewport exposes the tool workspace, not delivery notes.
- Density is usually `workbench_dense`.
- Monospace, code blocks, logs, command snippets, and structured metadata need careful hierarchy.

## Required Patterns

- Workspace shell with navigation, command/action region, output/result region, and detail/error panel.
- Copy buttons, status indicators, logs, retry actions, and clear failure classification.
- Keyboard-friendly controls and visible focus.
- Code/log formatting with wrapping or horizontal scroll.
- Empty states that help the developer start.

## Layout

- Split panes are acceptable when both panes are actively used.
- Terminal/log panels must not dominate unrelated workflows.
- Mobile support may be limited if the product is desktop-first, but narrow view should not break.

## Avoid

- Using developer jargon in a non-developer product.
- Dumping raw JSON or logs without structure when the user needs decisions.
- Hiding destructive runtime actions beside routine copy/download controls.
