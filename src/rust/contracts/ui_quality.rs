use std::collections::BTreeSet;

use delivery_core::RepairIssue;
use serde_json::{json, Value};

use crate::{FrontendExperience, FrontendExperienceLevel, TechnicalBaselineContract};

pub const UI_SCENARIO_KINDS: [&str; 13] = [
    "admin_dashboard",
    "data_console",
    "fintech_workstation",
    "fintech_consumer_app",
    "consumer_app",
    "mobile_responsive",
    "mobile_native",
    "marketing_site",
    "corporate_site",
    "docs_site",
    "developer_tool",
    "immersive_3d",
    "custom_product_ui",
];

pub const UI_QUALITY_LEVELS: [&str; 4] = [
    "technical_demo",
    "usable_internal_product",
    "production_internal_product",
    "polished_product",
];

pub const UI_SURFACE_POLICIES: [&str; 4] = [
    "business_ui_only",
    "developer_runtime_ui",
    "documentation_ui",
    "marketing_ui",
];

pub const UI_LAYOUT_BASELINES: [&str; 9] = [
    "sidebar_topbar_table_detail",
    "data_console",
    "mobile_task_flow",
    "native_mobile_stack",
    "docs_shell",
    "marketing_narrative",
    "corporate_information",
    "scene_first_3d",
    "custom_product_layout",
];

pub const UI_DENSITIES: [&str; 4] = ["workbench_dense", "balanced", "comfortable", "immersive"];

pub const UI_SEMANTIC_TOKEN_POLICIES: [&str; 1] = ["semantic_tokens_required"];

pub const UI_FORBIDDEN_USER_VISIBLE_CONTENT: [&str; 5] = [
    "runtime_commands",
    "technical_stack_explanation",
    "delivery_progress_notes",
    "verification_instructions",
    "internal_workflow_terms",
];

pub const UI_REQUIRED_STATES: [&str; 5] =
    ["loading", "success", "error", "empty", "business_blocking"];

pub const UI_CORE_REFERENCE_IDS: [&str; 8] = [
    "uix.core",
    "uix.anti-patterns",
    "uix.tokens.color-system",
    "uix.tokens.typography",
    "uix.tokens.spacing",
    "uix.tokens.layout-grid",
    "uix.tokens.motion",
    "uix.tokens.radius-elevation",
];

pub const UI_SCENARIO_REFERENCE_IDS: [&str; 13] = [
    "uix.scenarios.admin-dashboard",
    "uix.scenarios.data-console",
    "uix.scenarios.fintech-workstation",
    "uix.scenarios.fintech-consumer-app",
    "uix.scenarios.consumer-app",
    "uix.scenarios.mobile-responsive",
    "uix.scenarios.mobile-native",
    "uix.scenarios.marketing-site",
    "uix.scenarios.corporate-site",
    "uix.scenarios.docs-site",
    "uix.scenarios.developer-tool",
    "uix.scenarios.immersive-3d",
    "uix.core",
];

pub const UI_STACK_REFERENCE_IDS: [&str; 7] = [
    "uix.stacks.react",
    "uix.stacks.vue",
    "uix.stacks.plain-html",
    "uix.stacks.native-mobile",
    "uix.stacks.threejs",
    "uix.stacks.svelte",
    "uix.stacks.uniapp",
];

pub fn ui_quality_enum_refs() -> Value {
    json!({
        "scenarioKind": UI_SCENARIO_KINDS,
        "qualityLevel": UI_QUALITY_LEVELS,
        "surfacePolicy": UI_SURFACE_POLICIES,
        "layoutBaseline": UI_LAYOUT_BASELINES,
        "density": UI_DENSITIES,
        "semanticTokenPolicy": UI_SEMANTIC_TOKEN_POLICIES,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiState": UI_REQUIRED_STATES,
        "knownReferenceIds": known_ui_reference_ids()
    })
}

