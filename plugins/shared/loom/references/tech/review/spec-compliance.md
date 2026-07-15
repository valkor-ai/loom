# Specification Compliance Review

Use this reference before implementation-quality review to determine whether the change delivers the accepted behavior, constraints, and boundaries. Exact workflow field names remain outside this reference.

## Compliance Frame

Convert accepted requirements into observable obligations: actor, trigger, preconditions, inputs, decisions/rules, state transition, durable/external effects, user/API feedback, readback, and failure behavior.

Distinguish mandatory behavior, conditional behavior, deferred/excluded scope, and assumptions. Do not silently promote an implementation assumption into a requirement or treat deferred work as a current defect.

Use confirmed business language and existing repository behavior to resolve terms. Personal conventions and “typical UX” cannot override an explicit decision.

## Missing Requirement Checks

Trace each important obligation to implementation and evidence. Presence of a file, endpoint, model, or button does not prove the workflow completes.

Check primary behavior plus explicitly required negative paths: blocked transitions, validation, permission denial, duplicate action, stale data, conflict, not-found, unavailable dependency, and retry/recovery.

Confirm durable/external effects when the behavior depends on persistence, events, files, messages, payments, notifications, or another service.

Confirm readback when users or callers must observe a changed status, identifier, version, total, assignment, or generated output.

Check cross-surface closure: UI command to accepted interface, interface to application/domain behavior, persistence/integration effect, response/error mapping, and visible refresh/reconciliation.

## Scope Creep Checks

Identify user-visible workflows, permissions, entities, integrations, jobs, caches, abstractions, dependencies, infrastructure, and migrations not required by current scope.

Extra code is a finding only when it changes behavior, increases risk/maintenance, conflicts with architecture, or consumes an ownership boundary the change did not receive.

Necessary supporting code is not scope creep when it is the smallest way to make accepted behavior build, run, or remain safe.

Generated product UI must not display delivery progress, runtime commands, stack explanations, verification instructions, placeholders, or internal process language.

## Interpretation Gap Checks

Compare ambiguous implementation choices with confirmed decisions, accepted examples, existing analogous features, and domain invariants.

Inspect default ordering, timezone/date boundary, money/rounding, status semantics, empty/null meaning, identifier format, ownership/tenant scope, and retry/idempotency because these are common silent interpretation gaps.

If two interpretations remain valid and user-owned, report the exact unresolved choice. Do not invent a preferred answer and call the implementation defective.

## Contract Pair Checks

Review contracts that must stay aligned:

- API method/path/input/output/status/error/auth with client calls and tests.
- Entity/domain rules with database constraints and migration shape.
- State transitions with UI eligibility, API enforcement, and audit/history.
- Configuration variables with local/runtime/deploy consumption.
- Event/job payloads with producer, consumer, idempotency, and retry policy.
- UI action target with displayed record identity and returned readback.

A mismatch can violate scope even when each side compiles independently.

## Existing Behavior And Compatibility

Determine whether the accepted change preserves or intentionally changes existing public behavior, persisted data, URLs, API consumers, file formats, configuration, and deployment/runtime assumptions.

Do not demand compatibility when a confirmed breaking migration replaces the old contract. Do require migration/cutover evidence when old data or consumers continue to exist.

## Evidence Mapping

For each important obligation, identify source and verification evidence that proves the branch. A single broad test name or result summary is insufficient when it does not reveal the exercised behavior.

Missing evidence is not automatically missing implementation. Classify whether code is absent/wrong or behavior is merely unproved so the repair targets the right boundary.

## Compliance Finding Shape

A strong compliance finding states the accepted obligation, actual behavior, concrete source/evidence, user/system impact, and smallest correction.

Avoid “does not match spec” without naming the requirement and mismatch. Avoid bundling separate missing behaviors with unrelated scope creep.

## Approval Bar

The delivered behavior matches mandatory current scope, important negative paths are present, cross-boundary contracts close, deferred/excluded work stays out, and available evidence supports those conclusions.

## Unsafe Review Defaults

- Checking files or endpoints instead of complete behavior.
- Reviewing only the happy path when rejection/blocking is required.
- Treating all extra code as scope creep regardless of necessity/impact.
- Filling ambiguity with reviewer preference.
- Assuming compilation proves cross-surface contract alignment.
- Confusing missing evidence with confirmed missing implementation.
