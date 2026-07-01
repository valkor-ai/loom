# UIX Scenario: Corporate Site

Use for company, organization, venue, portfolio, institution, or brand information sites.

## Baseline

- The brand, organization, place, or object must be a first-viewport signal.
- Navigation should make stakeholder tasks obvious: overview, services, cases, news, contact, careers, docs, or support.
- Visual tone should fit the organization without falling into category-reflex palettes.
- Density is `comfortable`.

## Structure

```html
<main data-region="corporate-site">
  <section data-region="identity-hero"></section>
  <section data-region="capabilities"></section>
  <section data-region="proof"></section>
  <section data-region="resources-or-news"></section>
  <section data-region="contact"></section>
</main>
```

```css
.corporate-section {
  padding: var(--space-12) var(--space-6);
}

.corporate-inner {
  width: min(100%, 1180px);
  margin: 0 auto;
}

.proof-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: var(--space-6);
}
```

## Required Patterns

- Clear identity and primary audience path.
- Structured content sections with real proof: capabilities, cases, locations, people, history, media, or resources.
- Contact/conversion paths that are visible and accessible.
- Responsive media that remains inspectable.
- Footer with useful navigation, not filler.

## Layout Rules

- Use editorial hierarchy, section bands, and media composition.
- Keep prose readable and avoid stretching paragraphs on wide screens.
- Use cards only for repeated content such as cases, people, articles, or resources.
- Preserve a hint of the next section in the first viewport when practical.

## Avoid

- Generic corporate hero with abstract gradients and no concrete identity.
- Overloaded nav that hides primary actions.
- Stock-like imagery that does not reveal the actual organization or offering.