pub fn ui_quality_contract_shape() -> Value {
    json!({
        "scenario": {
            "kind": UI_SCENARIO_KINDS.join(" | "),
            "referenceId": "known uix reference id",
            "reason": "string"
        },
        "qualityLevel": UI_QUALITY_LEVELS.join(" | "),
        "surfacePolicy": UI_SURFACE_POLICIES.join(" | "),
        "layoutBaseline": UI_LAYOUT_BASELINES.join(" | "),
        "density": UI_DENSITIES.join(" | "),
        "semanticTokenPolicy": UI_SEMANTIC_TOKEN_POLICIES.join(" | "),
        "referenceProfile": {
            "referenceIds": ["known uix reference id"],
            "loadMode": "skill_reference_by_id"
        },
        "forbiddenUserVisibleContent": [UI_FORBIDDEN_USER_VISIBLE_CONTENT.join(" | ")],
        "requiredUiStates": [{
            "state": UI_REQUIRED_STATES.join(" | "),
            "expectation": "string"
        }],
        "businessUiRules": [{
            "ruleId": "string",
            "expectation": "string"
        }]
    })
}

pub fn build_ui_quality_seed(
    frontend: Option<&FrontendExperience>,
    baseline: Option<&TechnicalBaselineContract>,
) -> Value {
    let required = frontend.map(|item| item.required).unwrap_or(false);
    let primary_scenario = infer_primary_scenario(frontend, baseline);
    let stack_refs = infer_stack_reference_ids(baseline);
    let mut required_reference_ids = UI_CORE_REFERENCE_IDS
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    let scenario_ref = scenario_reference_id(primary_scenario).to_string();
    if !required_reference_ids.contains(&scenario_ref) {
        required_reference_ids.push(scenario_ref);
    }
    for stack_ref in &stack_refs {
        if !required_reference_ids.contains(stack_ref) {
            required_reference_ids.push(stack_ref.clone());
        }
    }
    json!({
        "required": required,
        "scenarioCandidates": scenario_candidates(primary_scenario),
        "qualityLevel": frontend_quality_level(frontend),
        "surfacePolicyCandidates": surface_policy_candidates(primary_scenario),
        "layoutBaselineCandidates": layout_baseline_candidates(primary_scenario),
        "densityCandidates": density_candidates(primary_scenario),
        "semanticTokenPolicy": "semantic_tokens_required",
        "requiredReferenceIds": required_reference_ids,
        "stackReferenceCandidates": stack_refs,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiStates": UI_REQUIRED_STATES,
        "selectionRule": "Pick one scenarioKind from scenarioCandidates. Keep requiredReferenceIds in referenceProfile.referenceIds, add only known ids from enumRefs.uiQuality. Do not expose runtime commands, stack explanations, progress notes, verification instructions, or Loom/internal workflow terms in user-visible UI."
    })
}

