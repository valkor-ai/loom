# SQL Dialect Selection

Use this reference to keep portable SQL rules separate from provider-specific implementation rules.

## When To Use

- The task changes schema, migration SQL, hand-written SQL, ORM native queries, pagination, date/time expressions, JSON expressions, upsert behavior, or provider-specific mappings.
- The accepted Technical Baseline names a database provider whose syntax, type affinity, index behavior, or transaction behavior affects the task.
- Do not load dialect guidance for an API, controller, frontend, or service task that does not own persistence behavior.

## Source Facts

- Treat the accepted Technical Baseline, repository migration files, driver, ORM, and runtime configuration as the provider source of truth.
- Keep provider identity separate from task subject. A PostgreSQL selection does not by itself make a task a query, migration, performance, or transaction task.
- Do not infer a provider from a task title or business prose. The MCP selection uses structured stack signals and task ownership.
- If the provider version, extension, migration tool, or ORM mapping is unknown, use the portable subset and record the exact compatibility gap.

## Common Compatibility Rules

- Use the target provider's identity, type, timestamp, decimal, JSON, enum, quoting, parameter, and collation semantics.
- Keep schema or migration SQL, ORM mappings, serialized API fields, and persistence tests aligned.
- Preserve deterministic ordering for pagination and include a stable tie-breaker.
- Treat upsert conflict targets, affected-row behavior, and retry/idempotency semantics as part of the contract.
- Do not copy syntax from another provider because it looks equivalent.

## Provider Overlays

MCP maps the selected SQL reference group to a provider overlay only when the task owns the matching subject:

| Provider | Overlay references |
|---|---|
| MySQL | `mysql.schema`, `mysql.queries`, `mysql.transactions` |
| PostgreSQL | `postgresql.schema`, `postgresql.queries`, `postgresql.transactions` |

MariaDB remains a separate provider signal. It must not silently load the MySQL overlay without an explicit compatibility decision.

## Verification Focus

- Run provider-sensitive migrations, mappings, or queries against the configured target/test provider.
- Verify ORM startup/schema validation when mappings or migrations change.
- Use representative data for type, null, ordering, JSON, upsert, lock, or pagination behavior owned by the task.
- If the provider is unavailable, record the exact behavior that was not verified and keep implementation conservative.

## Evidence Focus

Name the selected provider and decision proved: type mapping, identity, time semantics, pagination, JSON, upsert, index behavior, transaction behavior, or target-provider proof.

## Anti-Patterns

- Loading every provider overlay for every persistence task.
- Treating a provider name as permission to add provider-only features.
- Claiming dialect compatibility from a different database such as SQLite or H2.
- Hiding version, extension, ORM, or migration incompatibility behind a generic successful unit test.
