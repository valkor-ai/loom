# Test And Evidence Review

Use this reference to judge whether submitted evidence proves the changed behavior at a depth proportional to risk. Evidence quality depends on the claim-to-check mapping, not command count or suite size.

## Evidence Inventory

Classify available evidence as source inspection, compiler/type/static analysis, unit/component tests, integration/contract tests, migration/database checks, runtime/API/browser/device probes, deployment checks, or manual observation.

Record what environment/backend/provider/browser/device/config the check actually used. Do not generalize beyond that boundary.

Distinguish executed evidence from planned commands, copied output, fixture-only behavior, and author narrative.

## Strong Evidence

- The check exercises the changed branch through its public/owned boundary and asserts an observable result.
- Success plus a meaningful blocking/failure/concurrency path is covered when those outcomes drive different behavior.
- Durable writes are read back from the selected persistence integration where persistence correctness changed.
- Interface checks assert method/path/input/status/output/error/auth behavior, not only server startup or an empty list.
- UI checks prove relevant loading/empty/ready/error/disabled/submitting/readback states and stable action targets.
- Migrations are applied/validated against representative existing schema/data and selected provider behavior.
- Security evidence checks deny/cross-user/tenant/ownership behavior, not only an authenticated happy path.
- Performance evidence states workload, build/runtime mode, baseline, measurement, and correctness guard.

## Weak Evidence

- Build, formatter, linter, or typecheck is the only evidence for changed runtime behavior.
- Tests assert private calls/state/snapshots while public behavior could remain wrong.
- A mocked collaborator bypasses the authorization, transaction, serialization, or integration behavior being claimed.
- Runtime probe checks health or first empty response for a multi-step workflow.
- Test name/summary claims a branch but assertions do not distinguish it.
- Browser screenshot proves appearance but not action, state, responsive, accessibility, or API binding.
- Command/outcome/environment is missing or the result predates the latest repair.

## Claim Mapping

For each important changed obligation, map implementation location, evidence check, assertion/result, and remaining limitation.

Verify that an evidence identifier points to an actual executed result and that the result belongs to the current task/change version. Stale pre-repair evidence must not approve post-repair code automatically.

One focused check can prove several tightly coupled obligations; one broad suite name cannot prove everything without visible assertions/results.

## Test Quality

Review determinism, isolation, representative fixtures, assertion specificity, cleanup, and failure sensitivity.

Look for tests that cannot fail because mocks return the implementation's desired value, expected values are derived by the same code, exceptions are ignored, assertions are absent, or retries hide flaky outcomes.

Check state/time/random/order/global environment cleanup and parallel safety. Arbitrary sleeps signal synchronization uncertainty.

For regression fixes, evidence should reproduce the old failure or assert the exact invariant that was broken.

## Layer Selection

Use unit tests for local logic, integration tests for boundary contracts, provider/runtime checks for infrastructure semantics, and browser/device tests for rendering/interaction/platform behavior.

Do not require the highest layer for every change. Select the cheapest layer that can actually prove the risk, then add higher-layer evidence for cross-module closure or environment-specific semantics.

Compiler/type/lint/build evidence remains valuable for public type, import, code generation, and production-bundle constraints but cannot replace behavior checks.

## Environment Limitations

Separate product failure from unavailable toolchain/browser/device/service/credentials. Preserve any source/static/lower-layer evidence still possible and state the exact unproved risk.

An environment gap blocks approval only when the required risk cannot be established at another credible layer and current policy requires that evidence.

Do not rerun the same unavailable command through generic code repair without an environment change.

## Scope And Cost

Focused tests are appropriate for isolated changes; shared contracts, migrations, authorization, runtime routing, framework configuration, and cross-surface workflows justify broader affected-lane checks.

Do not demand a full suite when a targeted regression plus affected package checks close the risk. Do not accept a tiny target when shared behavior has a wide blast radius.

## Evidence Finding Shape

State the unproved behavior, why current evidence cannot prove it, the risk, and the smallest additional check or correction. Avoid “needs more tests” without a concrete assertion and layer.

If source inspection confirms an actual defect, report the product defect rather than only an evidence gap.

## Approval Bar

Important changed behavior has current, credible, risk-proportionate evidence; negative/deny/recovery paths are covered where meaningful; provider/runtime limits are explicit; and no known gap contradicts completion.

## Unsafe Review Defaults

- Counting tests/commands instead of mapping claims.
- Treating green build/typecheck as runtime proof.
- Accepting stale evidence after repair.
- Demanding full end-to-end coverage for every local change.
- Routing environment unavailability as a source defect.
- Reporting “more tests” without naming behavior and assertion.