pub fn ui_quality_contract_template(ui_quality_seed: &Value) -> Value {
    let scenario = ui_quality_seed
        .pointer("/scenarioCandidates/0/kind")
        .and_then(Value::as_str)
        .unwrap_or("custom_product_ui");
    let scenario_reference = ui_quality_seed
        .pointer("/scenarioCandidates/0/referenceId")
        .and_then(Value::as_str)
        .unwrap_or("uix.core");
    let quality_level = ui_quality_seed
        .get("qualityLevel")
        .and_then(Value::as_str)
        .unwrap_or("production_internal_product");
    let surface_policy = ui_quality_seed
        .pointer("/surfacePolicyCandidates/0")
        .and_then(Value::as_str)
        .unwrap_or("business_ui_only");
    let layout_baseline = ui_quality_seed
        .pointer("/layoutBaselineCandidates/0")
        .and_then(Value::as_str)
        .unwrap_or("custom_product_layout");
    let density = ui_quality_seed
        .pointer("/densityCandidates/0")
        .and_then(Value::as_str)
        .unwrap_or("balanced");
    let semantic_token_policy = ui_quality_seed
        .get("semanticTokenPolicy")
        .and_then(Value::as_str)
        .unwrap_or("semantic_tokens_required");
    let reference_ids = ui_quality_seed
        .get("requiredReferenceIds")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            UI_CORE_REFERENCE_IDS
                .iter()
                .map(|item| Value::String((*item).to_string()))
                .chain(std::iter::once(Value::String(
                    scenario_reference.to_string(),
                )))
                .collect()
        });

    json!({
        "scenario": {
            "kind": scenario,
            "referenceId": scenario_reference,
            "reason": "Selected from uiQualitySeed.scenarioCandidates for the confirmed frontend surfaces and product context."
        },
        "qualityLevel": quality_level,
        "surfacePolicy": surface_policy,
        "layoutBaseline": layout_baseline,
        "density": density,
        "semanticTokenPolicy": semantic_token_policy,
        "referenceProfile": {
            "referenceIds": reference_ids,
            "loadMode": "skill_reference_by_id"
        },
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiStates": [
            {
                "state": "loading",
                "expectation": "Primary data surfaces provide stable loading treatment without layout jumps."
            },
            {
                "state": "success",
                "expectation": "Completed actions give business-language confirmation and refresh affected data."
            },
            {
                "state": "error",
                "expectation": "System errors are visible, recoverable, and do not expose implementation details."
            },
            {
                "state": "empty",
                "expectation": "Empty data views explain the business state and keep next actions available."
            },
            {
                "state": "business_blocking",
                "expectation": "Business-rule blocks are clearly separated from technical failures."
            }
        ],
        "businessUiRules": [
            {
                "ruleId": "business_surface_first",
                "expectation": "The first viewport is the actual product/work surface, not a landing page or process explanation."
            },
            {
                "ruleId": "functional_density_matches_scenario",
                "expectation": "Layout density, controls, and navigation match the selected scenario and expected repeat-use workflow."
            },
            {
                "ruleId": "semantic_tokens_applied",
                "expectation": "Color, type, spacing, radius, and state styling use a coherent semantic token system."
            }
        ]
    })
}

