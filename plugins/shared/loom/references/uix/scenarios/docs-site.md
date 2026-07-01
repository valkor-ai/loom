# UIX Scenario: Docs Site

Use for documentation, knowledge bases, API references, guides, technical manuals, and help centers.

## Baseline

- First viewport shows documentation structure and a direct path to useful content.
- Density is `balanced`.
- Reading, searching, navigation, and code/examples are the core workflow.
- The UI should not look like a marketing page when the task is reference lookup.

## Required Patterns

- Left navigation or section index, content region, and optional right table of contents.
- Search or quick navigation when content volume requires it.
- Code blocks, callouts, tables, and examples with readable styling.
- Current page/section state and next/previous routes.
- Empty/no-result states for search.

## Layout

- Prose width around 60-75 characters.
- Sticky nav/TOC only when it does not cover content.
- Mobile: collapsible navigation and readable code overflow.

## Avoid

- Full-width paragraphs.
- Code blocks with poor contrast or no overflow behavior.
- Hiding docs navigation behind multiple clicks on desktop.
