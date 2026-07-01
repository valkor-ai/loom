# Admin Dashboard UIX

Use for staff consoles, back-office systems, ERP/CRM screens, review tools, account operations, internal workbenches, and business management surfaces.

## Baseline

- Efficiency and scanability matter more than spectacle.
- The first screen should be the work surface: navigation, page title, filters or primary actions, data list/table, detail panel, and task feedback.
- Use an app shell when the product has more than one business surface: sidebar or top navigation, page header, content region, and contextual actions.
- Keep density appropriate for repeated staff work. Prefer compact tables, grouped forms, drawers, tabs, and stable detail panels over oversized cards.

## Required Patterns

- Lists need toolbar/search/filter, pagination or virtual scrolling, selected-row state, empty/loading/error states, and row-level actions.
- Detail views need identity summary, current status, key fields, lifecycle or audit context when relevant, and actions close to the object.
- Forms need sections, labels, inline validation, submit loading, success feedback, business-blocking feedback, and preserved input on failure.
- Destructive or irreversible operations need confirmation, clear consequence copy, and post-action readback.

## Must Not

- Do not show runtime commands, stack descriptions, delivery notes, or verification instructions in the staff UI.
- Do not start with a marketing hero or "capability overview" cards.
- Do not mix unrelated future modules into the current phase navigation unless they are disabled and clearly out of scope.