pub fn validate_ui_quality_contract(frontend_experience: &Value) -> Vec<RepairIssue> {
    let mut issues = Vec::new();
    let required = frontend_experience
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !required {
        return issues;
    }
    let Some(contract) = frontend_experience.get("uiQualityContract") else {
        issues.push(issue(
            "UI_QUALITY_CONTRACT_REQUIRED",
            "content.frontendExperience.uiQualityContract",
            "frontendExperience.required=true requires uiQualityContract so UI quality is planned at generation time.",
        ));
        return issues;
    };
    if !contract.is_object() {
        issues.push(issue(
            "UI_QUALITY_CONTRACT_INVALID",
            "content.frontendExperience.uiQualityContract",
            "uiQualityContract must be an object.",
        ));
        return issues;
    }
    require_string_in(
        contract,
        "/scenario/kind",
        "content.frontendExperience.uiQualityContract.scenario.kind",
        &UI_SCENARIO_KINDS,
        "UI_QUALITY_SCENARIO_INVALID",
        &mut issues,
    );
    require_string_in(
        contract,
        "/qualityLevel",
        "content.frontendExperience.uiQualityContract.qualityLevel",
        &UI_QUALITY_LEVELS,
        "UI_QUALITY_LEVEL_INVALID",
        &mut issues,
    );
    require_string_in(
        contract,
        "/surfacePolicy",
        "content.frontendExperience.uiQualityContract.surfacePolicy",
        &UI_SURFACE_POLICIES,
        "UI_QUALITY_SURFACE_POLICY_INVALID",
        &mut issues,
    );
    require_string_in(
        contract,
        "/layoutBaseline",
        "content.frontendExperience.uiQualityContract.layoutBaseline",
        &UI_LAYOUT_BASELINES,
        "UI_QUALITY_LAYOUT_BASELINE_INVALID",
        &mut issues,
    );
    require_string_in(
        contract,
        "/density",
        "content.frontendExperience.uiQualityContract.density",
        &UI_DENSITIES,
        "UI_QUALITY_DENSITY_INVALID",
        &mut issues,
    );
    require_string_in(
        contract,
        "/semanticTokenPolicy",
        "content.frontendExperience.uiQualityContract.semanticTokenPolicy",
        &UI_SEMANTIC_TOKEN_POLICIES,
        "UI_QUALITY_TOKEN_POLICY_INVALID",
        &mut issues,
    );
    require_non_empty_string(
        contract,
        "/scenario/reason",
        "content.frontendExperience.uiQualityContract.scenario.reason",
        "UI_QUALITY_SCENARIO_REASON_REQUIRED",
        &mut issues,
    );
    let scenario_kind = contract
        .pointer("/scenario/kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let scenario_ref = contract
        .pointer("/scenario/referenceId")
        .and_then(Value::as_str);
    let expected_scenario_ref = scenario_reference_id(scenario_kind);
    if scenario_ref != Some(expected_scenario_ref) {
        issues.push(issue(
            "UI_QUALITY_SCENARIO_REFERENCE_INVALID",
            "content.frontendExperience.uiQualityContract.scenario.referenceId",
            "scenario.referenceId must match the selected scenario kind.",
        ));
    }
    validate_reference_ids(contract, expected_scenario_ref, &mut issues);
    validate_required_string_array(
        contract,
        "/forbiddenUserVisibleContent",
        "content.frontendExperience.uiQualityContract.forbiddenUserVisibleContent",
        &UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "UI_QUALITY_FORBIDDEN_CONTENT_INVALID",
        &mut issues,
    );
    validate_required_ui_states(contract, &mut issues);
    validate_business_rules(contract, &mut issues);
    issues
}

pub fn known_ui_reference_ids() -> Vec<&'static str> {
    let mut ids = Vec::new();
    ids.extend(UI_CORE_REFERENCE_IDS);
    ids.extend(UI_SCENARIO_REFERENCE_IDS);
    ids.extend(UI_STACK_REFERENCE_IDS);
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn infer_primary_scenario(
    frontend: Option<&FrontendExperience>,
    baseline: Option<&TechnicalBaselineContract>,
) -> &'static str {
    let haystack = ui_haystack(frontend, baseline);
    if contains_any(&haystack, &["three", "3d", "webgl", "canvas", "immersive"]) {
        "immersive_3d"
    } else if contains_any(
        &haystack,
        &["docs", "documentation", "文档", "知识库", "guide"],
    ) {
        "docs_site"
    } else if contains_any(
        &haystack,
        &["marketing", "landing", "官网", "营销", "campaign"],
    ) {
        "marketing_site"
    } else if contains_any(&haystack, &["corporate", "company", "企业官网", "品牌"]) {
        "corporate_site"
    } else if contains_any(
        &haystack,
        &[
            "fintech", "finance", "stock", "trading", "证券", "资金", "交易", "账户",
        ],
    ) {
        if contains_any(&haystack, &["consumer", "client", "用户端", "app"]) {
            "fintech_consumer_app"
        } else {
            "fintech_workstation"
        }
    } else if contains_any(
        &haystack,
        &[
            "dashboard",
            "admin",
            "后台",
            "管理",
            "staff",
            "operator",
            "工作人员",
        ],
    ) {
        "admin_dashboard"
    } else if contains_any(
        &haystack,
        &[
            "table",
            "grid",
            "data",
            "report",
            "analytics",
            "列表",
            "console",
        ],
    ) {
        "data_console"
    } else if contains_any(
        &haystack,
        &[
            "developer",
            "devtool",
            "ide",
            "debug",
            "sdk",
            "api explorer",
        ],
    ) {
        "developer_tool"
    } else if contains_any(
        &haystack,
        &["native", "ios", "android", "react native", "flutter"],
    ) {
        "mobile_native"
    } else if contains_any(&haystack, &["mobile", "responsive", "h5", "app"]) {
        "mobile_responsive"
    } else if contains_any(&haystack, &["consumer", "customer", "shop", "content"]) {
        "consumer_app"
    } else {
        "custom_product_ui"
    }
}

