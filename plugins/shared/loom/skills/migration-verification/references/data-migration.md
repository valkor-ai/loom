# Data Migration

Identify the currently supported schema, each writer and reader, and the invariant the migration must preserve. Use representative existing rows, including nulls, duplicates, and boundary values where relevant. A clean database install cannot establish upgrade behavior.

When versions coexist, verify the additive schema with the old application before enabling new writes. Rehearse an interrupted backfill, resume from its last durable checkpoint, and repeat a completed batch to check idempotency. Cover concurrent updates so a stale backfill cannot overwrite a newer value.

Compare durable readback, rejected rows, and domain invariants before applying stricter constraints. Define reconciliation for any intentional difference. For large tables, use the actual database engine to check locks, transaction duration, and batch behavior.

Retire old fields after the approved compatibility window and after evidence shows no supported reader, writer, or recovery path depends on them.
