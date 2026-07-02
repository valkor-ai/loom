# UIX Scenario: Docs Site

Use for documentation, knowledge bases, API references, guides, technical manuals, and help centers. Reading, search, navigation, and examples are the product workflow.

## Baseline

- First viewport shows documentation structure and a direct path to useful content.
- Density is `balanced`.
- The UI should not look like a marketing page when the task is reference lookup.
- Content width, code readability, navigation state, and search quality matter more than decoration.

## Docs Shell

```html
<div data-region="docs-shell">
  <header data-region="docs-topbar"></header>
  <aside data-region="docs-nav"></aside>
  <main data-region="docs-content"></main>
  <aside data-region="docs-toc"></aside>
</div>
```

```css
.docs-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: 260px minmax(0, 760px) 220px;
  justify-content: center;
  gap: var(--space-8);
  padding: 0 var(--space-6);
}

.docs-nav,
.docs-toc {
  position: sticky;
  top: 64px;
  height: calc(100dvh - 64px);
  overflow: auto;
}

.docs-content {
  min-width: 0;
  padding: var(--space-8) 0 var(--space-12);
  line-height: 1.65;
}

@media (max-width: 1180px) {
  .docs-shell {
    grid-template-columns: 240px minmax(0, 760px);
  }
  .docs-toc {
    display: none;
  }
}

@media (max-width: 767px) {
  .docs-shell {
    display: block;
    padding: 0 var(--space-4);
  }
  .docs-nav {
    position: fixed;
    inset: 0 auto 0 0;
    width: min(300px, 86vw);
    transform: translateX(-100%);
    background: var(--surface);
    z-index: var(--z-modal);
  }
}
```

## Required Patterns

- Left navigation or section index, content region, and optional right table of contents.
- Search or command palette when content volume requires it.
- Code blocks with language label, copy control, readable contrast, and overflow behavior.
- Callouts for note/tip/warning/danger states.
- Current page/section state and next/previous routes.
- Search empty/no-result states with recovery suggestions.

## Content Anatomy

```html
<article data-region="doc-page">
  <header>
    <p data-region="eyebrow"></p>
    <h1></h1>
    <p data-region="summary"></p>
  </header>
  <section data-region="body"></section>
  <nav data-region="page-pagination"></nav>
</article>
```

```css
.docs-content h1 { font-size: var(--text-3xl); line-height: 1.15; }
.docs-content h2 { margin-top: var(--space-10); padding-bottom: var(--space-2); border-bottom: 1px solid var(--border); }
.docs-content p,
.docs-content li { max-width: 72ch; }
.docs-content pre { overflow: auto; border-radius: var(--radius-md); }
.docs-content code { font-family: var(--font-mono); }
```

## Documentation Interactions

- Search/no-result state should suggest alternate terms or navigation.
- Code examples need copy controls only when copy is implemented.
- API reference pages need parameter, response, error, and example sections with stable anchors.
- Guides need prerequisites, steps, expected result, and troubleshooting.
- Version, platform, or language switchers should show the current selection.

## Verification Signals

- Left nav active state and page heading agree.
- Long code blocks scroll without breaking the page.
- Mobile nav opens/closes and returns to content without losing scroll unexpectedly.
- The page can be read without marketing content blocking reference lookup.

## Avoid

- Full-width paragraphs.
- Code blocks with poor contrast or no overflow behavior.
- Hiding docs navigation behind multiple clicks on desktop.
- Marketing hero sections that delay access to documentation.