fn scenario_reference_id(scenario: &str) -> &'static str {
    match scenario {
        "admin_dashboard" => "uix.scenarios.admin-dashboard",
        "data_console" => "uix.scenarios.data-console",
        "fintech_workstation" => "uix.scenarios.fintech-workstation",
        "fintech_consumer_app" => "uix.scenarios.fintech-consumer-app",
        "consumer_app" => "uix.scenarios.consumer-app",
        "mobile_responsive" => "uix.scenarios.mobile-responsive",
        "mobile_native" => "uix.scenarios.mobile-native",
        "marketing_site" => "uix.scenarios.marketing-site",
        "corporate_site" => "uix.scenarios.corporate-site",
        "docs_site" => "uix.scenarios.docs-site",
        "developer_tool" => "uix.scenarios.developer-tool",
        "immersive_3d" => "uix.scenarios.immersive-3d",
        _ => "uix.core",
    }
}

fn scenario_candidates(primary: &str) -> Vec<Value> {
    let mut candidates = vec![scenario_candidate(
        primary,
        "primary signal from confirmed frontend target and technical baseline",
    )];
    for fallback in fallback_scenarios(primary) {
        candidates.push(scenario_candidate(
            fallback,
            "fallback when the concrete surface semantics fit this scenario better",
        ));
    }
    candidates
}

fn scenario_candidate(kind: &str, reason: &str) -> Value {
    json!({
        "kind": kind,
        "referenceId": scenario_reference_id(kind),
        "reason": reason
    })
}

fn fallback_scenarios(primary: &str) -> Vec<&'static str> {
    match primary {
        "fintech_workstation" => vec!["admin_dashboard", "data_console"],
        "admin_dashboard" => vec!["data_console", "custom_product_ui"],
        "data_console" => vec!["admin_dashboard", "custom_product_ui"],
        "mobile_native" => vec!["mobile_responsive", "consumer_app"],
        "mobile_responsive" => vec!["consumer_app", "custom_product_ui"],
        "fintech_consumer_app" => vec!["consumer_app", "mobile_responsive"],
        "immersive_3d" => vec!["consumer_app", "custom_product_ui"],
        "docs_site" => vec!["developer_tool", "corporate_site"],
        "marketing_site" => vec!["corporate_site", "consumer_app"],
        "corporate_site" => vec!["marketing_site", "docs_site"],
        "developer_tool" => vec!["data_console", "docs_site"],
        "consumer_app" => vec!["mobile_responsive", "custom_product_ui"],
        _ => vec!["admin_dashboard", "consumer_app"],
    }
}

fn frontend_quality_level(frontend: Option<&FrontendExperience>) -> &'static str {
    match frontend.map(|item| item.experience_level) {
        Some(FrontendExperienceLevel::None) | None => "usable_internal_product",
        Some(FrontendExperienceLevel::TechnicalDemo) => "usable_internal_product",
        Some(FrontendExperienceLevel::UsableInternalProduct) => "production_internal_product",
        Some(FrontendExperienceLevel::PolishedProduct) => "polished_product",
    }
}

fn surface_policy_candidates(primary: &str) -> Vec<&'static str> {
    match primary {
        "marketing_site" => vec!["marketing_ui"],
        "corporate_site" | "docs_site" => vec!["documentation_ui"],
        "developer_tool" => vec!["developer_runtime_ui", "business_ui_only"],
        _ => vec!["business_ui_only"],
    }
}

fn layout_baseline_candidates(primary: &str) -> Vec<&'static str> {
    match primary {
        "admin_dashboard" | "fintech_workstation" => {
            vec!["sidebar_topbar_table_detail", "data_console"]
        }
        "data_console" | "developer_tool" => vec!["data_console", "sidebar_topbar_table_detail"],
        "mobile_responsive" | "fintech_consumer_app" | "consumer_app" => {
            vec!["mobile_task_flow", "custom_product_layout"]
        }
        "mobile_native" => vec!["native_mobile_stack", "mobile_task_flow"],
        "docs_site" => vec!["docs_shell"],
        "marketing_site" => vec!["marketing_narrative"],
        "corporate_site" => vec!["corporate_information", "marketing_narrative"],
        "immersive_3d" => vec!["scene_first_3d"],
        _ => vec!["custom_product_layout"],
    }
}

