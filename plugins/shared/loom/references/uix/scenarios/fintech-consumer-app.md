# UIX Scenario: Fintech Consumer App

Use for customer-facing financial, trading, banking, wallet, or investment experiences. Trust, clarity, error recovery, and mobile usability matter more than novelty.

## Baseline

- First viewport supports the customer's current financial task: overview, account, transaction, order, payment, or investment action.
- Density is `comfortable` or `balanced`.
- Sensitive values, confirmations, and risk messages must be explicit and localized.
- The interface distinguishes pending, completed, failed, blocked, and reversible states.

## Mobile App Shell

```css
.finance-app {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  background: var(--surface);
}

.finance-content {
  min-width: 0;
  overflow: auto;
  padding: var(--space-4);
  padding-bottom: calc(var(--space-4) + env(safe-area-inset-bottom, 0));
}

.finance-bottom-nav {
  min-height: 64px;
  padding-bottom: env(safe-area-inset-bottom, 0);
  border-top: 1px solid var(--border);
  background: var(--surface-raised);
}
```

## Required Patterns

- Account/portfolio summary with units and last-updated context when relevant.
- Transaction/order/action forms with review step for high-risk actions.
- Inline validation and business-blocking messages.
- Confirmation receipt with reference id, status, and next action.
- Security and permission states without exposing implementation details.
- Mobile-first navigation and touch targets when the target is app-like.

## Financial Components

```html
<section data-region="account-summary">
  <header></header>
  <div data-region="primary-balance"></div>
  <div data-region="quick-actions"></div>
</section>

<section data-region="transaction-list"></section>
<section data-region="review-and-confirm"></section>
<section data-region="receipt"></section>
```

```css
.amount {
  font-variant-numeric: tabular-nums;
  letter-spacing: 0;
}

.transaction-row {
  min-height: 64px;
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr) auto;
  gap: var(--space-3);
  align-items: center;
}
```

## Sensitive Action Flow

```html
<section data-region="financial-action">
  <section data-region="input-step"></section>
  <section data-region="review-step"></section>
  <section data-region="receipt-step"></section>
</section>
```

- Review screens should show amount, account, fee/limit, timing, and irreversible consequences when relevant.
- Receipts should include reference id, timestamp, status, and next action.
- Security prompts should explain what the user must do, not how the system is implemented.
- Mask sensitive values when the product requires privacy, but keep enough context for confirmation.

## States

- Loading preserves balance/transaction layout and never shows misleading zero values.
- Empty explains whether no data exists, access is restricted, or setup is incomplete.
- Error includes recovery and support route for financial actions.
- Business-blocking distinguishes insufficient balance, eligibility, limit, compliance, and market state restrictions.
- Success receipt persists long enough to copy/share/reference.

## Verification Signals

- Loading states never imply an incorrect balance/order state.
- Risk/fee/limit information is visible before confirmation.
- Error recovery path is visible for failed financial actions.
- Mobile touch targets and safe areas are checked.

## Avoid

- Confirming risky actions only through a disappearing toast.
- Dark or "premium" styling that harms legibility.
- Hiding fees, limits, or irreversible consequences.
