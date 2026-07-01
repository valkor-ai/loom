# UIX Scenario: Fintech Workstation

Use for staff-facing finance, securities, account, risk, compliance, and transaction operations. The UI must support accuracy, auditability, and business-rule clarity.

## Baseline

- First viewport is an operational workstation: queue/list, filters, detail, and action path.
- Density is usually `workbench_dense` with clear visual grouping.
- Business states and blocking rules must be more prominent than decorative branding.
- Numeric data uses tabular alignment and clear units/currency/date formats.

## Required Patterns

- Record search and filtering by business identifiers.
- List/detail workflow with state, risk, eligibility, and latest event visible.
- Business actions tied to the current record: open, approve, reject, freeze, close, reissue, bind, deposit, withdraw, etc. as applicable.
- Rule feedback near the action: why blocked, what condition must change, and whether retry is possible.
- Audit trail or event history when the domain requires traceability.
- Staff-facing Chinese copy or project language for all visible labels.

## Layout

- Desktop: sidebar/topbar shell, list/table main region, right detail/action panel.
- Forms should group identity, account, authorization, money/security, and status sections.
- Mobile or narrow views use record cards plus full-screen detail/actions; never squeeze critical financial tables into unreadable columns.

## States

- Loading: scoped skeleton for list/detail/action panel.
- Empty: distinguish no records, no search results, and not yet configured.
- Error: recoverable and free of stack traces or internal tool names.
- Business-blocking: explicit rule message and affected field/object.
- Success: refresh list/detail and show new status/event.

## Avoid

- Generic "finance" visual cliches that reduce readability.
- Hiding risk or eligibility behind tooltips only.
- Treating business-rule blocks as generic errors.
