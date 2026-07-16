# UI Reference Foundry Comparison

This is a maintainer-only migration and coverage record. It is not part of the
Loom skill references, is not included in any agent `referenceLoadPlan`, and
must never be requested by an agent during delivery.

## File Coverage Matrix

| Loom area | Loom-owned responsibility | Foundry source area | Status |
| --- | --- | --- | --- |
| `uix/core.md` | UI production baseline and task-brief bridge | `SKILL.md`, command workflow | Batch 6 complete; retained as Loom-specific |
| `uix/system.md` | semantic tokens, shared primitives, token consumption | `tokens`, `unify` | Batch 1 complete |
| `uix/surface-decision.md` | known/hybrid/custom surface implementation | `scenario`, `forge` | Batch 6 complete |
| `uix/verification.md` | rendered, state, visual, interaction evidence | `audit`, `verify-output` | Batch 6 complete |
| `uix/web-implementation.md` | browser semantics, layout resilience, hydration | stack and audit references | Batch 2 complete |
| `uix/anti-patterns.md` | product-boundary and AI-slop constraints | `anti-patterns` | Batch 6 complete; retained and audited |
| `uix/content.md` | product copy and forbidden content boundary | scenario references | Batch 2 complete |
| `uix/data.md` | business data surfaces and readback | admin/fintech scenarios | Batch 2 complete |
| `uix/interaction.md` | actions, forms, feedback, continuity | scenario and audit references | Batch 2 complete |
| `uix/mobile.md` | responsive and touch behavior | mobile scenarios and layout tokens | Batch 2 complete |
| `uix/frameworks.md` | UIX/framework boundary | stack references | Batch 4 complete |
| `uix/tokens/*` | semantic visual primitives and layout decisions | `references/tokens/*` | Batch 1 complete |
| `uix/scenarios/*` | product-surface anatomy, workflow continuity, risk states, and responsive behavior | `references/scenarios/*` | Batch 3 complete |
| `uix/stacks/*` | UIX implementation structure per stack, state ownership, and platform rendering boundary | `references/stacks/*` | Batch 4 complete |
| `uix/templates/*` | adaptable CSS/Tailwind token starting points | `templates/*` | Batch 1 complete |

## Capability Disposition Matrix

| Foundry capability | Loom destination | Disposition and boundary | Runtime consumer |
| --- | --- | --- | --- |
| Scenario baseline | `uix/scenarios`, `surface-decision.md` | Translated into product-surface rules and a structured surface decision. Foundry scenario command output is not copied. | UI quality seed and surface contract |
| Stack guidance | `uix/stacks`, `tech/frontend`, `tech/code` | Split by ownership: rendered composition in UIX, framework/runtime in `tech/frontend`, language rules in `tech/code`. | Reference routing |
| Token system | `uix/tokens`, `uix/templates`, `designTokenAssetPlan` | Translated into one semantic token authority with existing-style evidence, merge policy, and explicit no-parallel-system policy. | Architecture, Execution, TaskResult, Review |
| Anti-patterns | `uix/anti-patterns.md` | Absorbed as product-boundary and anti-demo rules. Foundry wording and command names are excluded. | UI quality rules and Review |
| Render/audit checks | `uix/verification.md`, Review | Translated into product-surface, state/interaction, visual-system, and rendered-runtime evidence dimensions. | Review matrix |
| `detect-stack` | Technical Baseline | Reused only as structured stack signals. UIX does not implement a second stack detector. | MCP seed generation |
| `extract-tokens` | `designTokenAssetPlan.existingStyleEvidence` | Translated as repository evidence fields and token convergence decisions. No external extraction command is run by the Agent workflow. | Architecture candidate and Execution token plan |
| `match-profile` | `surfaceDecisionCandidate.patternRankings`, `uiSurfaceDecisionContract.patternDecision` | Implemented as ranked known/hybrid/custom surface reasoning. Custom mode requires complete semantic and layout facts. | Architecture submit normalization |
| `unify` | `system.md`, token references, `designTokenAssetPlan.mergePolicy` | Implemented as reuse/extend rules and a single token authority. No parallel Foundry token registry is introduced. | Execution and Review |
| `optimize` | `uix/verification.md`, UI quality rules, Review | Translated into repeatable inspection and evidence expectations. It is not a separate post-hoc command that can replace production guidance. | Execution evidence and Review |
| `brand` | None | Explicitly omitted from current runtime. Loom has no accepted brand-profile input, so no default brand palette or brand library is injected into unrelated products. | Maintainer-only future consideration |
| Foundry command files | None | Explicitly omitted. Commands, installation, CLI workflow, and external paths are not Agent-facing UIX knowledge. | None |

## Batch Progress

| Batch | Scope | Status | Required evidence |
| --- | --- | --- | --- |
| 1 | tokens, templates, system | complete | 13 UI quality tests, template resolution, UIX duplication test |
| 2 | cross-cutting UIX guidance | complete | scoped UI quality seed and reference-routing tests |
| 3 | business and presentation scenarios | complete | scenario completeness and positive/negative scenario routing |
| 4 | UIX stacks and frontend boundary | complete | stack routing and duplicate-load tests |
| 5 | Foundry workflow capability translation | complete | disposition coverage and ownership tests |
| 6 | MCP integration and global audit | complete | load-plan, evidence, duplication, and full UIX integration tests |

## Rules For Updates

- Add external source mapping only here, never inside agent-facing UIX files.
- Mark a capability as omitted only with a technical reason and an owner for any future work.
- Treat reference prose, MCP request instructions, and derived contract fields as separate sources of authority.
- Remove superseded or duplicated guidance in the same change that adds its replacement.

## Translation Acceptance Criteria

- A translated capability must have one runtime owner and one maintainer record.
- A capability is not considered migrated because a similarly named paragraph was copied; it must change a generated decision, an implementation choice, or an evidence requirement.
- An omitted capability must name the missing Loom input or responsibility. It must not be represented by a relaxed default.
- UIX references must remain consumable without this document, the external Foundry tree, or Foundry command knowledge.
