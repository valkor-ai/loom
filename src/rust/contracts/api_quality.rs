use serde_json::{json, Value};

use crate::{
    PlanningGenerationContract, RequirementDetailItem, ScopeItem, TechnicalBaselineContract,
};

const API_SIGNAL_WORDS: &[&str] = &[
    "api",
    "接口",
    "endpoint",
    "http",
    "rest",
    "controller",
    "route",
    "handler",
    "后端接口",
    "服务接口",
    "调用接口",
];

const BACKEND_SIGNAL_WORDS: &[&str] = &[
    "backend",
    "server",
    "api",
    "service",
    "controller",
    "spring",
    "fastapi",
    "express",
    "nestjs",
    "django",
    "rails",
    "laravel",
    "asp.net",
    "dotnet",
    "后端",
    "服务端",
];

const COLLECTION_SIGNAL_WORDS: &[&str] = &[
    "list",
    "search",
    "filter",
    "sort",
    "page",
    "pagination",
    "列表",
    "查询",
    "检索",
    "筛选",
    "排序",
    "分页",
];

const CONTRACT_SIGNAL_WORDS: &[&str] = &[
    "openapi",
    "swagger",
    "api doc",
    "api spec",
    "contract",
    "schema",
    "sdk",
    "codegen",
    "接口文档",
    "接口规范",
    "契约",
];

const EVOLUTION_SIGNAL_WORDS: &[&str] = &[
    "version",
    "v1",
    "v2",
    "deprecated",
    "deprecation",
    "backward compatible",
    "breaking change",
    "兼容",
    "版本",
    "废弃",
    "破坏性",
];

const OPERATIONS_SIGNAL_WORDS: &[&str] = &[
    "idempotency",
    "idempotent",
    "idempotency-key",
    "retry",
    "retry-after",
    "rate limit",
    "rate-limit",
    "throttle",
    "429",
    "503",
    "cache",
    "etag",
    "last-modified",
    "conditional request",
    "if-match",
    "if-none-match",
    "request id",
    "x-request-id",
    "幂等",
    "重复提交",
    "重试",
    "限流",
    "节流",
    "缓存",
    "条件请求",
    "请求id",
    "请求ID",
    "追踪",
];

pub fn build_api_quality_seed(
    planning_contract: &PlanningGenerationContract,
    technical_baseline: &TechnicalBaselineContract,
) -> Value {
    let signals = collect_api_seed_signals(planning_contract, technical_baseline);
    if signals.api_signal_count == 0 {
        return Value::Null;
    }
    let mut api_groups = vec![
        "core".to_string(),
        "resource".to_string(),
        "errors".to_string(),
        "security".to_string(),
    ];
    if signals.collection_signal_count > 0 {
        api_groups.push("pagination".to_string());
    }
    if signals.contract_signal_count > 0 {
        api_groups.push("contract".to_string());
    }
    if signals.evolution_signal_count > 0 {
        api_groups.push("evolution".to_string());
    }
    if signals.operations_signal_count > 0 {
        api_groups.push("operations".to_string());
    }
    let reference_load_plan = api_reference_load_plan(&api_groups);
    json!({
        "required": true,
        "qualityLevel": "production_api_contract",
        "selectionReason": signals.reason,
        "techReferenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": {
                "api": api_groups
            },
            "referenceLoadPlan": reference_load_plan
        },
        "interfaceContract": {
            "appliesTo": "Architecture content.interfaces entries with type=http_api or task-owned HTTP API bindings.",
            "requiredFields": [
                "interfaceId",
                "name",
                "type",
                "resource",
                "operationKind",
                "method",
                "path",
                "requestSchema",
                "responseSchema",
                "statusCodes",
                "errorSchema",
                "scopeRefs",
                "acceptanceRefs"
            ],
            "conditionalFields": [
                "paginationPolicy",
                "filterFields",
                "sortFields",
                "authPolicy",
                "contractFileRefs",
                "compatibilityPolicy",
                "idempotencyPolicy",
                "cachePolicy",
                "conditionalRequestPolicy",
                "rateLimitPolicy",
                "retryPolicy",
                "requestIdPolicy"
            ]
        },
        "generationRules": [
            "Use apiQualitySeed only for current-phase API/interface work; do not add API work for deferred scope.",
            "Represent API contracts in Architecture interfaces and downstream apiContractRequirements; do not paste API reference prose into candidates.",
            "Read only files listed in techReferenceProfile.referenceLoadPlan; selected API groups are semantic evidence labels, not path maps.",
            "Do not add versioned paths or deprecation policy unless techReferenceProfile.referenceLoadPlan selects tech/api/evolution.md.",
            "Do not require OpenAPI files unless techReferenceProfile.referenceLoadPlan selects tech/api/contract.md or the repository already owns one.",
            "Do not add idempotency, cache, rate-limit, retry, or request-id infrastructure unless techReferenceProfile.referenceLoadPlan selects tech/api/operations.md or the repository already owns that convention."
        ]
    })
}