fn density_candidates(primary: &str) -> Vec<&'static str> {
    match primary {
        "admin_dashboard" | "data_console" | "fintech_workstation" | "developer_tool" => {
            vec!["workbench_dense", "balanced"]
        }
        "marketing_site" | "immersive_3d" => vec!["immersive", "comfortable"],
        "mobile_native" | "mobile_responsive" | "consumer_app" | "fintech_consumer_app" => {
            vec!["comfortable", "balanced"]
        }
        _ => vec!["balanced"],
    }
}

fn infer_stack_reference_ids(baseline: Option<&TechnicalBaselineContract>) -> Vec<String> {
    let stack = baseline
        .map(|item| item.stack.to_string().to_lowercase())
        .unwrap_or_default();
    let mut refs = Vec::new();
    if contains_any(&stack, &["react", "next", "vite"]) {
        refs.push("uix.stacks.react".to_string());
    }
    if contains_any(&stack, &["vue", "nuxt"]) {
        refs.push("uix.stacks.vue".to_string());
    }
    if contains_any(&stack, &["svelte", "sveltekit"]) {
        refs.push("uix.stacks.svelte".to_string());
    }
    if contains_any(&stack, &["html", "vanilla", "plain"]) {
        refs.push("uix.stacks.plain-html".to_string());
    }
    if contains_any(
        &stack,
        &[
            "react native",
            "flutter",
            "swift",
            "kotlin",
            "ios",
            "android",
        ],
    ) {
        refs.push("uix.stacks.native-mobile".to_string());
    }
    if contains_any(&stack, &["three", "webgl", "3d"]) {
        refs.push("uix.stacks.threejs".to_string());
    }
    if contains_any(
        &stack,
        &[
            "uniapp",
            "uni-app",
            "miniapp",
            "mini app",
            "mini-program",
            "wechat",
            "weixin",
            "小程序",
        ],
    ) {
        refs.push("uix.stacks.uniapp".to_string());
    }
    refs.sort();
    refs.dedup();
    refs
}

