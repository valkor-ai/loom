# UI Reference Foundry Comparison

This is a maintainer-only migration and coverage record. It is not part of the
Loom skill references, is not included in any agent `referenceLoadPlan`, and
must never be requested by an agent during delivery.

## File Coverage Matrix

| Loom area | Loom-owned responsibility | Foundry source area | Status |
| --- | --- | --- | --- |
| `uix/core.md` | UI production baseline and `uiProductionBrief` bridge | `SKILL.md`, command workflow | retained as Loom-specific |
| `uix/system.md` | semantic tokens, shared primitives, token consumption | `tokens`, `unify` | Batch 1 complete |
| `uix/surface-decision.md` | known/hybrid/custom surface implementation | `scenario`, `forge` | Batch 6 audit |
| `uix/verification.md` | rendered, state, visual, interaction evidence | `audit`, `verify-output` | Batch 2 complete; Batch 6 audit |
| `uix/web-implementation.md` | browser semantics, layout resilience, hydration | stack and audit references | Batch 2 complete |
| `uix/anti-patterns.md` | product-boundary and AI-slop constraints | `anti-patterns` | retained and audited |
| `uix/content.md` | product copy and forbidden content boundary | scenario references | Batch 2 complete |
| `uix/data.md` | business data surfaces and readback | admin/fintech scenarios | Batch 2 complete |
| `uix/interaction.md` | actions, forms, feedback, continuity | scenario and audit references | Batch 2 complete |
| `uix/mobile.md` | responsive and touch behavior | mobile scenarios and layout tokens | Batch 2 complete |
| `uix/frameworks.md` | UIX/framework boundary | stack references | Batch 4 |
| `uix/tokens/*` | semantic visual primitives and layout decisions | `references/tokens/*` | Batch 1 complete |
| `uix/scenarios/*` | product-surface anatomy, workflow continuity, risk states, and responsive behavior | `references/scenarios/*` | Batch 3 complete |
| `uix/stacks/*` | UIX implementation structure per stack | `references/stacks/*` | Batch 4 |
| `uix/templates/*` | adaptable CSS/Tailwind token starting points | `templates/*` | Batch 1 complete |

## Capability Disposition Matrix

| Foundry capability | Loom destination | Disposition | Runtime consumer |
| --- | --- | --- | --- |
| Scenario baseline | `uix/scenarios`, `surface-decision.md` | translate decision rules, do not copy CLI output | UI quality seed and surface contract |
| Stack guidance | `uix/stacks`, `tech/frontend`, `tech/code` | split by UIX, framework, and language ownership | reference routing |
| Token system | `uix/tokens`, `uix/templates`, `designTokenAssetPlan` | adapt into one token authority | Execution and TaskResult |
| Anti-patterns | `uix/anti-patterns.md` | absorb and preserve product boundary rules | UI quality rules and Review |
| Render/audit checks | `uix/verification.md`, Review | translate evidence requirements | Review matrix |
| `detect-stack` | Technical Baseline | already represented by structured stack signals | MCP seed generation |
| `extract-tokens` | style evidence and token plan | capability gap to evaluate | Architecture/Execution |
| `match-profile` | surface/scenario decision | capability gap to evaluate | Architecture/Execution |
| `unify` | system and token convergence rules | capability gap to evaluate | Execution/Review |
| `optimize` | UI optimization workflow | capability gap to evaluate | future MCP responsibility |
| `brand` | no direct runtime equivalent | do not import the full brand library; design a separate decision model if needed | future UI planning |
| Foundry command files | no direct runtime copy | translate underlying decisions only | MCP and references |

## Batch Progress

| Batch | Scope | Status | Required evidence |
| --- | --- | --- | --- |
| 1 | tokens, templates, system | complete | 13 UI quality tests, template resolution, UIX duplication test |
| 2 | cross-cutting UIX guidance | complete | scoped UI quality seed and reference-routing tests |
| 3 | business and presentation scenarios | complete | scenario completeness and positive/negative scenario routing |
| 4 | UIX stacks and frontend boundary | pending | stack routing and duplicate-load tests |
| 5 | Foundry workflow capability translation | pending | disposition coverage and ownership tests |
| 6 | MCP integration and global audit | pending | load-plan, evidence, duplication, and full UIX integration tests |

## Rules For Updates

- Add external source mapping only here, never inside agent-facing UIX files.
- Mark a capability as omitted only with a technical reason and an owner for any future work.
- Treat reference prose, MCP request instructions, and derived contract fields as separate sources of authority.
- Remove superseded or duplicated guidance in the same change that adds its replacement.
