# Data Architecture

Use this reference when Architecture needs to describe data ownership, transactions, invariants, migrations, and consistency. Do not use it to choose a concrete database product; Technical Baseline owns that selection.

## Inputs

Read the confirmed technical baseline for selected persistence technologies and frameworks. Then decide how the current phase should use them.

Examples:

- If the baseline selected relational storage, define table/entity ownership, constraints, transaction boundaries, and migration expectations.
- If the baseline selected document storage, define aggregate boundaries, document shape ownership, query shape, and update consistency.
- If the baseline selected key-value/cache storage, define source of truth, cache invalidation, TTL, and fallback behavior.
- If the baseline selected no persistence for this phase, record why state is derived, in-memory, or deferred.

## Required Decisions

| Area | Architecture Output |
|---|---|
| Ownership | Which module owns each entity, aggregate, or table/document. |
| Invariants | Which rules must be enforced before data is written or state changes. |
| Transactions | Which operations must be atomic and what can be eventually consistent. |
| Relationships | Which relationships are strong references, weak references, denormalized values, or derived views. |
| Migrations | What schema/data change is needed and which task should own it. |
| Read models | Which list/detail/search/query views are required and what fields they expose. |
| Failure behavior | How duplicate, stale, invalid, or partial writes are handled. |

## Current Phase Fit

Keep data architecture scoped to the active phase:

- Do not model future entities just because later phases may use them.
- Do not create generic "User", "Config", or "Audit" entities unless the phase requires them.
- Do not add cache/search/queue storage unless a current requirement depends on it.
- Do not weaken domain invariants to simplify scaffolding.

## NFR Hooks

Create NFR entries when data architecture creates quality obligations:

- data integrity: constraints, uniqueness, lifecycle safety
- reliability: transactional boundary, retry, idempotency
- performance: index/query expectation, pagination, bounded reads
- maintainability: migration readability, provider-compatible mappings
- observability: lifecycle events or error logging for critical transitions

## Risk Hooks

Create risk entries for:

- schema and domain model drift
- provider type mismatch
- validation existing only in UI
- partial write across multiple stores
- read model exposing stale or incomplete business state
- hidden dependency on default ORM/framework behavior

## Anti-Patterns

- Re-selecting the database in Architecture.
- Writing entities without owner modules.
- Treating DTO fields, entity fields, and persistence columns as unrelated.
- Deferring validation until UI or tests only.
- Omitting migration impact for persistent fields.

## TaskPlan Implications

Data-affecting tasks should usually own:

- entity/model changes
- persistence/migration changes
- repository/query changes
- API DTO/payload changes
- same-provider persistence tests or runtime readback checks

These may also carry engineering quality requirements; keep architecture quality and engineering quality as separate but complementary obligations.