pub fn api_reference_load_plan(api_groups: &[String]) -> Vec<Value> {
    api_groups
        .iter()
        .map(|group| {
            json!({
                "refId": format!("tech.api.{group}"),
                "path": format!("tech/api/{group}.md"),
                "reason": format!("Selected API {group} quality reference for current-phase interface design.")
            })
        })
        .collect()
}

pub fn api_quality_enum_refs() -> Value {
    json!({
        "knownReferenceGroups": {
            "api": ["core", "resource", "errors", "pagination", "contract", "security", "evolution", "operations"]
        },
        "interfaceType": ["http_api", "service_method", "external_adapter", "event", "job", "cli_command"],
        "httpMethod": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"],
        "operationKind": ["create", "read_list", "read_detail", "replace", "update", "delete", "state_transition", "domain_action", "search", "export"],
        "paginationStrategy": ["not_applicable", "page_size", "offset_limit", "cursor", "keyset"],
        "authRequirement": ["not_applicable", "required", "optional", "deferred_with_risk"],
        "statusCodeCategory": ["success", "validation", "business_conflict", "not_found", "auth", "rate_limit", "service_unavailable", "server_error"],
        "contractArtifact": ["aac_interface", "openapi", "schema_file", "source_code", "test"]
    })
}

#[derive(Default)]
struct ApiSeedSignals {
    api_signal_count: usize,
    collection_signal_count: usize,
    contract_signal_count: usize,
    evolution_signal_count: usize,
    operations_signal_count: usize,
    reason: String,
}

