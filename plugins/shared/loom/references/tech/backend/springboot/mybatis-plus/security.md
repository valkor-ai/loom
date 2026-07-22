# MyBatis-Plus SQL Safety

Treat SQL-fragment APIs as high-risk boundaries.

- `apply`, `last`, `inSql`, `notInSql`, `exists`, `notExists`, and `setSql` accept SQL fragments; use them only with fixed templates or validated allowlists.
- User input may be a bound value, never a column name, table name, order expression, or SQL fragment.
- Prefer lambda columns and typed conditions. Translate client sort/filter keys through a closed mapping.
- Review XML and annotation SQL for `${}` or equivalent string substitution. Use parameter binding for values.
- Keep tenant, ownership, authorization, and audit conditions in a uniform service/plugin boundary rather than adding them ad hoc in controllers.
- Block-attack and illegal-SQL interceptors are defense in depth; they do not make unsafe SQL safe.

## Verification Focus

Test accepted and rejected sort/filter inputs, malicious fragment attempts, tenant/ownership conditions, scoped updates/deletes, and the stable API error path for rejected input.

## Review Boundary

- Review both Mapper XML and Wrapper construction; checking only controller validation is insufficient.
- Keep rejected input observable through the existing API error contract without echoing raw SQL fragments.
- Treat a new SQL-fragment escape hatch as a security-impacting change requiring focused evidence.
- Verify that authorization and tenant filters cannot be removed by an alternate Mapper method.
- Keep security-sensitive SQL changes visible in the task's existing review evidence.
- Do not use SQL interception as a substitute for an explicit application policy.
