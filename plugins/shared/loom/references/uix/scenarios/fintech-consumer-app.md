# UIX Scenario: Fintech Consumer App

Use for customer-facing financial, trading, banking, wallet, or investment experiences. Trust, clarity, error recovery, and mobile usability matter more than visual novelty.

## Baseline

- First viewport supports the customer's current financial task: overview, account, transaction, order, payment, or investment action.
- Density is `comfortable` or `balanced`.
- Sensitive values, confirmations, and risk messages must be explicit and localized.
- The interface must distinguish pending, completed, failed, blocked, and reversible states.

## Required Patterns

- Account/portfolio summary with clear units and last-updated context when relevant.
- Transaction/order/action forms with review step for high-risk actions.
- Inline validation and business-blocking messages.
- Confirmation receipt with reference id, status, and next action.
- Security and permission states without exposing implementation details.
- Mobile-first navigation and touch targets when the target is an app-like surface.

## Layout

- Mobile: one primary task per screen, sticky safe-area-aware primary action when useful.
- Desktop/responsive: summary plus detail panels; avoid overwhelming consumer users with staff-console density.
- Use cards for account/transaction summaries, but avoid decorative repeated cards with no workflow.

## States

- Loading: preserve balance/transaction layout and avoid misleading zero values.
- Empty: explain whether no data exists or access is restricted.
- Error: clear recovery and support route for financial actions.
- Business-blocking: distinguish insufficient balance, eligibility, limits, and compliance restrictions.

## Avoid

- Showing financial numbers without units, sign, date, or status.
- Confirming risky actions only through a disappearing toast.
- Dark or "premium" styling that harms legibility.