fn ui_haystack(
    frontend: Option<&FrontendExperience>,
    baseline: Option<&TechnicalBaselineContract>,
) -> String {
    let mut parts = Vec::new();
    if let Some(frontend) = frontend {
        parts.push(frontend.kind.clone());
        for audience in &frontend.audiences {
            parts.push(audience.name.clone());
            parts.extend(audience.primary_jobs.clone());
        }
        for surface in &frontend.surfaces {
            parts.push(surface.name.clone());
            parts.extend(surface.primary_jobs.clone());
        }
        for view in &frontend.data_views {
            parts.push(view.name.clone());
            parts.push(view.purpose.clone());
            if let Some(target) = &view.target_object {
                parts.push(target.clone());
            }
        }
        for action in &frontend.actions {
            parts.push(action.label.clone());
            if let Some(target) = &action.target_object {
                parts.push(target.clone());
            }
        }
        for path in &frontend.operation_paths {
            parts.push(path.name.clone());
            parts.push(path.user_goal.clone());
            parts.push(path.selection_summary.clone());
            if let Some(target) = &path.target_object {
                parts.push(target.clone());
            }
        }
        parts.extend(frontend.must_not.clone());
        if let Some(summary) = &frontend.confirmation_summary {
            parts.push(summary.clone());
        }
    }
    if let Some(baseline) = baseline {
        parts.push(baseline.stack.to_string());
        parts.push(serde_json::to_string(&baseline.project_kind).unwrap_or_default());
        parts.push(serde_json::to_string(&baseline.scope).unwrap_or_default());
    }
    parts.join(" ").to_lowercase()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn validate_reference_ids(
    contract: &Value,
    expected_scenario_ref: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let known = known_ui_reference_ids()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let Some(items) = contract
        .pointer("/referenceProfile/referenceIds")
        .and_then(Value::as_array)
    else {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_PROFILE_REQUIRED",
            "content.frontendExperience.uiQualityContract.referenceProfile.referenceIds",
            "referenceProfile.referenceIds must list the UIX reference ids used for this frontend contract.",
        ));
        return;
    };
    if items.is_empty() {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_PROFILE_REQUIRED",
            "content.frontendExperience.uiQualityContract.referenceProfile.referenceIds",
            "referenceProfile.referenceIds must not be empty.",
        ));
        return;
    }
    let mut actual = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let Some(id) = item.as_str() else {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_ID_INVALID",
                &format!("content.frontendExperience.uiQualityContract.referenceProfile.referenceIds[{index}]"),
                "reference id must be a string.",
            ));
            continue;
        };
        if !known.contains(id) {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_ID_INVALID",
                &format!("content.frontendExperience.uiQualityContract.referenceProfile.referenceIds[{index}]"),
                "reference id must be one of enumRefs.uiQuality.knownReferenceIds.",
            ));
        }
        actual.insert(id.to_string());
    }
    for required in UI_CORE_REFERENCE_IDS
        .into_iter()
        .chain(std::iter::once(expected_scenario_ref))
    {
        if !actual.contains(required) {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_ID_REQUIRED",
                "content.frontendExperience.uiQualityContract.referenceProfile.referenceIds",
                "referenceProfile.referenceIds must include core, anti-pattern, all core token, and selected scenario UIX references.",
            ));
            break;
        }
    }
    let load_mode = contract
        .pointer("/referenceProfile/loadMode")
        .and_then(Value::as_str);
    if load_mode != Some("skill_reference_by_id") {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_LOAD_MODE_INVALID",
            "content.frontendExperience.uiQualityContract.referenceProfile.loadMode",
            "referenceProfile.loadMode must be skill_reference_by_id.",
        ));
    }
}

fn validate_required_string_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    required_values: &[&str],
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(items) = root.pointer(pointer).and_then(Value::as_array) else {
        issues.push(issue(code, field_path, "field must be an array."));
        return;
    };
    let actual = items
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for required in required_values {
        if !actual.contains(required) {
            issues.push(issue(
                code,
                field_path,
                "field must include all required enum values from the UI quality contract.",
            ));
            return;
        }
    }
}

fn validate_required_ui_states(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(items) = contract.get("requiredUiStates").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_QUALITY_STATES_REQUIRED",
            "content.frontendExperience.uiQualityContract.requiredUiStates",
            "requiredUiStates must list the user-visible states that frontend implementation must cover.",
        ));
        return;
    };
    let mut actual = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let state = item.get("state").and_then(Value::as_str);
        let expectation = item.get("expectation").and_then(Value::as_str);
        match state {
            Some(value) if UI_REQUIRED_STATES.contains(&value) => {
                actual.insert(value);
            }
            _ => issues.push(issue(
                "UI_QUALITY_STATE_INVALID",
                &format!(
                    "content.frontendExperience.uiQualityContract.requiredUiStates[{index}].state"
                ),
                "requiredUiStates.state must be one of enumRefs.uiQuality.requiredUiState.",
            )),
        }
        if expectation.map(str::trim).unwrap_or_default().is_empty() {
            issues.push(issue(
                "UI_QUALITY_STATE_EXPECTATION_REQUIRED",
                &format!("content.frontendExperience.uiQualityContract.requiredUiStates[{index}].expectation"),
                "requiredUiStates entries must include a concrete expectation.",
            ));
        }
    }
    for required in UI_REQUIRED_STATES {
        if !actual.contains(required) {
            issues.push(issue(
                "UI_QUALITY_STATE_REQUIRED",
                "content.frontendExperience.uiQualityContract.requiredUiStates",
                "requiredUiStates must include loading, success, error, empty, and business_blocking.",
            ));
            break;
        }
    }
}

