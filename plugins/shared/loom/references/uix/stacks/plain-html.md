# UIX Stack: Plain HTML

Use for static HTML, vanilla JavaScript, simple Tailwind/CDN pages, server-rendered templates, or projects without a frontend framework.

## Structure

- Keep semantic HTML first: landmarks, headings, forms, labels, tables, buttons, links.
- Use CSS variables for tokens when no design system exists.
- Split CSS and JavaScript if the project already has separate files; avoid massive inline style/script blocks for non-trivial products.

## Implementation Rules

- Progressive enhancement: core content and forms should remain understandable without complex JavaScript when possible.
- Use event delegation or small modules for interactions such as drawers, tabs, filters, toasts, and forms.
- Provide visible focus states and ARIA only where native semantics are insufficient.
- Use stable class names for layout primitives, states, and reusable components.

## Tailwind Notes

- If Tailwind is already present, map semantic decisions through config/classes rather than arbitrary one-off values.
- Avoid long class strings that hide repeated patterns; extract components or utility classes when repetition grows.

## Verification

- Open the HTML or local server render.
- Check keyboard navigation, responsive breakpoints, and form validation.
- Verify no product UI contains setup or verification instructions.
