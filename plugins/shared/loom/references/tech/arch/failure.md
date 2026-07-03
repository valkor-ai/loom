# Failure Modes And Architecture Risks

Use this reference when writing behavior failure paths, runtime failure expectations, or `architectureQuality.risks`.

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

- `riskId`
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

Only record risks that affect current implementation, verification, or repair routing.

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

## Review Routing

- Risk missing from AAC when failure mode is architectural -> architecture repair.
- Risk exists but no task owns mitigation -> taskplan repair.
- Task owns risk but result gives no evidence -> execution repair.

## Anti-Patterns

- Treating all errors as generic 500 responses.
- Putting validation only in frontend code.
- Ignoring partial writes or duplicate submissions.
- Using a broad risk with no owner artifacts.
- Creating risks for future phases that current tasks cannot mitigate.
