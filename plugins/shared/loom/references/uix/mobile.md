# UIX Focus: Mobile

Load this when a web surface must be responsive, when a mobile/native scenario is selected, or when the task changes touch behavior.

## Baseline

- Mobile is not a squeezed desktop.
- One primary task per screen is the default.
- Touch targets are comfortable and separated.
- Sticky top/bottom bars respect safe areas and do not hide content.
- Hover-only behavior is invalid.

## Layout

- Use single-column task flow for forms and details.
- Convert dense tables into list/detail cards or drill-down routes unless comparison truly requires horizontal table scroll.
- Keep primary action visible near the end of the task or in a safe sticky region.
- Collapse sidebars to drawers, rails, or bottom navigation.
- Use mobile viewport units and safe-area padding when full-height screens are used.

## Inputs

- Use correct input types for number, email, phone, date, search, and password.
- Keep labels visible.
- Place validation next to the field.
- Preserve values when the keyboard opens/closes or validation fails.

## Verification

- Check narrow viewport, keyboard behavior, scroll, touch targets, and sticky bars.
- Check long labels and business messages in the target language.
- Check that error and success feedback remain visible after submit.
