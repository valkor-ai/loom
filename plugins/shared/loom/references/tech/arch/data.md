# Data Architecture

Use this reference when Architecture needs to describe data ownership, transactions, invariants, migrations, and consistency. Do not use it to choose a concrete database product; Technical Baseline owns that selection.

## Inputs

Read the confirmed technical baseline for selected persistence technologies and frameworks. Then decide how the current phase should use them.

Examples:

- If the baseline selected relational storage, define table/entity ownership, constraints, transaction boundaries, and migration expectations.
- If the baseline selected document storage, define aggregate boundaries, document shape ownership, query shape, and update consistency.
- If the baseline selected key-value/cache storage, define source of truth, cache invalidation, TTL, and fallback behavior.
- If the baseline selected search storage, define source-of-truth sync, indexed fields, stale index behavior, and rebuild expectations.
- If the baseline selected time-series storage, define timestamp ownership, retention, rollups, late-arriving data, and query windows.
- If the baseline selected graph storage, define node/edge ownership, traversal boundaries, and consistency with source entities.
- If the baseline selected object/file storage, define metadata ownership, lifecycle, access control, cleanup, and link persistence.
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
| Retention and cleanup | Which records/files/events expire, archive, or require manual cleanup when current scope creates durable data. |
| Derived data | How indexes, projections, cached values, or denormalized fields are rebuilt and verified. |

## Store-Specific Modeling

Use selected storage facts from Technical Baseline; do not select a new product here.

| Store Shape | Architecture Should Define |
|---|---|
| Relational | entity/table owner, constraints, joins, transaction boundary, migration owner, index expectation. |
| Document | aggregate document owner, embedded vs referenced data, update atomicity, schema compatibility, query projections. |
| Key-value/cache | source of truth, key namespace, TTL, invalidation, fallback, cache miss behavior. |
| Search | indexed fields, analyzer/search behavior if relevant, sync trigger, stale result tolerance, rebuild strategy. |
| Time-series | timestamp semantics, retention, aggregation/rollup, late data handling, query window limits. |
| Graph | node/edge ownership, traversal depth, consistency with source records, cycle or orphan handling. |
| Object/file | metadata record, storage key ownership, access policy, cleanup on failed writes, orphan detection. |

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
- orphaned files, stale search indexes, expired cache surviving source updates, or graph edges drifting from source records

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