fn collect_api_seed_signals(
    planning_contract: &PlanningGenerationContract,
    technical_baseline: &TechnicalBaselineContract,
) -> ApiSeedSignals {
    let mut signals = ApiSeedSignals::default();
    let stack_supports_backend_api =
        stack_contains_any(&technical_baseline.stack, BACKEND_SIGNAL_WORDS);
    for text in planning_texts(planning_contract) {
        if contains_any(&text, API_SIGNAL_WORDS) || contains_any(&text, BACKEND_SIGNAL_WORDS) {
            signals.api_signal_count += 1;
        }
        if contains_any(&text, COLLECTION_SIGNAL_WORDS) {
            signals.collection_signal_count += 1;
        }
        if contains_any(&text, CONTRACT_SIGNAL_WORDS) {
            signals.contract_signal_count += 1;
        }
        if contains_any(&text, EVOLUTION_SIGNAL_WORDS) {
            signals.evolution_signal_count += 1;
        }
        if contains_any(&text, OPERATIONS_SIGNAL_WORDS) {
            signals.operations_signal_count += 1;
        }
    }
    signals.reason = if signals.api_signal_count > 0 {
        if stack_supports_backend_api {
            "Current phase has explicit backend/API/interface signals and the confirmed technical baseline includes a backend-capable stack.".to_string()
        } else {
            "Current phase has explicit backend/API/interface signals in current scope, requirement details, or frontend text.".to_string()
        }
    } else {
        "No current-phase API or backend interface signal detected.".to_string()
    };
    signals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConfidenceLevel, FrontendExperience, FrontendExperienceLevel, FrontendOperationPath,
        FrontendTargetSelectionMode, PlanningContractContextRefs, PlanningContractPhaseScope,
        PlanningContractSource, PlanningContractStatus, PlanningContractTechnicalBaseline,
        PlanningDeploymentRules, PlanningGenerationContract, PlanningHandoff, PlanningInputs,
        PlanningRules, ProjectKind, QualityGates, RequirementDetailsIndex, ScopeIsolationRules,
        ScopeItem, ScopeSource, TechnicalBaselineApproval, TechnicalBaselineApprovalType,
        TechnicalBaselineContract, TechnicalBaselineScope, TechnicalBaselineSource,
    };

    fn baseline(stack: Value) -> TechnicalBaselineContract {
        TechnicalBaselineContract {
            schema_version: "1.0".to_string(),
            technical_baseline_id: "tb_1".to_string(),
            delivery_id: "delivery_1".to_string(),
            phase_id: "phase_1".to_string(),
            status: crate::TechnicalBaselineStatus::Confirmed,
            source: TechnicalBaselineSource::UserConfirmed,
            project_kind: ProjectKind::Greenfield,
            scope: TechnicalBaselineScope::Project,
            stack,
            constraints: vec![],
            evidence: vec![],
            approval: TechnicalBaselineApproval {
                r#type: TechnicalBaselineApprovalType::UserConfirmed,
                confirmed_at: None,
                reason: None,
            },
            confidence: ConfidenceLevel::High,
            requires_user_confirmation: None,
            reasoning_summary: vec![],
            alternatives: vec![],
            created_at: "2026-07-03T00:00:00Z".to_string(),
            updated_at: "2026-07-03T00:00:00Z".to_string(),
        }
    }

    fn planning_contract(
        business_goal: &str,
        frontend_experience: Option<FrontendExperience>,
        detail_summary: &str,
    ) -> PlanningGenerationContract {
        PlanningGenerationContract {
            schema_version: "1.0".to_string(),
            planning_contract_id: "pgc_1".to_string(),
            delivery_id: "delivery_1".to_string(),
            phase_id: "phase_1".to_string(),
            status: PlanningContractStatus::Ready,
            source: PlanningContractSource {
                brainstorm_run_id: "brainstorm_1".to_string(),
                brainstorm_contract_id: "brainstorm_contract_1".to_string(),
                roadmap_id: None,
                phase_id: "phase_1".to_string(),
                technical_baseline_id: "tb_1".to_string(),
            },
            phase_scope: PlanningContractPhaseScope {
                phase_name: "Current phase".to_string(),
                phase_goal: business_goal.to_string(),
                included: vec![],
                deferred: vec![],
                excluded: vec![],
                acceptance_candidates: vec![],
            },
            context_refs: PlanningContractContextRefs {
                brainstorm_contract_ref: "brainstorm.json".to_string(),
                repository_context_ref: None,
                delivery_concept_glossary_ref: None,
                phase_concept_grounding_ref: None,
                confirmed_frontend_experience_ref: None,
                current_frontend_experience_ref: None,
            },
            technical_baseline: PlanningContractTechnicalBaseline {
                technical_baseline_id: "tb_1".to_string(),
                status: crate::TechnicalBaselineStatus::Confirmed,
                scope: TechnicalBaselineScope::Project,
                summary: json!({}),
                must_follow: true,
            },
            planning_inputs: PlanningInputs {
                business_goal: business_goal.to_string(),
                actors: vec![],
                capability_groups: vec![],
                business_flows: vec![],
                frontend_experience,
                user_facing_language: None,
                source_refs: vec![],
                context_notes: vec![],
            },
            requirement_details: RequirementDetailsIndex {
                schema_version: "1.0".to_string(),
                authority: "test".to_string(),
                source_brainstorm_contract_ref: "brainstorm.json".to_string(),
                items: if detail_summary.is_empty() {
                    vec![]
                } else {
                    vec![crate::RequirementDetailItem {
                        detail_id: "detail_1".to_string(),
                        kind: "business".to_string(),
                        title: "Detail".to_string(),
                        summary: detail_summary.to_string(),
                        required_for_current_phase: true,
                        priority: "must".to_string(),
                        source_field_refs: vec![],
                        source_refs: vec![],
                        scope_refs: vec![],
                        acceptance_refs: vec![],
                        concept_refs: vec![],
                        frontend_refs: vec![],
                        impact_tags: vec![],
                        lifecycle_stage: "current".to_string(),
                        quality: "confirmed".to_string(),
                        unresolved_note: None,
                    }]
                },
                extraction_warnings: vec![],
            },
            planning_rules: PlanningRules {
                scope_isolation: ScopeIsolationRules {
                    only_plan_current_phase: true,
                    forbid_deferred_scope_implementation: true,
                    forbid_future_phase_implementation: true,
                },
                output_requirements: crate::OutputRequirements {
                    must_create_architecture_artifact_contract: true,
                    must_create_task_plan: true,
                    task_plan_must_reference_acceptance: true,
                },
                deployment: PlanningDeploymentRules {
                    default_enabled: false,
                    requires_explicit_user_request: true,
                },
            },
            quality_gates: QualityGates {
                requires_architecture_before_task_plan: true,
                requires_acceptance_coverage: true,
                requires_verification_evidence: true,
            },
            handoff: PlanningHandoff {
                ready_for_architecture: true,
                ready_for_task_plan: true,
                blocking_reasons: vec![],
                next_node: "architecture".to_string(),
            },
            created_at: "2026-07-03T00:00:00Z".to_string(),
            updated_at: "2026-07-03T00:00:00Z".to_string(),
        }
    }

    fn ui_only_frontend_experience() -> FrontendExperience {
        FrontendExperience {
            required: true,
            kind: "web".to_string(),
            experience_level: FrontendExperienceLevel::UsableInternalProduct,
            audiences: vec![],
            surfaces: vec![],
            data_views: vec![],
            actions: vec![],
            operation_paths: vec![FrontendOperationPath {
                path_id: "path_1".to_string(),
                name: "Review local dashboard".to_string(),
                user_goal: "Inspect local status widgets".to_string(),
                surface_ref: Some("surface_1".to_string()),
                workflow_ref: Some("flow_1".to_string()),
                target_object: Some("dashboard".to_string()),
                selection_mode: FrontendTargetSelectionMode::NotApplicable,
                selection_summary: "Open the dashboard from navigation.".to_string(),
                data_view_refs: vec!["view_1".to_string()],
                action_refs: vec!["action_1".to_string()],
                required_states: vec![],
                source_refs: vec![],
            }],
            must_not: vec![],
            confirmation_summary: None,
        }
    }

    fn scope_item(label: &str, summary: &str) -> ScopeItem {
        ScopeItem {
            id: format!("scope_{}", label.replace(' ', "_")),
            label: label.to_string(),
            items: vec![summary.to_string()],
            reason: None,
            source: ScopeSource::UserConfirmed,
        }
    }

    #[test]
    fn api_quality_seed_does_not_treat_ui_operation_path_as_api_signal() {
        let seed = build_api_quality_seed(
            &planning_contract(
                "Create staff dashboard page",
                Some(ui_only_frontend_experience()),
                "",
            ),
            &baseline(json!({ "frontend": "React" })),
        );
        assert!(
            seed.is_null(),
            "UI-only operation paths must not trigger API references: {seed:#}"
        );
    }

    #[test]
    fn api_quality_seed_does_not_treat_backend_stack_alone_as_api_signal() {
        let seed = build_api_quality_seed(
            &planning_contract(
                "Create staff dashboard page",
                Some(ui_only_frontend_experience()),
                "",
            ),
            &baseline(json!({
                "frontend": "React",
                "backend": "Spring Boot",
                "database": "PostgreSQL"
            })),
        );
        assert!(
            seed.is_null(),
            "Backend-capable baseline must not trigger API references for a UI-only current phase: {seed:#}"
        );
    }

    #[test]
    fn api_quality_seed_ignores_deferred_api_scope() {
        let mut planning = planning_contract("Create local admin dashboard", None, "");
        planning.phase_scope.deferred = vec![scope_item(
            "Future API",
            "Later phase will implement purchase request REST API endpoints.",
        )];
        let seed = build_api_quality_seed(
            &planning,
            &baseline(json!({
                "frontend": "React",
                "backend": "Spring Boot"
            })),
        );
        assert!(
            seed.is_null(),
            "Deferred API scope must not trigger current-phase API references: {seed:#}"
        );
    }

    #[test]
    fn api_quality_seed_selects_api_references_from_explicit_api_text() {
        let seed = build_api_quality_seed(
            &planning_contract(
                "Implement purchase request API",
                None,
                "Provide list query endpoint with pagination.",
            ),
            &baseline(json!({ "frontend": "React" })),
        );
        assert_eq!(seed["required"], json!(true));
        assert_eq!(
            seed["techReferenceProfile"]["groups"]["api"],
            json!(["core", "resource", "errors", "security", "pagination"])
        );
    }

    #[test]
    fn api_quality_seed_selects_operations_reference_from_explicit_operational_text() {
        let seed = build_api_quality_seed(
            &planning_contract(
                "Implement purchase request API",
                None,
                "Submission endpoint must prevent duplicate submit with idempotency key and return retry-after on rate limit.",
            ),
            &baseline(json!({ "backend": "Spring Boot" })),
        );
        assert_eq!(seed["required"], json!(true));
        assert_eq!(
            seed["techReferenceProfile"]["groups"]["api"],
            json!(["core", "resource", "errors", "security", "operations"])
        );
        assert!(seed["interfaceContract"]["conditionalFields"]
            .as_array()
            .expect("conditional fields")
            .iter()
            .any(|field| field.as_str() == Some("idempotencyPolicy")));
    }
}

