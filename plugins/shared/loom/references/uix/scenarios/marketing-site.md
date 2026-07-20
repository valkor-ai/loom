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

## Conversion Surface Quality

- Hero media should reveal the product, place, person, object, workflow, or outcome when those exist.
- Proof sections should use concrete evidence: screenshots, real metrics with context, customer logos, comparisons, demos, or examples.
- Pricing/comparison sections must be scannable and responsive; do not hide plan differences in long prose.
- Forms and CTAs need validation, success, and error states like any other product UI.
- The first viewport should hint at the next section so the page feels continuous, not like a full-screen poster.

## Verification Signals

- H1 is the product/offer/category/name, not a vague slogan.
- Primary and secondary CTAs remain visible and readable across breakpoints.
- Media has accessible alt/labels and does not obscure text.

## Avoid

- Generic "hero + 4 features + testimonials + CTA + footer" without product specificity.
- Gradient-only hero backgrounds when relevant media/product view is needed.
- Decorative screenshots that are too small, blurred, or cropped to inspect.
- Marketing sections inside an operational product first screen.

## Scroll Rhythm And Proof

Marketing pages need a readable sequence from offer to evidence to action. Each
section should earn its space by answering a buyer question or enabling the
next decision.

```text
offer/object -> problem or outcome -> concrete proof -> comparison/details
-> objection handling -> primary conversion action
```

- Put the literal product, place, object, or offer in the first viewport; descriptive value propositions support it.
- Pair claims with inspectable media, product states, customer evidence, comparison facts, or a concrete demonstration.
- Keep one primary conversion action per section and preserve a clear route to it after media or interaction.
- Use section transitions to establish hierarchy, not to create decorative whitespace that hides the next useful content.
- Repeat the essential action only when the page is long enough to justify it, and keep its label consistent.

## Media And Interaction

- Images and video reveal the actual offering and remain readable on mobile; do not use dark or blurred media where inspection matters.
- Interactive demos expose a stable fallback, keyboard alternative, reduced-motion behavior, and a clear reset path.
- Carousels expose slide identity, controls, pause behavior, and a non-animated way to inspect every item.
- Forms show field-level errors, preserve input, prevent duplicate submission, and confirm the submitted destination or next step.
