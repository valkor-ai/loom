# UIX Scenario: Fintech Workstation

Use for staff-facing finance, securities, account, risk, compliance, and transaction operations. The UI must support accuracy, auditability, and business-rule clarity.

## Baseline

- First viewport is an operational workstation: queue/list, filters, detail, and action path.
- Density is usually `workbench_dense` with clear visual grouping.
- Business states and blocking rules are more prominent than decorative branding.
- Numeric data uses tabular alignment and clear units/currency/date formats.

## Workstation Shell

```css
.fintech-workstation {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  background: var(--surface);
}

.record-workspace {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(420px, 1fr) minmax(360px, 440px);
  gap: var(--space-4);
  padding: var(--space-4);
}

@media (max-width: 1100px) {
  .record-workspace {
    grid-template-columns: minmax(0, 1fr);
  }
}
```

## Required Patterns

- Search by business identifiers: account id, customer id, order id, transaction id, certificate id, or similar domain keys.
- List/detail workflow with current state, risk, eligibility, and latest event visible.
- Business actions tied to the selected record: open, approve, reject, freeze, close, reissue, bind, deposit, withdraw, etc. as applicable.
- Rule feedback near the action: why blocked, what condition must change, and whether retry is possible.
- Audit trail or event history when the domain requires traceability.
- Staff-facing Chinese copy or project language for all visible labels.

## Record Detail Anatomy

```html
<section data-region="record-detail">
  <header data-region="record-summary"></header>
  <section data-region="eligibility"></section>
  <section data-region="action-panel"></section>
  <section data-region="domain-fields"></section>
  <section data-region="event-history"></section>
</section>
```

```css
.money,
.quantity,
.account-number {
  font-variant-numeric: tabular-nums;
}

.rule-block {
  border: 1px solid var(--danger-border);
  background: var(--danger-surface);
  color: var(--danger-text);
  border-radius: var(--radius-md);
  padding: var(--space-3);
}
```

## Action And Risk Pattern

```html
<section data-region="action-eligibility">
  <dl data-region="risk-facts"></dl>
  <div data-region="eligible-actions"></div>
  <div data-region="blocking-rules" aria-live="polite"></div>
</section>
```

- Show eligibility before high-risk actions.
- Separate warning, error, and business-blocking status.
- Keep actor, timestamp, affected account/order, and latest state visible for audit-sensitive flows.
- Use explicit confirmation/review steps for irreversible or regulated actions.

## States

- Loading is scoped to list/detail/action panel.
- Empty distinguishes no records, no search results, and unavailable permission.
- Technical errors are recoverable and free of stack traces or internal tool names.
- Business-blocking messages cite the rule and affected object.
- Success refreshes list/detail and appends or updates the visible event.

## Verification Signals

- Numeric values use tabular alignment and units.
- Business-blocking copy cites the affected record and rule.
- Audit/history area changes after successful state transitions when required.
- No financial/risk state is communicated by color alone.

## Avoid

- Generic "finance" visual cliches that reduce readability.
- Hiding risk or eligibility behind tooltips only.
- Treating business-rule blocks as generic errors.
- Showing financial numbers without units, sign, date, or status.

## Dense Financial Workbench

Density supports repeated review only when identity and risk remain visible. Use
stable columns and compact rows for comparison, then expand the selected record
without losing the work queue.

```text
queue filters -> record identity/status -> amount and key dates -> eligibility
-> selected detail -> approval/rejection -> row + audit update
```

- Keep queue criteria, selected record identity, status, amount, currency, and action eligibility in the first scan path.
- Use tabular numerals and right alignment for comparable amounts, but keep currency and unit labels adjacent.
- Keep a stable detail region or route that preserves queue position, filters, and selection after navigation.
- Represent pending, escalated, blocked, approved, rejected, and reversed states with text and an explicit reason where applicable.
- Avoid decorative KPI panels that push the queue or approval action below the first useful viewport.

## Risk And Audit Continuity

- High-impact actions show a review summary, actor/role, authorization condition, and confirmation requirement before submission.
- Rejection or escalation requires a reason that appears beside the affected decision and in the history view.
- Audit events include timestamp, actor, action, previous state, next state, and a readable reference to the affected record.
- If an action is unavailable, show the business rule or missing permission near the action rather than silently disabling it.

```html
<aside data-region="decision-panel">
  <dl data-region="approval-facts"></dl>
  <p data-region="eligibility-message"></p>
  <textarea data-region="decision-reason"></textarea>
  <div data-region="decision-feedback" aria-live="polite"></div>
</aside>
```
