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

## Result And Error Surfaces

```html
<section data-region="tool-runner">
  <header data-region="tool-controls"></header>
  <section data-region="tool-output"></section>
  <aside data-region="tool-diagnostics"></aside>
</section>
```

- Output should have status, duration/progress when known, copy/download where useful, and clear empty state.
- Configuration forms should validate before running costly or destructive actions.
- Raw JSON/logs can be shown, but only with structure, search, wrapping/scroll, and explanation of what the user should do.
- Dangerous actions should be separated from routine controls by placement and style.

## Verification Signals

- Keyboard and copy flows work.
- Long logs or JSON do not overflow the page.
- Technical errors are useful to the developer without exposing unrelated internal workflow details.

## Avoid

- Using developer jargon in a non-developer product.
- Dumping raw JSON or logs without structure when the user needs decisions.
- Hiding destructive runtime actions beside routine copy/download controls.

## Safe Technical Output

Technical output is part of the product only when it helps the user inspect or
operate the tool. Keep raw output bounded and distinguish user-actionable
failure from diagnostic detail.

```html
<section data-region="result" aria-live="polite">
  <header data-region="result-summary"></header>
  <div data-region="actionable-message"></div>
  <details data-region="diagnostic-output">
    <summary>Details</summary>
    <pre><code></code></pre>
  </details>
</section>
```

- Put status, duration, result identity, and the next action in the summary region.
- Keep stack traces, request payloads, and verbose logs behind an explicit detail boundary when they are useful but not required for the next decision.
- Redact secrets, tokens, personal data, and environment paths before rendering or copying output.
- Provide copy and retry actions with clear success and failure feedback; do not make users select a log with the pointer.
- Preserve the command, query, or selected object when a run fails so correction does not require rebuilding the setup.

## Keyboard Workflow

Keyboard movement must follow the workspace order: navigation, input/command,
run action, result, then details. Focus should move to newly available result
or error content without trapping the user in a panel.

- Every icon-only command has an accessible name and a visible focus state.
- Shortcut actions must have a pointer-accessible equivalent and must not fire while the user is typing in a text editor.
- A resizable panel has a keyboard-accessible alternative or a usable default width.
- Copy, retry, cancel, and expand actions report their result through local status text.
