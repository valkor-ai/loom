# UIX Scenario: Marketing Site

Use for landing pages, product marketing, campaigns, pricing, launches, and conversion-oriented pages.

## Baseline

- First viewport communicates the product, offer, or category directly.
- Visual media should reveal the product, result, place, person, or experience when possible.
- Hero text is not inside a card. The next section should be hinted below the fold on common viewports.
- Density is `immersive` or `comfortable`.

## Page Structure

```html
<main data-region="marketing-page">
  <section data-region="hero"></section>
  <section data-region="proof"></section>
  <section data-region="product-workflow"></section>
  <section data-region="comparison-or-pricing"></section>
  <section data-region="conversion"></section>
</main>
```

```css
.marketing-hero {
  min-height: min(760px, 88dvh);
  display: grid;
  align-items: end;
  padding: var(--space-8);
  position: relative;
  overflow: hidden;
}

.hero-copy {
  max-width: 760px;
  padding-bottom: var(--space-8);
  z-index: 1;
}

.hero-media {
  position: absolute;
  inset: 0;
  object-fit: cover;
}
```

## Required Patterns

- Clear headline, supporting value prop, primary CTA, and secondary route.
- Product proof: screenshots, real media, data, testimonial, comparison, demo, or workflow preview.
- Section rhythm with varied layout, not repeated equal cards.
- Responsive media and typography that do not overlap.
- Accessible CTA and navigation.

## Layout Rules

- Use full-bleed or immersive hero media when available.
- Avoid split text/card hero layouts that make the page look templated.
- H1 should be the brand/product/place/person name or literal offer/category.
- Supporting copy carries descriptive value props.
- Keep CTA group visible but not floating over unreadable media.

## Avoid

- Generic "hero + 4 features + testimonials + CTA + footer" without product specificity.
- Gradient-only hero backgrounds when relevant media/product view is needed.
- Decorative screenshots that are too small, blurred, or cropped to inspect.
- Marketing sections inside an operational product first screen.
