# Planning Discipline

Use this reference during Brainstorm, architecture/candidate generation, task planning, or phase continuation.

## Keep scope explicit

- Preserve the confirmed phase scope; do not widen it during planning or execution.
- Separate current delivery obligations from next-phase seeds.
- Record exclusions when they prevent accidental work later.

## Slice vertically

- Prefer thin, complete slices that cross the required layers and can be verified on their own.
- Avoid horizontal layer-only tasks unless the request is explicitly infrastructure-only, migration-only, or cleanup-only.
- If prefactoring is needed, make it a bounded enabling task with its own verification signal.

## Make tasks executable

- Each task should have a clear behavior, source refs, expected changed surface, acceptance intent, and result evidence.
- Keep dependencies explicit and order blockers before dependent tasks.
- Prefer one durable verification path per task over many vague checks.

## Avoid overplanning

- Do not invent future features to make the plan feel complete.
- Keep speculative alternatives as notes unless the user or returned instruction asks for a decision.
- Let `continue` route the next step instead of hand-selecting internal Loom nodes.
