# Design System UIX

Load this file for design systems, tokens, component specs, theming, dark mode, localization, icon systems, motion systems, or design-system audits.

## Tokens

- Prefer existing design tokens over raw values. If tokens exist, components should consume semantic or component tokens, not hard-coded colors, spacing, type, radius, elevation, or motion values.
- Token tiers should stay clear: global raw values, semantic aliases, and component-scoped tokens.
- Token names should describe purpose, not visual accident. Prefer `color-action-primary` over `blue-500` in component code.
- If adding tokens, document usage context and keep them compatible with theming and future platform mapping.

## Components

- Component work should cover overview, anatomy, variants, props/API, states, behavior, accessibility, and usage rules.
- Required states usually include default, hover, focus, active, disabled, loading, error, and selected/current when relevant.
- Specify behavior, not just appearance: keyboard interaction, pointer/touch behavior, responsive behavior, motion, truncation, and edge cases.
- Reuse existing components and primitives before introducing new ones.

## Theming And Modes

- Dark mode, high-contrast mode, and brand variants should emerge from semantic tokens, not per-component one-off overrides.
- Do not rely on color alone for semantic state. Pair color with label, icon, shape, pattern, or position.
- Check contrast for body text, large text, UI boundaries, focus rings, data marks, and disabled states.

## Motion

- Use a small motion vocabulary: duration tokens, easing tokens, and clear choreography rules.
- Motion must communicate state, relationship, or feedback. Avoid decorative movement that slows repeated workflows.
- Respect `prefers-reduced-motion` or platform-equivalent settings globally.
- Loading spinners and progress indicators may remain when they convey essential state, but sliding, scaling, parallax, and rotation should reduce or disappear.

## Localization

- Design flexible containers for text expansion. Do not size controls to English copy only.
- Use logical layout properties when possible: inline/block, start/end, and `text-align: start` instead of left/right assumptions.
- Directional icons mirror in RTL; non-directional icons, logos, numerals, clocks, and brand marks usually do not.
- Test at least one long-expansion locale and one RTL-like layout when localization is required.

## Handoff

- Handoff-ready UI evidence should name token usage, component reuse, responsive behavior, state coverage, accessibility requirements, asset handling, and edge cases.
- When design-system compliance is uncertain, record it as a manual-review risk instead of silently inventing a parallel system.
