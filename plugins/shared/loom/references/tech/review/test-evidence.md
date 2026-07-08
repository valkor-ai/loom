# Loom Review Test And Evidence Quality

Use this reference to judge whether the implementation evidence is strong enough for approval. Evidence quality is not measured by command count; it is measured by whether the executed checks prove the changed behavior and the accepted contract.

## Evidence Sources

- Automated tests: unit, integration, component, API, contract, migration, or end-to-end tests.
- Static checks: compiler, type checker, linter, formatter check, schema validation, migration validation, security scan.
- Runtime probes: API calls, page rendering, health checks, smoke flows, CLI commands, deployment probes.
- Manual inspection: screenshots, diff refs, source reads, or reviewer-observed behavior when automation is unavailable.
- Known gaps: explicit limitations with impact and a reason the gap is acceptable or blocking.

## Strong Evidence

- A new or changed business branch has a test or probe that exercises the branch.
- A blocking validation rule has both success and rejection coverage when practical.
- Persistence behavior is proven with durable write and readback when the task changes stored state.
- API contracts are checked for status, response shape, and error shape, not only server startup.
- UI work is rendered or inspected for required states, not only compiled.
- Migration or schema work is verified against the selected database or the repository's migration validation path.

## Weak Evidence

- Only a build command is run for a behavioral change.
- Only an empty-list API response is checked for a CRUD workflow.
- Tests assert implementation details while the accepted business behavior remains untested.
- Implementation evidence claims a command passed without naming the command or outcome.
- Verification ids exist but are not linked to requirement detail evidence.
- Known gaps are hidden in prose instead of recorded as limitations or pending actions.

## Evidence Review Steps

1. Map each important requirement detail to the task result evidence that claims it.
2. Confirm verification ids referenced by detail evidence actually exist.
3. Inspect changed files or diff refs when evidence is too broad or optimistic.
4. Downgrade approval when evidence cannot prove a must-level requirement.
5. Treat missing evidence as an implementation repair when tests or small code changes can produce it.
6. Escalate for human review only when the environment prevents reliable automated or source-based verification.

## Approval Bar

Approval is allowed when evidence is sufficient for the risk of the changed behavior. Small isolated changes can pass with focused checks. Cross-module workflow, persistence, security, payment, authorization, or deployment changes require broader evidence because the failure impact is higher.
