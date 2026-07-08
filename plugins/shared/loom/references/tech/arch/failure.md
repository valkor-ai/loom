# Failure Modes And Architecture Risks

Use this reference when writing behavior failure paths, runtime failure expectations, or architecture risk records.

Failure modes are part of architecture, not a Review afterthought.

## Risk Categories

| Category | Examples |
|---|---|
| data_integrity | Duplicate records, invalid lifecycle transition, partial write, stale read, schema drift. |
| integration | External service unavailable, invalid response, retry duplication, contract mismatch. |
| runtime | Build/start failure, missing env, wrong probe path, background worker not running. |
| security | Unauthorized operation, sensitive data leak, unsafe error message. |
| operability | No observable signal for critical failure, unclear recovery path. |
| maintainability | Unclear ownership, duplicated business rules, hidden framework behavior. |

## Required Risk Shape

Each risk must include:

- stable risk id
- category
- severity
- likelihood
- impact
- mitigation
- owner artifact refs
- verification hints

Severity describes impact. Likelihood describes probability. Do not conflate them.

## Failure Mode Checklist

For each stateful or externally visible flow, consider:

- invalid input
- duplicate operation
- forbidden state transition
- missing authorization or role
- related record not found
- dependency unavailable
- write succeeds but follow-up step fails
- read model stale or incomplete
- runtime surface starts but API is unreachable
- user-visible feedback hides the real blocking reason

Only record risks that affect current implementation, verification, or repair ownership.

## Mitigation Quality

Good mitigation:

- names the owner module/interface/task area
- states the design or code behavior
- can be verified by tests, static checks, runtime probes, or review

Weak mitigation:

- "handle errors"
- "add validation"
- "make it robust"
- "monitor later"

## Repair Ownership

- Missing architectural failure-mode risk -> architecture owner.
- Risk exists but no task owns mitigation -> task planning owner.
- Task owns risk but implementation gives no evidence -> implementation owner.

## Anti-Patterns

- Treating all errors as generic 500 responses.
- Putting validation only in frontend code.
- Ignoring partial writes or duplicate submissions.
- Using a broad risk with no owner artifacts.
- Creating risks for future phases that current tasks cannot mitigate.
