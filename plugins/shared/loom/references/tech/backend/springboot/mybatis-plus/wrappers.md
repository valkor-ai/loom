# MyBatis-Plus Query And Update Wrappers

## Query Rules

- Prefer `LambdaQueryWrapper` or `Wrappers.lambdaQuery` for entity-backed columns.
- Use wrapper condition parameters for optional filters instead of duplicating branches.
- Map client sort keys to a fixed column allowlist. Never pass a user-provided field name directly to string-based ordering.
- Keep joins, grouping, subqueries, and provider-specific SQL in controlled XML or a dedicated query component when a Wrapper becomes opaque.

## Update Rules

- Use `LambdaUpdateWrapper#set(condition, column, value)` for conditional partial updates.
- Distinguish an omitted field from an explicit `null` before deciding whether to update it to `null`.
- Every update and delete needs a primary-key, tenant, ownership, or other explicit scope condition. Block-attack protection is only a final guard.
- Treat an update count of zero as meaningful when optimistic locking, ownership, or state transition rules apply.
- `setSql` is only for a controlled expression such as an atomic increment; bind values rather than concatenating them.

## Verification Focus

Cover omitted versus null fields, empty filters, sort allowlists, scoped writes, concurrent updates, and the exact generated result for complex conditions.

## Query Shape Boundary

- Use projections for list paths and keep selected columns aligned with the API contract.
- Keep large result sets bounded before materializing them in memory.
- Prove empty-condition behavior explicitly; an absent filter must not become an unscoped write or an accidental full-table read.
