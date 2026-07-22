# MyBatis-Plus Interceptors And Plugins

## When To Use

Use this reference when the task owns MyBatis-Plus pagination, tenant, data-permission, optimistic-lock, dynamic-table, block-attack, illegal-SQL, or SQL-diagnostic interception.

## Implementation Focus

Configure only the plugins required by the accepted architecture and task ownership.

| Capability | Required boundary |
|---|---|
| Pagination | Provider type, page-size limit, deterministic ordering, and no unbounded fallback |
| Optimistic locking | `@Version`, update-count conflict handling, and retry/rejection semantics |
| Tenant or data permission | Trusted context, uniform query coverage, explicit system-job and admin policy |
| Dynamic table name | Allowlisted suffix or routing rule; never free-form client input |
| Block attack | Final guard against unscoped update/delete, not a replacement for business conditions |
| Illegal SQL | Existing complex SQL must be checked for false positives before enabling |
| SQL analysis or printing | Development diagnostics only; redact sensitive values and avoid production overhead |

Plugin order is part of behavior. Verify every active `SqlSessionFactory`, interceptor order, excluded statements, tenant bypass rules, and interactions with XML/native SQL.

Do not add a global plugin to solve a local query. Do not claim that a plugin provides authentication, authorization, audit, or complete tenant isolation by itself.

## Verification Focus

Test permitted and denied tenant contexts, scoped and unscoped writes, stale versions, page limits, dynamic-table allowlists, plugin ordering, and false-positive interception paths.

## Change Boundary

- Register a plugin only on the factory and statements that own the capability.
- Document intentional bypasses for system jobs or administrative operations and test them separately.
- Keep provider-specific pagination and SQL rewriting evidence in the selected SQL reference.

## Evidence Focus

Record the active session factories, plugin order, excluded statements, bypass policy, provider assumptions, and focused permitted and denied-path results.