fn planning_texts(planning_contract: &PlanningGenerationContract) -> Vec<String> {
    let mut texts = vec![
        planning_contract.phase_scope.phase_name.clone(),
        planning_contract.phase_scope.phase_goal.clone(),
        planning_contract.planning_inputs.business_goal.clone(),
    ];
    for item in planning_contract.phase_scope.included.iter() {
        collect_scope_item_texts(item, &mut texts);
    }
    for acceptance in &planning_contract.phase_scope.acceptance_candidates {
        texts.push(acceptance.statement.clone());
    }
    for detail in planning_contract
        .requirement_details
        .items
        .iter()
        .filter(|detail| detail.required_for_current_phase)
    {
        collect_detail_texts(detail, &mut texts);
    }
    if let Some(frontend) = &planning_contract.planning_inputs.frontend_experience {
        for path in &frontend.operation_paths {
            texts.push(path.name.clone());
            texts.push(path.user_goal.clone());
            if let Some(target) = &path.target_object {
                texts.push(target.clone());
            }
        }
    }
    texts
}

fn collect_scope_item_texts(item: &ScopeItem, texts: &mut Vec<String>) {
    texts.push(item.label.clone());
    texts.extend(item.items.clone());
    if let Some(reason) = &item.reason {
        texts.push(reason.clone());
    }
}

fn collect_detail_texts(detail: &RequirementDetailItem, texts: &mut Vec<String>) {
    texts.push(detail.kind.clone());
    texts.push(detail.title.clone());
    texts.push(detail.summary.clone());
    texts.extend(detail.impact_tags.clone());
}

fn stack_contains_any(value: &Value, needles: &[&str]) -> bool {
    match value {
        Value::String(text) => contains_any(text, needles),
        Value::Array(items) => items.iter().any(|item| stack_contains_any(item, needles)),
        Value::Object(object) => object
            .iter()
            .any(|(key, value)| contains_any(key, needles) || stack_contains_any(value, needles)),
        _ => false,
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    let lower = text.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}
