# SQL Dialect Quality

Use this topic reference when `tech/code/sql/dialects.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies when SQL must match a specific database engine.

## When To Use

- The task changes migrations, schema DDL, hand-written SQL, ORM native queries, pagination SQL, date/time logic, JSON queries, upsert behavior, or database-specific configuration.
- Use this when PostgreSQL, MySQL/MariaDB, SQLite, SQL Server, Oracle, or another selected database affects syntax, type mapping, constraints, indexes, or runtime behavior.
- If the repository delegates all SQL generation to an ORM and the task does not touch SQL or mapping semantics, do not add dialect-specific work.

## Implementation Focus

- Treat the confirmed target database and existing migrations as the source of truth. Do not mix syntax from another engine because it is familiar.
- Choose identity/auto-increment syntax for the target dialect and ORM mapping: `IDENTITY`, `SERIAL`, `AUTO_INCREMENT`, sequences, UUIDs, or SQLite integer primary keys as appropriate.
- Map booleans, enums, UUIDs, JSON, timestamps, decimals, text/blob, and generated columns to the target dialect and the application layer. Watch SQLite type affinity and ORM schema validation behavior.
- Date/time arithmetic, current timestamp functions, interval syntax, and timezone handling must be dialect-correct and consistent with application expectations.
- Pagination must use the target dialect's stable pattern and deterministic ordering. Do not rely on offset pagination without an `ORDER BY`.
- JSON operators, generated columns, GIN indexes, functional indexes, full-text search, and collation/case-insensitive search are dialect features; use them only when the target database supports them and they are justified by the task.
- Upsert semantics differ by dialect. Preserve conflict target, update columns, and idempotency behavior explicitly.
- Identifier quoting, reserved words, parameter markers, string concatenation, and case sensitivity must match the database and driver.
- Keep migration SQL, ORM mappings, and integration tests aligned so startup/schema validation fails early when a type or constraint is wrong.

## Verification Focus

- Run migrations or schema initialization against the configured target/test database, not a different dialect, when dialect behavior is the point of the task.
- Verify ORM startup/schema validation when entity mappings or migrations changed.
- Execute the changed native query, pagination, upsert, JSON expression, or date/time expression with representative data.
- If the target database is unavailable, record the exact dialect behavior that could not be verified and keep the SQL conservative.

## Evidence Notes

- Record `sql.dialects` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/sql/dialects.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the dialect decision: identity, type mapping, timestamp/date arithmetic, pagination, JSON, upsert, collation, ORM compatibility, or target-database proof.
