# Code Review Method

Use this reference to inspect a completed change with professional skepticism, repository context, and risk-proportionate depth. Exact output fields, decision values, and workflow routing are supplied separately; this file owns the technical review method.

## Review Posture

Treat implementation summaries, test reports, and author comments as claims to verify. Assume good intent without assuming correctness.

Understand the requested outcome and affected user/system behavior before reading details. If the purpose is unclear, identify the missing decision rather than reviewing style in a vacuum.

Review the change that exists, not an imagined redesign. Prefer the smallest correction that restores the accepted behavior and repository architecture.

Separate correctness and risk from personal preference. Existing formatter, linter, framework convention, and local patterns decide style unless the style creates a concrete defect.

Do not edit source while acting as reviewer. A review must preserve independent observation and leave implementation to the repair owner.

## Scope Reconstruction

Identify changed files, generated versus source-owned files, public interfaces, data/schema changes, runtime/config changes, tests, migrations, dependencies, and deployment impact.

Trace how changed entry points call into domain, data, integration, and UI boundaries. A small diff in a shared helper, migration, policy, or configuration file can have a wider blast radius than a large isolated feature.

Check repository status and surrounding code so unchanged assumptions, user changes, and generated output are not misattributed to the current work.

Read enough context around each change to understand invariants and callers; avoid judging an isolated line without its ownership boundary.

## Review Order

1. Confirm the accepted behavior and scope.
2. Inspect architecture and public contract changes.
3. Trace primary and important negative paths through changed code.
4. Apply language/framework/data references selected for the changed tasks.
5. Assess security, concurrency, reliability, performance, and operational effects according to risk.
6. Evaluate test and runtime evidence against the behavior changed.
7. Write only concrete findings whose impact and repair are supportable.

Finish spec compliance before spending time on polish. Code quality matters only for code that belongs in the change.

## Risk-Based Depth

Increase depth for authorization, tenant isolation, money, destructive actions, personal/sensitive data, migrations, concurrency, async jobs, caches, public APIs, shared libraries, deployment/configuration, and irreversible external effects.

For high-risk paths, inspect both allow and deny behavior, transaction/rollback boundaries, idempotency/retry, stale version handling, audit/log exposure, and recovery after partial failure.

For narrow low-risk changes, focused source inspection and targeted tests may be sufficient. Do not demand broad integration work unrelated to the changed contract.

## Repository Fit

Check whether the change follows existing module ownership, dependency direction, error model, naming, API shape, state management, and test approach.

A new abstraction should isolate real complexity or serve real consumers. Flag speculative layers only when they add maintenance/risk or obscure behavior, not because all one-use helpers are inherently wrong.

Preserve accepted stack and dependency policy. New packages require a concrete capability, compatibility, security/license, runtime, and maintenance rationale.

## Change Interaction

Look across files for contracts that must move together: API server/client, schema/model/migration, route/link, config/env/deploy, serializer/type, UI state/action, cache/invalidation, and implementation/test fixture.

Check deletion and rename fallout: stale imports, routes, migrations, feature flags, docs/config, generated clients, deployment assets, and compatibility boundaries.

Trace repeated execution, process restart, concurrent requests, account/tenant switch, and old persisted data where those are plausible lifecycle events.

## Review Restraint

Do not block on hypothetical scale, future extensibility, broad refactors, or unmeasured optimization. State an assumption only when it materially affects current correctness.

Do not repeat automated diagnostics as findings without explaining the underlying impact and changed location.

Avoid praise, questions, and suggestions that bury blocking issues. Positive observations are useful only when they clarify why a pattern should be preserved.

## Completion Check

- Every blocking observation is grounded in current changed behavior or required evidence.
- File/location and impact are specific enough for another engineer to reproduce the issue.
- Similar symptoms sharing one root cause are not duplicated.
- Minor notes are genuinely non-blocking and do not conflict with the overall decision.
- The proposed repair stays within the smallest responsible boundary.

## Unsafe Review Defaults

- Trusting summaries or green checks without mapping them to changed behavior.
- Reviewing only the diff while ignoring callers and contract counterparts.
- Treating formatter/linter preferences as defects.
- Demanding future architecture unrelated to current requirements.
- Sending a source defect to human judgment when code repair is clear.
- Writing many vague findings instead of one evidenced root cause.
