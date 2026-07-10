# UI Surface Decision Model Refactor

Maintainer-only implementation note. This file is not part of the runtime Loom reference set and must not be loaded by agents during normal delivery. Use it to keep the UI surface decision model refactor aligned while the Rust contracts, request generation, execution projection, result validation, review, and UIX references are migrated.

## Goal

Replace the current split UI quality chain with one authoritative UI surface decision contract:

```text
Architecture surface decision candidate
-> MCP-normalized uiSurfaceDecisionContract
-> TaskPlan task-scoped ownership
-> Execution task-scoped view
-> TaskResult evidence
-> Review checks
```

The refactor must not add a parallel UI contract beside the current one. Existing UI logic may remain only when it consumes the single decision contract directly and does not preserve old UI quality fields as a second authority.

## Current Ownership Map

| Current field or function | Current owner | Current role | Target owner | Refactor action |
| --- | --- | --- | --- | --- |
| `uiQualitySeed` | MCP request generation | Gives scenario candidates, reference plan, gates, token plan, and selection rules to Architecture | MCP request generation | Keep as an input seed, but stop treating scenario candidates as the final semantic decision. |
| `infer_primary_scenario()` | MCP Rust keyword inference | Guesses UI scenario from text and stack signals | None as authority | Remove as an authoritative classifier. Keep only as non-binding candidate hint if still useful. |
| `uiQualityContract.scenario/layoutBaseline/density` | Architecture candidate plus MCP normalization | Stores coarse UI scenario and visual baseline | `uiSurfaceDecisionContract.patternDecision/layoutModel` | Replace with structured pattern, semantic facts, and layout anatomy. |
| `uiQualityContract.referenceProfile/referenceLoadPlan` | MCP normalization | Selects UIX reference files | `uiSurfaceDecisionContract.referencePlan` | Keep MCP-owned derivation, but derive from decision contract instead of coarse scenario. |
| `uiQualityContract.qualityGates` | MCP normalization | Stores generated UI quality gates | `uiSurfaceDecisionContract.qualityRules` | Replace with rule objects derived from pattern, regions, actions, states, tokens, and content boundary. |
| `uiQualityContract.businessUiRules` | Agent candidate | Broad business UI assertions | None | Delete or fold into `qualityRules`; do not keep separate self-assertion rules. |
| `uiSurfaceRegistry.surfaces` | Agent architecture section | Stores surface purpose, composition, information, action, state, visual, responsive models | `uiSurfaceDecisionContract.regions/actions/states` | Reuse the useful structure, but make it the normalized decision contract rather than a parallel registry. |
| `frontendExperienceRequirement.uiQualityContract` | TaskPlan | Copies the full AAC UI quality object into frontend tasks | Removed from new artifacts | Do not copy or persist it. TaskPlan carries `uiSurfaceDecisionContractRef` plus region/action/state/rule ownership only. |
| `frontendExperienceRequirement.uiTaskQualityGates` | TaskPlan | Stores task-scoped gate copies | Removed from new artifacts | Do not generate or persist it. Task-scoped rule ownership is expressed through `uiSurfaceOwnership.qualityRuleIdsInScope`. |
| `uiProductionBrief` | Execution request | Repackages registry and quality contract into task guidance | Execution task-scoped view | Generate it directly from `uiSurfaceDecisionContract`; do not keep an old quality-contract view beside it. |
| `styleAssetPlan` | Execution request | Repackages design token plan and reference plan | Execution task-scoped view | Keep one MCP-owned reference plan and one token asset plan; execution may pass task-relevant paths forward but must not create a second authority. |
| `frontendQualitySelfCheck.referenceGroupsChecked/referenceFilesChecked` | TaskResult | Evidence that agent read references | TaskResult evidence | Downgrade to read evidence. It must not prove UI quality satisfaction. |
| `frontendQualitySelfCheck.businessUiRulesChecked` | TaskResult | Agent self-certifies broad business UI rules | Removed from new results | Reject it on new TaskResult payloads; quality proof must use surface quality rule evidence. |
| `frontendQualitySelfCheck.surfacesCovered` | TaskResult | Surface evidence by `surfaceId` | Surface region/action/state evidence | Reject it on new TaskResult payloads. Primary proof is `surfaceRegionEvidence`, `surfaceActionEvidence`, `surfaceStateEvidence`, and `surfaceQualityRuleEvidence`. |
| `frontendQualitySelfCheck.gateResults` | TaskResult | Agent reports generated gate results | Removed from new results | Reject it on new TaskResult payloads. Review consumes `surfaceQualityRuleEvidence` against `uiSurfaceDecisionContract.qualityRules`. |
| `frontend_quality_gate_review()` | Review | Summarizes self-check gate statuses | Review | Rework to compare decision contract against implementation evidence and rendered/source facts, not just self-check claims. |

## Replacement Contract Shape

