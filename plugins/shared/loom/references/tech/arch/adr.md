# ADR Guidance For Loom Architecture

Use this reference when writing architecture decision records.

Loom ADRs are structured architecture decision entries, not Markdown files unless the user's project explicitly asks for ADR documents.

## Decision Quality

Each decision must make four things unambiguous:

- **forces**: current requirements, constraints, and existing boundaries that make a decision necessary
- **choice**: the structural rule implementation will follow
- **ownership**: modules and interfaces that can satisfy or violate the choice
- **evidence**: observable implementation or verification signals that prove the choice was respected

## Decision Categories

| Category | Use When |
|---|---|
| architecture_style | Selecting single app, modular monolith, service split, event-driven, CQRS-style separation, or similar structure. |
| module_boundary | Defining ownership and responsibility between modules. |
| data_boundary | Defining entity ownership, transaction boundary, migration impact, or read/write model. |
| integration_boundary | Defining external adapter, async dependency, or interface boundary. |
| runtime_boundary | Defining build/start/probe/environment/runtime surface constraints. |
| security_boundary | Defining authorization, sensitive data, audit, or exposure boundaries. |
| operability | Defining observability, recovery, or operational simplicity trade-offs. |

## Alternatives

Alternatives are mandatory because they prove the decision is a trade-off, not a guess.

For each alternative:

- name the alternative
- state the trade-off
- explain why it was rejected for the current phase

Do not list strawman alternatives. Compare realistic options.

Every alternative needs a name, the real trade-off it would create, and a rejection reason tied to current-phase forces. Product names or patterns copied from an example are not alternatives unless they could actually satisfy the same requirement.

## Consequences

Positive consequences should describe implementation or verification benefits.

Negative consequences should describe real cost, limitation, or future risk.

Neutral consequences can capture follow-up awareness without creating current tasks.

At least one positive and one negative consequence are required for a meaningful trade-off. Empty consequence headings or paraphrases of the decision do not establish architectural cost.

## Verification Hints

Verification hints should identify evidence that proves the decision was respected:

- changed module files align with module ownership
- API paths use declared interface boundary
- persistence changes preserve transaction/invariant rules
- runtime scripts/probes match runtime delivery contract
- tests or static checks cover the stated risk

## Anti-Patterns

- ADRs with only "we choose X because it is simple."
- Alternatives that are impossible or unrelated.
- Consequences that repeat the decision.
- Decisions that implement future phase scope.
- Decisions without source refs to scope, acceptance, or requirement details.

## Current Phase Rule

An ADR belongs in the current phase only when it affects current implementation or verification. General future architecture notes should stay outside the current-phase design.