fn validate_business_rules(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(items) = contract.get("businessUiRules").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_QUALITY_BUSINESS_RULES_REQUIRED",
            "content.frontendExperience.uiQualityContract.businessUiRules",
            "businessUiRules must record generation-time UI rules for TaskPlan and Execution.",
        ));
        return;
    };
    if items.is_empty() {
        issues.push(issue(
            "UI_QUALITY_BUSINESS_RULES_REQUIRED",
            "content.frontendExperience.uiQualityContract.businessUiRules",
            "businessUiRules must not be empty.",
        ));
    }
    for (index, item) in items.iter().enumerate() {
        for key in ["ruleId", "expectation"] {
            if item
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                issues.push(issue(
                    "UI_QUALITY_BUSINESS_RULE_INVALID",
                    &format!("content.frontendExperience.uiQualityContract.businessUiRules[{index}].{key}"),
                    "businessUiRules entries must include ruleId and expectation.",
                ));
            }
        }
    }
}

fn require_string_in(
    root: &Value,
    pointer: &str,
    field_path: &str,
    allowed: &[&str],
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(value) = root.pointer(pointer).and_then(Value::as_str) else {
        issues.push(issue(
            code,
            field_path,
            "field must be a string enum value.",
        ));
        return;
    };
    if !allowed.contains(&value) {
        issues.push(issue(
            code,
            field_path,
            "field uses an unsupported enum value.",
        ));
    }
}

fn require_non_empty_string(
    root: &Value,
    pointer: &str,
    field_path: &str,
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    if root
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(issue(code, field_path, "field must be a non-empty string."));
    }
}

fn issue(code: &str, field_path: &str, message: &str) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("candidate".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{build_ui_quality_seed, known_ui_reference_ids, UI_CORE_REFERENCE_IDS};

    #[test]
    fn known_ui_reference_ids_resolve_to_shared_reference_files() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for reference_id in known_ui_reference_ids() {
            let relative = reference_file_for_id(reference_id);
            let path = repo_root
                .join("plugins/shared/loom/references/uix")
                .join(relative);
            assert!(
                path.exists(),
                "UIX reference id {reference_id} must resolve to {}",
                path.display()
            );
        }
    }

    #[test]
    fn ui_quality_seed_includes_all_core_reference_ids() {
        let seed = build_ui_quality_seed(None, None);
        let reference_ids = seed
            .get("requiredReferenceIds")
            .and_then(serde_json::Value::as_array)
            .expect("seed must include requiredReferenceIds")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<std::collections::BTreeSet<_>>();

        for reference_id in UI_CORE_REFERENCE_IDS {
            assert!(
                reference_ids.contains(reference_id),
                "uiQualitySeed.requiredReferenceIds must include {reference_id}"
            );
        }
    }

    #[test]
    fn focused_uix_references_keep_operational_depth() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for reference_id in known_ui_reference_ids() {
            if !reference_id.starts_with("uix.scenarios.")
                && !reference_id.starts_with("uix.tokens.")
                && !reference_id.starts_with("uix.stacks.")
            {
                continue;
            }
            let path = repo_root
                .join("plugins/shared/loom/references/uix")
                .join(reference_file_for_id(reference_id));
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            let line_count = content.lines().count();
            assert!(
                line_count >= 40,
                "UIX reference {reference_id} is too thin ({line_count} lines)"
            );
            assert!(
                content.contains("```"),
                "UIX reference {reference_id} should include a concrete structure or token example"
            );
        }
    }

    fn reference_file_for_id(reference_id: &str) -> String {
        if reference_id == "uix.core" {
            return "core.md".to_string();
        }
        if reference_id == "uix.anti-patterns" {
            return "anti-patterns.md".to_string();
        }
        for (prefix, directory) in [
            ("uix.tokens.", "tokens"),
            ("uix.scenarios.", "scenarios"),
            ("uix.stacks.", "stacks"),
        ] {
            if let Some(name) = reference_id.strip_prefix(prefix) {
                return format!("{directory}/{name}.md");
            }
        }
        panic!("unknown UIX reference id prefix: {reference_id}");
    }
}