The authoritative contract should be introduced as `uiSurfaceDecisionContract` under `frontendExperience`.

| Contract area | Responsibility |
| --- | --- |
| `patternDecision` | Records known, hybrid, or custom pattern selection, ranking evidence, confidence, and mismatch reasoning. |
| `semanticFacts` | Captures user jobs, information shapes, operation models, risk factors, navigation model, device posture, and product mode. Supports known values plus structured `other` extensions. |
| `layoutModel` | Defines desktop/tablet/mobile layout anatomy, primary work region, allowed presentation, forbidden presentation, density, and responsiveness. |
| `regionModel` | Defines UI regions with region ids, roles, purposes, placement, required contents, forbidden contents, and evidence refs. |
| `informationModel` | Defines primary objects, fields, scan order, identity/status handling, comparison/detail needs, and long-content behavior. |
| `actionModel` | Defines primary/contextual/risk actions, placement, pending/success/error behavior, and post-success updates. |
| `stateModel` | Defines loading, empty, validation, error, success, disabled, and business-blocking placement by region/action. |
| `compositionConstraints` | Holds structural rules such as no marketing hero, no feature explainer wall, no decorative filler before workflow, and pattern-specific presentation constraints. |
| `contentBoundary` | Holds allowed user-visible copy classes and forbidden internal/process/technical copy classes. |
| `referencePlan` | MCP-owned UIX files selected from pattern, stack, token plan, and custom needs. |
| `qualityRules` | MCP-owned executable rules derived from the contract and consumed by TaskResult and Review. |

## Known, Hybrid, And Custom Policy

| Mode | Acceptance requirement | Quality requirement |
| --- | --- | --- |
| `known` | Agent submits ranked pattern evidence; selected pattern satisfies required signals with valid refs. | MCP derives the known pattern blueprint, reference plan, and quality rules. |
| `hybrid` | Agent identifies primary and secondary patterns with boundary evidence; primary pattern remains responsible for shell and main workflow. | MCP derives primary rules and scoped secondary rules without duplicating unrelated rules. |
| `custom` | Agent proves known patterns mismatch with evidence refs and supplies complete semantic facts, layout anatomy, regions, actions, states, responsiveness, and content boundary. | Custom is stricter, not looser: it must satisfy universal workflow, information, action, state, responsive, composition, token, and content-boundary rules. |

`custom` must never mean "unknown, therefore relaxed." It means "no known blueprint applies, therefore the agent must provide a complete contract from primitive UI obligations."

## Duplicate Logic To Remove

| Duplicate area | Remove or collapse |
| --- | --- |
| Scenario, layout, and density in `uiQualityContract`, `uiSurfaceRegistry.visualModel`, and `uiProductionBrief.layoutContract` | Collapse into `uiSurfaceDecisionContract.layoutModel`; execution may pass a task-scoped view forward but must not reinterpret it. |
| State fields in `requiredUiStates`, `stateRefs`, `statePlacementModel`, `uiProductionBrief.stateContract`, and `statesCovered` | Collapse into `uiSurfaceDecisionContract.stateModel`; TaskResult records state evidence only. |
| Composition fields in `requiredComposition`, `forbiddenComposition`, `compositionModel.*`, and `visualModel.antiDemoRules` | Collapse into `compositionConstraints`; keep content terms separate in `contentBoundary`. |
| Reference plan in `uiQualityContract.referenceProfile`, `styleAssetPlan`, and TaskResult checked-reference fields | Keep one MCP-owned reference plan; TaskResult may record read evidence but not use it as quality proof. |
| `qualityGates`, `uiTaskQualityGates`, `businessUiRules`, and review matrix expectations | Collapse authority into `qualityRules` derived from the decision contract and task scope; do not retain old-field shadows in new requests, artifacts, results, or review packets. |
| `frontendExperience.surfaces` and `uiSurfaceRegistry.surfaces` as competing downstream inputs | Use existing frontend facts only as input; downstream execution/review consume the normalized decision contract. |

## Migration Batches

1. Field ownership map and deletion plan.
2. `uiSurfaceDecisionContract` schema and enum/open-extension policy.
3. Architecture request candidate shape.
4. Architecture submit normalization and validation.
5. Scenario keyword inference demotion.
6. TaskPlan task-scope ownership.
7. Execution task-scoped view.
8. TaskResult template.
9. TaskResult validation.
10. Review checks.
11. Duplicate logic cleanup.
12. UIX reference repositioning.
13. Unit and contract tests.
14. Multi-surface regression.
15. Final review for redundancy, token size, and repair-loop behavior.

## Review Checklist For Each Batch

- The batch moves one current responsibility toward the single decision contract.
- It does not add a second field that preserves the old authority.
- It removes or demotes at least one stale/duplicated responsibility when possible.
- It keeps reference loading MCP-owned and task-scoped.
- It preserves custom/hybrid expressiveness without weakening quality requirements.
- It includes tests when runtime behavior changes.
