# Actionable Review Findings

Use this reference when converting verified observations into concise findings. Exact schema, enum values, references, and next actions are supplied by the active review request; this file owns finding quality.

## Finding Content

Each finding should make five facts clear:

1. What concrete behavior, contract, safety property, or evidence is wrong.
2. Where it occurs in a changed file or other narrow inspected source.
3. Under which input/state/lifecycle it manifests.
4. What user/system impact follows.
5. What smallest responsible correction would close it.

Write the title as the defect, not a category label: “Concurrent retries can create duplicate charges,” not “Concurrency issue.”

Lead with impact and evidence. Keep background only when it is needed to understand why the code is wrong.

## Evidence And Location

Cite the narrowest changed location that demonstrates the defect. Include related consumer/test/config locations only when they establish the mismatch.

Explain the causal path from code to impact. A linter warning, failed test, matrix signal, or suspicious line is evidence to investigate, not always the root defect.

Do not cite an entire module, broad summary object, or old snapshot when a current file/function/check is available.

When the defect is omission, anchor to the changed owner where behavior should exist and state the missing branch/contract.

## Severity By Impact

Choose severity from realistic impact and reach:

- Highest: exploitable authorization/security, data loss/corruption, irreversible external effect, crash/unavailable primary path, or impossible required delivery.
- High: accepted behavior missing/wrong, serious integration/concurrency/reliability defect, or evidence absent for a must-level high-risk behavior.
- Low: localized non-blocking maintainability/readability/performance risk with a plausible future or secondary impact.
- Informational: limitation, question, neutral observation, or positive context that does not require code repair.

Do not inflate severity to force prioritization. Do not downgrade a deterministic product defect because a workaround exists.

## Category And Root Cause

Classify by the root defect rather than the visible symptom. A UI “Not Found” caused by a rewritten API path is an integration/API routing defect; a missing test is secondary if source already proves the bug.

Combine symptoms that share one repair. Split findings that have independent causes, owners, or fixes.

Separate:

- missing/wrong implementation,
- accepted behavior not satisfied,
- insufficient evidence,
- environment/review limitation,
- architecture/planning ownership gap,
- current scope mismatch.

## Actionability

Describe the invariant or behavior the repair must establish, not a speculative rewrite. Mention an implementation technique only when it is necessary or the safe correction is unambiguous.

Examples should be short and adapted to the repository. Do not paste generic replacement code that ignores local framework, error, transaction, or test conventions.

A finding should let the repair owner answer: what to change, what not to expand, and what evidence will prove closure.

## Questions And Ambiguity

Ask a question only when the accepted behavior is genuinely unresolved. First inspect requirements, surrounding code, analogous features, types, tests, and configuration.

State the conflicting interpretations and why the choice changes implementation. Do not phrase a known defect as a question to soften it.

## Non-Blocking Notes

Use notes sparingly for actual limitations, useful follow-up, or a pattern worth preserving. Do not require speculative refactors, unmeasured optimization, stylistic preferences, or future scope.

Positive feedback should be specific and brief; it must not bury findings or become a required quota.

## Repair Ownership

Identify the smallest boundary able to correct the root cause: current implementation, missing verification, plan/task ownership, architecture/interface contract, environment capability, or user-owned decision.

Do not send clear code defects to human review, and do not ask implementation repair to invent an unresolved product decision.

## Final Consistency

Before submitting findings, verify:

- Every blocking finding has current evidence and concrete impact.
- Severity matches impact and overall decision.
- Location belongs to the reviewed change or explains a direct changed interaction.
- Duplicate root causes are consolidated.
- Suggested correction does not expand scope unnecessarily.
- Evidence gaps are not mislabeled as confirmed product defects, or vice versa.
- No finding depends only on personal preference.

## Anti-Patterns

- “Needs more tests” without naming unproved behavior and assertion.
- “Code is messy” without a concrete risk.
- Repeating automated output without root-cause analysis.
- Writing a markdown report template instead of the required structured result.
- Bundling unrelated defects into one large item.
- Empty praise, apology, or agreement language that adds no technical information.
- Using human review to avoid assigning a clear repair owner.
