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

## Content Surfaces

- Identity hero: brand/object/place is visible immediately; supporting copy explains role and audience.
- Proof: cases, credentials, locations, products, media, team, customers, or outcomes.
- Resource/news: list items need date/category/title and clear route.
- Contact: business path, form or contact channels, and validation/success/error states when interactive.

```html
<section data-region="identity-hero">
  <div data-region="identity-copy"></div>
  <figure data-region="identity-media"></figure>
</section>
```

## Verification Signals

- Brand/product/place/object is visible in the first viewport.
- Navigation, contact, and footer links support real stakeholder tasks.
- Images/media remain inspectable and not only atmospheric.

## Avoid

- Generic corporate hero with abstract gradients and no concrete identity.
- Overloaded nav that hides primary actions.
- Stock-like imagery that does not reveal the actual organization or offering.

## Proof And Conversion Continuity

The page must connect identity to credible proof and a stakeholder action. A
hero sentence without evidence or a reachable contact path is an incomplete
corporate surface.

```text
identity -> capability or offering -> proof -> relevant resource -> contact/action
```

- Match each major claim with inspectable proof such as a case, location, product detail, credential, person, document, or outcome.
- Give proof items an identity, category, date, and destination when they link to deeper content.
- Keep the primary contact or inquiry action available after the proof section; do not make the user return to the hero.
- Interactive contact forms need field-level validation, preserved input, submitting state, success confirmation, and recoverable failure.
- Treat media as evidence when it reveals the organization, venue, product, people, or work. Decorative atmosphere cannot carry the main claim.

## Responsive Identity

At narrow widths, preserve the identity signal and stakeholder path before
reducing decoration. Use a single-column reading order that keeps the object,
proof, and action connected.

```css
.identity-hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 0.8fr);
  gap: var(--space-8);
  align-items: center;
}

@media (max-width: 767px) {
  .identity-hero { grid-template-columns: 1fr; gap: var(--space-5); }
  .identity-hero [data-region="identity-media"] { order: -1; }
}
```

Do not hide the organization name, location, contact route, or proof labels
behind hover-only interactions on mobile.
