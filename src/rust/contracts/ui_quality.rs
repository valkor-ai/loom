use std::collections::{BTreeMap, BTreeSet};

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

pub const UI_DESIGN_TOKEN_STRATEGIES: [&str; 5] = [
    "reuse_existing",
    "extend_existing",
    "create_css_tokens",
    "create_tailwind_tokens",
    "not_applicable",
];

pub const UI_DESIGN_TOKEN_TEMPLATE_IDS: [&str; 2] = ["tokens-css", "tokens-tailwind"];

pub const UI_DESIGN_TOKEN_MERGE_POLICIES: [&str; 1] = ["preserve_existing_project_tokens"];

pub const UI_DESIGN_TOKEN_DUPLICATION_POLICIES: [&str; 1] = ["do_not_create_parallel_token_system"];

pub const UI_QUALITY_GATE_SEVERITIES: [&str; 2] = ["must", "should"];

pub const UI_QUALITY_GATE_STATUSES: [&str; 5] = [
    "satisfied",
    "partial",
    "missing",
    "blocked_by_environment",
    "not_applicable",
];

pub const UI_FORBIDDEN_USER_VISIBLE_CONTENT: [&str; 5] = [
    "runtime_commands",
    "technical_stack_explanation",
    "delivery_progress_notes",
    "verification_instructions",
    "internal_workflow_terms",
];

pub const UI_REQUIRED_STATES: [&str; 5] =
    ["loading", "success", "error", "empty", "business_blocking"];

pub const UI_REFERENCE_GROUP_KEYS: [&str; 6] = [
    "core",
    "focus",
    "tokens",
    "scenarios",
    "stacks",
    "templates",
];

pub const UI_CORE_REFERENCE_ITEMS: [&str; 6] = [
    "core",
    "anti-patterns",
    "system",
    "interaction",
    "content",
    "verification",
];

pub const UI_FOCUS_REFERENCE_ITEMS: [&str; 3] = ["data", "mobile", "frameworks"];

pub const UI_TOKEN_REFERENCE_ITEMS: [&str; 6] = [
    "color-system",
    "typography",
    "spacing",
    "layout-grid",
    "motion",
    "radius-elevation",
];

pub const UI_SCENARIO_REFERENCE_ITEMS: [&str; 12] = [
    "admin-dashboard",
    "data-console",
    "fintech-workstation",
    "fintech-consumer-app",
    "consumer-app",
    "mobile-responsive",
    "mobile-native",
    "marketing-site",
    "corporate-site",
    "docs-site",
    "developer-tool",
    "immersive-3d",
];

pub const UI_STACK_REFERENCE_ITEMS: [&str; 7] = [
    "react",
    "vue",
    "plain-html",
    "native-mobile",
    "threejs",
    "svelte",
    "uniapp",
];

pub fn ui_quality_enum_refs() -> Value {
    json!({
        "scenarioKind": UI_SCENARIO_KINDS,
        "qualityLevel": UI_QUALITY_LEVELS,
        "surfacePolicy": UI_SURFACE_POLICIES,
        "layoutBaseline": UI_LAYOUT_BASELINES,
        "density": UI_DENSITIES,
        "semanticTokenPolicy": UI_SEMANTIC_TOKEN_POLICIES,
        "designTokenStrategy": UI_DESIGN_TOKEN_STRATEGIES,
        "designTokenTemplateId": UI_DESIGN_TOKEN_TEMPLATE_IDS,
        "designTokenMergePolicy": UI_DESIGN_TOKEN_MERGE_POLICIES,
        "designTokenDuplicationPolicy": UI_DESIGN_TOKEN_DUPLICATION_POLICIES,
        "qualityGateSeverity": UI_QUALITY_GATE_SEVERITIES,
        "qualityGateStatus": UI_QUALITY_GATE_STATUSES,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiState": UI_REQUIRED_STATES,
        "knownReferenceGroups": known_ui_reference_groups()
    })
}

pub fn ui_quality_contract_shape() -> Value {
    json!({
        "scenario": {
            "kind": UI_SCENARIO_KINDS.join(" | "),
            "reference": {
                "group": "known UIX reference group",
                "item": "known UIX reference item"
            },
            "reason": "string"
        },
        "qualityLevel": UI_QUALITY_LEVELS.join(" | "),
        "surfacePolicy": UI_SURFACE_POLICIES.join(" | "),
        "layoutBaseline": UI_LAYOUT_BASELINES.join(" | "),
        "density": UI_DENSITIES.join(" | "),
        "semanticTokenPolicy": UI_SEMANTIC_TOKEN_POLICIES.join(" | "),
        "referenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": known_ui_reference_groups(),
            "referenceLoadPlan": [{
                "refId": "uix.core.core",
                "path": "uix/core.md",
                "reason": "Selected UIX reference file."
            }]
        },
        "designTokenAssetPlan": {
            "strategy": UI_DESIGN_TOKEN_STRATEGIES.join(" | "),
            "templateId": "known design token template item or null",
            "targetFiles": ["project-relative path"],
            "existingStyleEvidence": {
                "tailwindConfigRefs": ["project-relative path"],
                "tokenFileRefs": ["project-relative path"],
                "globalStyleRefs": ["project-relative path"],
                "componentThemeRefs": ["project-relative path"],
                "summary": "string"
            },
            "mergePolicy": UI_DESIGN_TOKEN_MERGE_POLICIES.join(" | "),
            "duplicationPolicy": UI_DESIGN_TOKEN_DUPLICATION_POLICIES.join(" | ")
        },
        "forbiddenUserVisibleContent": [UI_FORBIDDEN_USER_VISIBLE_CONTENT.join(" | ")],
        "requiredUiStates": [{
            "state": UI_REQUIRED_STATES.join(" | "),
            "expectation": "string"
        }],
        "businessUiRules": [{
            "ruleId": "string",
            "expectation": "string"
        }],
        "qualityGates": [{
            "gateId": "known UI gate id",
            "sourceRefId": "uix reference id such as uix.scenarios.admin-dashboard",
            "severity": UI_QUALITY_GATE_SEVERITIES.join(" | "),
            "appliesToSurfaceRoles": ["app_shell | page | record_list | record_detail | form | action_panel | navigation"],
            "expectation": "short executable UI quality expectation",
            "evidenceRequired": ["changed_files", "state_coverage", "source_check", "render_or_environment_reason"]
        }]
    })
}

pub fn build_ui_quality_seed(
    frontend: Option<&FrontendExperience>,
    baseline: Option<&TechnicalBaselineContract>,
) -> Value {
    let required = frontend.map(|item| item.required).unwrap_or(false);
    let primary_scenario = infer_primary_scenario(frontend, baseline);
    let stack_items = infer_stack_reference_items(baseline);
    let design_token_seed = design_token_asset_seed(baseline);
    let required_reference_groups =
        required_reference_groups(primary_scenario, &stack_items, &design_token_seed);
    let reference_load_plan = ui_reference_load_plan(&required_reference_groups);
    json!({
        "required": required,
        "scenarioCandidates": scenario_candidates(primary_scenario),
        "qualityLevel": frontend_quality_level(frontend),
        "surfacePolicyCandidates": surface_policy_candidates(primary_scenario),
        "layoutBaselineCandidates": layout_baseline_candidates(primary_scenario),
        "densityCandidates": density_candidates(primary_scenario),
        "semanticTokenPolicy": "semantic_tokens_required",
        "requiredReferenceGroups": required_reference_groups,
        "referenceLoadPlan": reference_load_plan,
        "stackReferenceCandidates": stack_items,
        "designTokenAssetPlan": design_token_seed,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiStates": UI_REQUIRED_STATES,
        "selectionRule": "Pick one scenarioKind from scenarioCandidates, copy requiredReferenceGroups into referenceProfile.groups, copy referenceLoadPlan into referenceProfile.referenceLoadPlan, and add only known group items from enumRefs.uiQuality. Use referenceLoadPlan as the only file-loading authority; groups are evidence labels."
    })
}

pub fn ui_quality_contract_template(ui_quality_seed: &Value) -> Value {
    let scenario = ui_quality_seed
        .pointer("/scenarioCandidates/0/kind")
        .and_then(Value::as_str)
        .unwrap_or("custom_product_ui");
    let scenario_reference = ui_quality_seed
        .pointer("/scenarioCandidates/0/reference")
        .cloned()
        .unwrap_or_else(|| scenario_reference_value("custom_product_ui"));
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
    let reference_groups = ui_quality_seed
        .get("requiredReferenceGroups")
        .cloned()
        .filter(|groups| groups.as_object().is_some_and(|object| !object.is_empty()))
        .unwrap_or_else(|| {
            let design_token_asset_plan = ui_quality_seed
                .get("designTokenAssetPlan")
                .cloned()
                .unwrap_or_else(default_design_token_asset_plan);
            required_reference_groups(scenario, &[], &design_token_asset_plan)
        });
    let reference_load_plan = ui_quality_seed
        .get("referenceLoadPlan")
        .cloned()
        .filter(|plan| plan.as_array().is_some_and(|items| !items.is_empty()))
        .unwrap_or_else(|| ui_reference_load_plan(&reference_groups));
    let design_token_asset_plan = ui_quality_seed
        .get("designTokenAssetPlan")
        .cloned()
        .unwrap_or_else(default_design_token_asset_plan);
    let quality_gates =
        ui_quality_gates_for_contract(scenario, &reference_groups, &design_token_asset_plan);

    json!({
        "scenario": {
            "kind": scenario,
            "reference": scenario_reference,
            "reason": "Selected from uiQualitySeed.scenarioCandidates for the confirmed frontend surfaces and product context."
        },
        "qualityLevel": quality_level,
        "surfacePolicy": surface_policy,
        "layoutBaseline": layout_baseline,
        "density": density,
        "semanticTokenPolicy": semantic_token_policy,
        "referenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": reference_groups,
            "referenceLoadPlan": reference_load_plan
        },
        "designTokenAssetPlan": design_token_asset_plan,
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
                "ruleId": "current_business_surface_complete",
                "expectation": "The task-owned user-visible surface completes the current-phase business workflow with its required data, actions, and feedback."
            }
        ],
        "qualityGates": quality_gates
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
    if contract.pointer("/scenario/referenceId").is_some() {
        issues.push(issue(
            "UI_QUALITY_LEGACY_REFERENCE_FIELD_NOT_ALLOWED",
            "content.frontendExperience.uiQualityContract.scenario.referenceId",
            "scenario.referenceId is not allowed; use scenario.reference group/item instead.",
        ));
    }
    let expected_scenario_ref = scenario_reference_value(scenario_kind);
    if contract.pointer("/scenario/reference") != Some(&expected_scenario_ref) {
        issues.push(issue(
            "UI_QUALITY_SCENARIO_REFERENCE_INVALID",
            "content.frontendExperience.uiQualityContract.scenario.reference",
            "scenario.reference must match the selected scenario kind as a group/item reference.",
        ));
    }
    validate_reference_groups(contract, &mut issues);
    validate_required_string_array(
        contract,
        "/forbiddenUserVisibleContent",
        "content.frontendExperience.uiQualityContract.forbiddenUserVisibleContent",
        &UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "UI_QUALITY_FORBIDDEN_CONTENT_INVALID",
        &mut issues,
    );
    validate_design_token_asset_plan(contract, &mut issues);
    validate_required_ui_states(contract, &mut issues);
    validate_business_rules(contract, &mut issues);
    validate_quality_gates(contract, &mut issues);
    issues
}

pub fn ui_quality_gates_for_contract(
    scenario: &str,
    reference_groups: &Value,
    design_token_plan: &Value,
) -> Value {
    let mut gates = Vec::new();
    push_gate(
        &mut gates,
        "anti.product_boundary.no_internal_process",
        "uix.core.anti-patterns",
        "must",
        &["app_shell", "page", "navigation", "record_list", "record_detail", "form"],
        "User-visible UI must not expose Loom/MCP terms, delivery progress, runtime commands, verification instructions, stack explanations, request ids, or future-phase planning language.",
        &["changed_files", "source_check", "forbidden_content_check"],
    );
    push_gate(
        &mut gates,
        "verify.rendered_viewports",
        "uix.core.verification",
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form"],
        "When a local preview is available, record desktop and mobile rendered inspection; when unavailable, record blocked_by_environment with the concrete blocker and fallback source checks.",
        &["render_or_environment_reason", "viewport_check", "fallback_source_check"],
    );
    match scenario {
        "admin_dashboard" | "fintech_workstation" => {
            push_admin_gates(&mut gates);
        }
        "data_console" | "developer_tool" => {
            push_data_gates(&mut gates);
        }
        "mobile_responsive" | "consumer_app" | "fintech_consumer_app" => {
            push_mobile_gates(&mut gates);
        }
        _ => {}
    }
    if reference_group_contains(reference_groups, "scenarios", "admin-dashboard") {
        push_admin_gates(&mut gates);
    }
    if reference_group_contains(reference_groups, "focus", "data")
        || reference_group_contains(reference_groups, "scenarios", "data-console")
    {
        push_data_gates(&mut gates);
    }
    if reference_group_contains(reference_groups, "focus", "mobile")
        || reference_group_contains(reference_groups, "scenarios", "mobile-responsive")
        || reference_group_contains(reference_groups, "scenarios", "mobile-native")
    {
        push_mobile_gates(&mut gates);
    }
    if reference_group_contains(reference_groups, "focus", "frameworks") {
        push_gate(
            &mut gates,
            "framework.component_structure",
            "uix.focus.frameworks",
            "must",
            &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
            "Real screens must separate shell, page orchestration, feature components, shared primitives, data/API helpers, and state-specific components when the workflow spans multiple regions.",
            &["changed_files", "component_split_evidence"],
        );
    }
    if reference_group_contains(reference_groups, "stacks", "react") {
        push_gate(
            &mut gates,
            "react.split.workflow_regions",
            "uix.stacks.react",
            "must",
            &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
            "React workbench UI must keep page orchestration separate from reusable feature components, data/API modules, formatters, and state-specific UI.",
            &["changed_files", "component_split_evidence", "state_ownership_evidence"],
        );
    }
    if design_token_plan
        .get("strategy")
        .and_then(Value::as_str)
        .is_some_and(|strategy| strategy != "not_applicable")
    {
        push_token_gates(&mut gates, design_token_plan);
    }
    dedupe_gates(gates)
}

fn push_admin_gates(gates: &mut Vec<Value>) {
    push_gate(
        gates,
        "admin.shell.work_surface",
        "uix.scenarios.admin-dashboard",
        "must",
        &["app_shell", "page"],
        "The first viewport must be the working business console with navigation, current page context, a real work region, and primary business action access.",
        &["changed_files", "surface_evidence", "source_check"],
    );
    push_gate(
        gates,
        "admin.topbar.context_actions",
        "uix.scenarios.admin-dashboard",
        "should",
        &["app_shell", "navigation", "page"],
        "Topbar/header content must provide operational context and relevant actions such as search, filters, user/workspace context, or primary action; it must not be filler description.",
        &["changed_files", "surface_evidence"],
    );
    push_gate(
        gates,
        "admin.list.filter_table_detail",
        "uix.scenarios.admin-dashboard",
        "must",
        &["record_list", "record_detail", "page"],
        "Record-management screens must preserve list context across filter, pagination, row selection, detail viewing, and mutations.",
        &["changed_files", "state_coverage", "workflow_evidence"],
    );
}

fn push_data_gates(gates: &mut Vec<Value>) {
    push_gate(
        gates,
        "data.surface.scan_action_path",
        "uix.focus.data",
        "must",
        &["record_list", "record_detail", "page"],
        "Data surfaces must show object identity, status, key fields, and available action in the same scan path, with loading, empty, error, and business-blocking states placed near the affected region.",
        &["changed_files", "state_coverage", "surface_evidence"],
    );
    push_gate(
        gates,
        "admin.state.scoped_feedback",
        "uix.core.interaction",
        "must",
        &["record_list", "record_detail", "form", "action_panel"],
        "Loading, success, validation, error, and business-blocking feedback must appear near the table, form, detail, row, or action they affect instead of only in a generic global message.",
        &["changed_files", "state_coverage", "business_feedback_evidence"],
    );
}

fn push_mobile_gates(gates: &mut Vec<Value>) {
    push_gate(
        gates,
        "admin.mobile.record_fallback",
        "uix.focus.mobile",
        "must",
        &["record_list", "record_detail", "page"],
        "Responsive record-management UI must keep the workflow usable on narrow screens through cards, drawer/detail route, or an explicit source-checked fallback; do not rely only on shrinking a dense table.",
        &["changed_files", "viewport_check", "responsive_source_check"],
    );
}

fn push_token_gates(gates: &mut Vec<Value>, design_token_plan: &Value) {
    let template_ref = match design_token_plan.get("templateId").and_then(Value::as_str) {
        Some("tokens-tailwind") => "uix.templates.tokens-tailwind",
        _ => "uix.templates.tokens-css",
    };
    push_gate(
        gates,
        "token.semantic_roles.coverage",
        template_ref,
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
        "Token assets must cover semantic surface, text, border, primary, status, focus, control, shell, table/list, and detail/action roles needed by the implemented UI.",
        &["token_asset_files", "token_consumer_files", "source_check"],
    );
    push_gate(
        gates,
        "token.single_source_consumed",
        "uix.core.system",
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
        "The UI must consume one token/theme source through the project style entry or component system and must not create a parallel token system.",
        &["token_asset_files", "token_consumer_files", "source_check"],
    );
}

fn push_gate(
    gates: &mut Vec<Value>,
    gate_id: &str,
    source_ref_id: &str,
    severity: &str,
    surface_roles: &[&str],
    expectation: &str,
    evidence_required: &[&str],
) {
    gates.push(json!({
        "gateId": gate_id,
        "sourceRefId": source_ref_id,
        "severity": severity,
        "appliesToSurfaceRoles": surface_roles,
        "expectation": expectation,
        "evidenceRequired": evidence_required
    }));
}

fn dedupe_gates(gates: Vec<Value>) -> Value {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for gate in gates {
        let Some(gate_id) = gate.get("gateId").and_then(Value::as_str) else {
            continue;
        };
        if seen.insert(gate_id.to_string()) {
            deduped.push(gate);
        }
    }
    Value::Array(deduped)
}

pub fn known_ui_reference_groups() -> Value {
    json!({
        "core": UI_CORE_REFERENCE_ITEMS,
        "focus": UI_FOCUS_REFERENCE_ITEMS,
        "tokens": UI_TOKEN_REFERENCE_ITEMS,
        "scenarios": UI_SCENARIO_REFERENCE_ITEMS,
        "stacks": UI_STACK_REFERENCE_ITEMS,
        "templates": UI_DESIGN_TOKEN_TEMPLATE_IDS
    })
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

fn scenario_reference_item(scenario: &str) -> (&'static str, &'static str) {
    match scenario {
        "admin_dashboard" => ("scenarios", "admin-dashboard"),
        "data_console" => ("scenarios", "data-console"),
        "fintech_workstation" => ("scenarios", "fintech-workstation"),
        "fintech_consumer_app" => ("scenarios", "fintech-consumer-app"),
        "consumer_app" => ("scenarios", "consumer-app"),
        "mobile_responsive" => ("scenarios", "mobile-responsive"),
        "mobile_native" => ("scenarios", "mobile-native"),
        "marketing_site" => ("scenarios", "marketing-site"),
        "corporate_site" => ("scenarios", "corporate-site"),
        "docs_site" => ("scenarios", "docs-site"),
        "developer_tool" => ("scenarios", "developer-tool"),
        "immersive_3d" => ("scenarios", "immersive-3d"),
        _ => ("core", "core"),
    }
}

fn scenario_reference_value(scenario: &str) -> Value {
    let (group, item) = scenario_reference_item(scenario);
    json!({
        "group": group,
        "item": item
    })
}

fn scenario_supporting_reference_items(scenario: &str) -> Vec<(&'static str, &'static str)> {
    let mut refs = vec![scenario_reference_item(scenario)];
    match scenario {
        "admin_dashboard" => {
            refs.extend([
                ("scenarios", "data-console"),
                ("focus", "data"),
                ("focus", "mobile"),
            ]);
        }
        "data_console" => {
            refs.extend([
                ("scenarios", "admin-dashboard"),
                ("focus", "data"),
                ("focus", "mobile"),
            ]);
        }
        "fintech_workstation" => {
            refs.extend([
                ("scenarios", "admin-dashboard"),
                ("scenarios", "data-console"),
                ("focus", "data"),
                ("focus", "mobile"),
            ]);
        }
        "developer_tool" => {
            refs.extend([
                ("scenarios", "data-console"),
                ("focus", "data"),
                ("focus", "mobile"),
            ]);
        }
        "consumer_app" | "fintech_consumer_app" => {
            refs.extend([("scenarios", "mobile-responsive"), ("focus", "mobile")]);
        }
        "mobile_responsive" | "mobile_native" => {
            refs.push(("focus", "mobile"));
        }
        "marketing_site" | "corporate_site" | "docs_site" => {
            refs.push(("focus", "mobile"));
        }
        "immersive_3d" => {
            refs.push(("focus", "mobile"));
        }
        _ => {}
    }
    refs.sort_unstable();
    refs.dedup();
    refs
}

fn required_reference_groups(
    scenario: &str,
    stack_items: &[String],
    design_token_plan: &Value,
) -> Value {
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    for item in UI_CORE_REFERENCE_ITEMS {
        push_reference_group_item(&mut groups, "core", item);
    }
    for item in UI_TOKEN_REFERENCE_ITEMS {
        push_reference_group_item(&mut groups, "tokens", item);
    }
    for (group, item) in scenario_supporting_reference_items(scenario) {
        push_reference_group_item(&mut groups, group, item);
    }
    if !stack_items.is_empty() {
        push_reference_group_item(&mut groups, "focus", "frameworks");
        for item in stack_items {
            push_reference_group_item(&mut groups, "stacks", item);
        }
    }
    if let Some(template_id) = design_token_plan.get("templateId").and_then(Value::as_str) {
        push_reference_group_item(&mut groups, "templates", template_id);
    }
    reference_groups_value(groups)
}

fn push_reference_group_item(
    groups: &mut BTreeMap<String, BTreeSet<String>>,
    group: &str,
    item: &str,
) {
    groups
        .entry(group.to_string())
        .or_default()
        .insert(item.to_string());
}

fn reference_groups_value(groups: BTreeMap<String, BTreeSet<String>>) -> Value {
    let mut object = serde_json::Map::new();
    for group in UI_REFERENCE_GROUP_KEYS {
        let values = groups
            .get(group)
            .map(|items| {
                items
                    .iter()
                    .map(|item| Value::String(item.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !values.is_empty() {
            object.insert(group.to_string(), Value::Array(values));
        }
    }
    Value::Object(object)
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
        "reference": scenario_reference_value(kind),
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

fn infer_stack_reference_items(baseline: Option<&TechnicalBaselineContract>) -> Vec<String> {
    let stack = baseline
        .map(|item| item.stack.to_string().to_lowercase())
        .unwrap_or_default();
    let mut refs = Vec::new();
    if contains_any(&stack, &["react", "next", "vite"]) {
        refs.push("react".to_string());
    }
    if contains_any(&stack, &["vue", "nuxt"]) {
        refs.push("vue".to_string());
    }
    if contains_any(&stack, &["svelte", "sveltekit"]) {
        refs.push("svelte".to_string());
    }
    if contains_any(&stack, &["html", "vanilla", "plain"]) {
        refs.push("plain-html".to_string());
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
        refs.push("native-mobile".to_string());
    }
    if contains_any(&stack, &["three", "webgl", "3d"]) {
        refs.push("threejs".to_string());
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
        refs.push("uniapp".to_string());
    }
    refs.sort();
    refs.dedup();
    refs
}

fn design_token_asset_seed(baseline: Option<&TechnicalBaselineContract>) -> Value {
    let stack = baseline
        .map(|item| item.stack.to_string().to_lowercase())
        .unwrap_or_default();
    if contains_any(
        &stack,
        &[
            "tailwind",
            "shadcn",
            "@tailwind",
            "daisyui",
            "nuxt tailwind",
        ],
    ) {
        json!({
            "strategy": "create_tailwind_tokens",
            "templateId": "tokens-tailwind",
            "targetFiles": ["tailwind.config.js"],
            "existingStyleEvidence": empty_style_evidence("No repository style evidence is available in the seed. During architecture, inspect RepositoryContext and project files; switch to reuse_existing or extend_existing when existing token/theme assets are found."),
            "mergePolicy": "preserve_existing_project_tokens",
            "duplicationPolicy": "do_not_create_parallel_token_system"
        })
    } else if contains_any(
        &stack,
        &[
            "react native",
            "flutter",
            "swift",
            "kotlin",
            "ios",
            "android",
            "native",
        ],
    ) {
        json!({
            "strategy": "not_applicable",
            "templateId": Value::Null,
            "targetFiles": [],
            "existingStyleEvidence": empty_style_evidence("Native mobile stacks should use the platform or existing app theme. CSS/Tailwind token templates are not directly applicable."),
            "mergePolicy": "preserve_existing_project_tokens",
            "duplicationPolicy": "do_not_create_parallel_token_system"
        })
    } else {
        json!({
            "strategy": "create_css_tokens",
            "templateId": "tokens-css",
            "targetFiles": ["src/styles/tokens.css"],
            "existingStyleEvidence": empty_style_evidence("No repository style evidence is available in the seed. During architecture, inspect RepositoryContext and project files; switch to reuse_existing or extend_existing when existing token/theme assets are found."),
            "mergePolicy": "preserve_existing_project_tokens",
            "duplicationPolicy": "do_not_create_parallel_token_system"
        })
    }
}

fn default_design_token_asset_plan() -> Value {
    json!({
        "strategy": "create_css_tokens",
        "templateId": "tokens-css",
        "targetFiles": ["src/styles/tokens.css"],
        "existingStyleEvidence": empty_style_evidence("No design token evidence was supplied."),
        "mergePolicy": "preserve_existing_project_tokens",
        "duplicationPolicy": "do_not_create_parallel_token_system"
    })
}

fn empty_style_evidence(summary: &str) -> Value {
    json!({
        "tailwindConfigRefs": [],
        "tokenFileRefs": [],
        "globalStyleRefs": [],
        "componentThemeRefs": [],
        "summary": summary
    })
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

fn reference_group_contains(reference_groups: &Value, group: &str, item: &str) -> bool {
    reference_groups
        .get(group)
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|value| value.as_str() == Some(item)))
}

fn validate_reference_groups(contract: &Value, issues: &mut Vec<RepairIssue>) {
    if contract.pointer("/referenceProfile/referenceIds").is_some() {
        issues.push(issue(
            "UI_QUALITY_LEGACY_REFERENCE_FIELD_NOT_ALLOWED",
            "content.frontendExperience.uiQualityContract.referenceProfile.referenceIds",
            "referenceProfile.referenceIds is not allowed; use referenceProfile.referenceLoadPlan and groups instead.",
        ));
    }
    let Some(groups) = contract
        .pointer("/referenceProfile/groups")
        .and_then(Value::as_object)
    else {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_PROFILE_REQUIRED",
            "content.frontendExperience.uiQualityContract.referenceProfile.groups",
            "referenceProfile.groups must list UIX references as group/item arrays.",
        ));
        return;
    };
    if groups.is_empty() {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_PROFILE_REQUIRED",
            "content.frontendExperience.uiQualityContract.referenceProfile.groups",
            "referenceProfile.groups must not be empty.",
        ));
        return;
    }
    let known = known_reference_group_sets();
    let mut actual = BTreeMap::<String, BTreeSet<String>>::new();
    for (group, value) in groups {
        if !UI_REFERENCE_GROUP_KEYS.contains(&group.as_str()) {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_GROUP_INVALID",
                "content.frontendExperience.uiQualityContract.referenceProfile.groups",
                "referenceProfile.groups contains an unsupported group key.",
            ));
            continue;
        }
        let Some(items) = value.as_array() else {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_GROUP_INVALID",
                &format!(
                    "content.frontendExperience.uiQualityContract.referenceProfile.groups.{group}"
                ),
                "referenceProfile group values must be arrays.",
            ));
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            let Some(item) = item.as_str() else {
                issues.push(issue(
                    "UI_QUALITY_REFERENCE_ITEM_INVALID",
                    &format!("content.frontendExperience.uiQualityContract.referenceProfile.groups.{group}[{index}]"),
                    "referenceProfile group items must be strings.",
                ));
                continue;
            };
            if !known
                .get(group.as_str())
                .is_some_and(|allowed| allowed.contains(item))
            {
                issues.push(issue(
                    "UI_QUALITY_REFERENCE_ITEM_INVALID",
                    &format!("content.frontendExperience.uiQualityContract.referenceProfile.groups.{group}[{index}]"),
                    "referenceProfile group item must be one of enumRefs.uiQuality.knownReferenceGroups for that group.",
                ));
            }
            actual
                .entry(group.clone())
                .or_default()
                .insert(item.to_string());
        }
    }
    let scenario_kind = contract
        .pointer("/scenario/kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let design_token_plan = contract.get("designTokenAssetPlan").unwrap_or(&Value::Null);
    let expected_stack_items = actual
        .get("stacks")
        .map(|items| items.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let required_groups =
        required_reference_groups(scenario_kind, &expected_stack_items, design_token_plan);
    let Some(required_object) = required_groups.as_object() else {
        return;
    };
    for (group, required_items) in required_object {
        let actual_items = actual.get(group).cloned().unwrap_or_default();
        for required in required_items.as_array().into_iter().flatten() {
            let Some(required) = required.as_str() else {
                continue;
            };
            if !actual_items.contains(required) {
                issues.push(issue(
                    "UI_QUALITY_REFERENCE_ITEM_REQUIRED",
                    "content.frontendExperience.uiQualityContract.referenceProfile.groups",
                    "referenceProfile.groups must include core, token, selected scenario, companion, stack, and token template UIX reference items required by this contract.",
                ));
                return;
            }
        }
    }
    let load_mode = contract
        .pointer("/referenceProfile/loadMode")
        .and_then(Value::as_str);
    if load_mode != Some("mcp_reference_load_plan") {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_LOAD_MODE_INVALID",
            "content.frontendExperience.uiQualityContract.referenceProfile.loadMode",
            "referenceProfile.loadMode must be mcp_reference_load_plan.",
        ));
    }
    validate_reference_load_plan(contract, &actual, issues);
}

fn known_reference_group_sets() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    let mut groups = BTreeMap::new();
    groups.insert("core", UI_CORE_REFERENCE_ITEMS.into_iter().collect());
    groups.insert("focus", UI_FOCUS_REFERENCE_ITEMS.into_iter().collect());
    groups.insert("tokens", UI_TOKEN_REFERENCE_ITEMS.into_iter().collect());
    groups.insert(
        "scenarios",
        UI_SCENARIO_REFERENCE_ITEMS.into_iter().collect(),
    );
    groups.insert("stacks", UI_STACK_REFERENCE_ITEMS.into_iter().collect());
    groups.insert(
        "templates",
        UI_DESIGN_TOKEN_TEMPLATE_IDS.into_iter().collect(),
    );
    groups
}

pub fn ui_reference_load_plan(reference_groups: &Value) -> Value {
    let Some(groups) = reference_groups.as_object() else {
        return Value::Array(vec![]);
    };
    let mut items = Vec::new();
    for (group, value) in groups {
        let Some(group_items) = value.as_array() else {
            continue;
        };
        for item in group_items.iter().filter_map(Value::as_str) {
            if let Some(path) = ui_reference_path(group, item) {
                items.push(json!({
                    "refId": format!("uix.{group}.{item}"),
                    "path": path,
                    "reason": format!("Selected UIX {group}.{item} reference for the current frontend quality contract.")
                }));
            }
        }
    }
    Value::Array(items)
}

fn ui_reference_path(group: &str, item: &str) -> Option<String> {
    match group {
        "core" | "focus" => Some(format!("uix/{item}.md")),
        "tokens" => Some(format!("uix/tokens/{item}.md")),
        "scenarios" => Some(format!("uix/scenarios/{item}.md")),
        "stacks" => Some(format!("uix/stacks/{item}.md")),
        "templates" => match item {
            "tokens-css" => Some("uix/templates/tokens.css.tpl".to_string()),
            "tokens-tailwind" => Some("uix/templates/tokens.tailwind.tpl".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn validate_reference_load_plan(
    contract: &Value,
    actual_groups: &BTreeMap<String, BTreeSet<String>>,
    issues: &mut Vec<RepairIssue>,
) {
    let expected = ui_reference_load_plan(&json!(actual_groups));
    let expected_paths = expected
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("path").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let Some(plan) = contract
        .pointer("/referenceProfile/referenceLoadPlan")
        .and_then(Value::as_array)
    else {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_LOAD_PLAN_REQUIRED",
            "content.frontendExperience.uiQualityContract.referenceProfile.referenceLoadPlan",
            "referenceProfile.referenceLoadPlan must list exact UIX files selected by referenceProfile.groups.",
        ));
        return;
    };
    let mut actual_paths = BTreeSet::new();
    for (index, item) in plan.iter().enumerate() {
        let Some(path) = item.get("path").and_then(Value::as_str) else {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_LOAD_PLAN_INVALID",
                &format!("content.frontendExperience.uiQualityContract.referenceProfile.referenceLoadPlan[{index}].path"),
                "referenceLoadPlan entries must include a path.",
            ));
            continue;
        };
        if item.get("refId").and_then(Value::as_str).is_none()
            || item.get("reason").and_then(Value::as_str).is_none()
        {
            issues.push(issue(
                "UI_QUALITY_REFERENCE_LOAD_PLAN_INVALID",
                &format!("content.frontendExperience.uiQualityContract.referenceProfile.referenceLoadPlan[{index}]"),
                "referenceLoadPlan entries must include refId, path, and reason.",
            ));
        }
        actual_paths.insert(path);
    }
    if actual_paths != expected_paths {
        issues.push(issue(
            "UI_QUALITY_REFERENCE_LOAD_PLAN_INVALID",
            "content.frontendExperience.uiQualityContract.referenceProfile.referenceLoadPlan",
            "referenceLoadPlan must exactly match the UIX files implied by referenceProfile.groups.",
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

fn validate_design_token_asset_plan(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(plan) = contract.get("designTokenAssetPlan") else {
        issues.push(issue(
            "UI_QUALITY_TOKEN_ASSET_PLAN_REQUIRED",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan",
            "uiQualityContract requires designTokenAssetPlan so semantic tokens are planned as concrete assets instead of page-local styles.",
        ));
        return;
    };
    if !plan.is_object() {
        issues.push(issue(
            "UI_QUALITY_TOKEN_ASSET_PLAN_INVALID",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan",
            "designTokenAssetPlan must be an object.",
        ));
        return;
    }
    require_string_in(
        plan,
        "/strategy",
        "content.frontendExperience.uiQualityContract.designTokenAssetPlan.strategy",
        &UI_DESIGN_TOKEN_STRATEGIES,
        "UI_QUALITY_TOKEN_ASSET_STRATEGY_INVALID",
        issues,
    );
    require_string_in(
        plan,
        "/mergePolicy",
        "content.frontendExperience.uiQualityContract.designTokenAssetPlan.mergePolicy",
        &UI_DESIGN_TOKEN_MERGE_POLICIES,
        "UI_QUALITY_TOKEN_ASSET_MERGE_POLICY_INVALID",
        issues,
    );
    require_string_in(
        plan,
        "/duplicationPolicy",
        "content.frontendExperience.uiQualityContract.designTokenAssetPlan.duplicationPolicy",
        &UI_DESIGN_TOKEN_DUPLICATION_POLICIES,
        "UI_QUALITY_TOKEN_ASSET_DUPLICATION_POLICY_INVALID",
        issues,
    );
    validate_design_token_template_id(plan, issues);
    validate_design_token_target_files(plan, issues);
    validate_design_token_style_evidence(plan, issues);
}

fn validate_design_token_template_id(plan: &Value, issues: &mut Vec<RepairIssue>) {
    let strategy = plan
        .get("strategy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let template = plan.get("templateId").unwrap_or(&Value::Null);
    match strategy {
        "create_css_tokens" => {
            if template.as_str() != Some("tokens-css") {
                issues.push(issue(
                    "UI_QUALITY_TOKEN_TEMPLATE_INVALID",
                    "content.frontendExperience.uiQualityContract.designTokenAssetPlan.templateId",
                    "create_css_tokens requires templateId=tokens-css.",
                ));
            }
        }
        "create_tailwind_tokens" => {
            if template.as_str() != Some("tokens-tailwind") {
                issues.push(issue(
                    "UI_QUALITY_TOKEN_TEMPLATE_INVALID",
                    "content.frontendExperience.uiQualityContract.designTokenAssetPlan.templateId",
                    "create_tailwind_tokens requires templateId=tokens-tailwind.",
                ));
            }
        }
        "reuse_existing" | "not_applicable" => {
            if !template.is_null() {
                issues.push(issue(
                    "UI_QUALITY_TOKEN_TEMPLATE_INVALID",
                    "content.frontendExperience.uiQualityContract.designTokenAssetPlan.templateId",
                    "reuse_existing and not_applicable require templateId=null.",
                ));
            }
        }
        "extend_existing" => {
            if !(template.is_null()
                || template.as_str().is_some_and(|id| {
                    UI_DESIGN_TOKEN_TEMPLATE_IDS
                        .iter()
                        .any(|known| known == &id)
                }))
            {
                issues.push(issue(
                    "UI_QUALITY_TOKEN_TEMPLATE_INVALID",
                    "content.frontendExperience.uiQualityContract.designTokenAssetPlan.templateId",
                    "extend_existing templateId must be null or a known UIX token template item.",
                ));
            }
        }
        _ => {}
    }
}

fn validate_design_token_target_files(plan: &Value, issues: &mut Vec<RepairIssue>) {
    let strategy = plan
        .get("strategy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(items) = plan.get("targetFiles").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_QUALITY_TOKEN_TARGET_FILES_INVALID",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan.targetFiles",
            "designTokenAssetPlan.targetFiles must be an array.",
        ));
        return;
    };
    if strategy != "not_applicable" && items.is_empty() {
        issues.push(issue(
            "UI_QUALITY_TOKEN_TARGET_FILES_REQUIRED",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan.targetFiles",
            "designTokenAssetPlan.targetFiles must list the token/theme files to reuse, extend, or create.",
        ));
    }
    if strategy == "not_applicable" && !items.is_empty() {
        issues.push(issue(
            "UI_QUALITY_TOKEN_TARGET_FILES_INVALID",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan.targetFiles",
            "not_applicable designTokenAssetPlan must leave targetFiles empty.",
        ));
    }
    for (index, item) in items.iter().enumerate() {
        if item.as_str().map(str::trim).unwrap_or_default().is_empty() {
            issues.push(issue(
                "UI_QUALITY_TOKEN_TARGET_FILES_INVALID",
                &format!("content.frontendExperience.uiQualityContract.designTokenAssetPlan.targetFiles[{index}]"),
                "targetFiles entries must be non-empty project-relative paths.",
            ));
        }
    }
}

fn validate_design_token_style_evidence(plan: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(evidence) = plan.get("existingStyleEvidence") else {
        issues.push(issue(
            "UI_QUALITY_TOKEN_STYLE_EVIDENCE_REQUIRED",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan.existingStyleEvidence",
            "designTokenAssetPlan must include compact existingStyleEvidence so agents know whether to reuse, extend, or create token assets.",
        ));
        return;
    };
    if !evidence.is_object() {
        issues.push(issue(
            "UI_QUALITY_TOKEN_STYLE_EVIDENCE_INVALID",
            "content.frontendExperience.uiQualityContract.designTokenAssetPlan.existingStyleEvidence",
            "existingStyleEvidence must be an object.",
        ));
        return;
    }
    for key in [
        "tailwindConfigRefs",
        "tokenFileRefs",
        "globalStyleRefs",
        "componentThemeRefs",
    ] {
        if !evidence.get(key).is_some_and(Value::is_array) {
            issues.push(issue(
                "UI_QUALITY_TOKEN_STYLE_EVIDENCE_INVALID",
                &format!("content.frontendExperience.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.{key}"),
                "existingStyleEvidence ref fields must be arrays.",
            ));
        }
    }
    require_non_empty_string(
        evidence,
        "/summary",
        "content.frontendExperience.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.summary",
        "UI_QUALITY_TOKEN_STYLE_EVIDENCE_SUMMARY_REQUIRED",
        issues,
    );
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

fn validate_quality_gates(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let scenario = contract
        .pointer("/scenario/kind")
        .and_then(Value::as_str)
        .unwrap_or("custom_product_ui");
    let reference_groups = contract
        .pointer("/referenceProfile/groups")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let design_token_plan = contract
        .get("designTokenAssetPlan")
        .cloned()
        .unwrap_or(Value::Null);
    let expected = ui_quality_gates_for_contract(scenario, &reference_groups, &design_token_plan);
    let expected_gate_ids = expected
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|gate| gate.get("gateId").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let Some(gates) = contract.get("qualityGates").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_QUALITY_GATES_REQUIRED",
            "content.frontendExperience.uiQualityContract.qualityGates",
            "uiQualityContract must include generated qualityGates so selected UIX references become executable and reviewable.",
        ));
        return;
    };
    if gates.is_empty() {
        issues.push(issue(
            "UI_QUALITY_GATES_REQUIRED",
            "content.frontendExperience.uiQualityContract.qualityGates",
            "uiQualityContract.qualityGates must not be empty.",
        ));
        return;
    }
    let mut actual_gate_ids = BTreeSet::new();
    for (index, gate) in gates.iter().enumerate() {
        let Some(gate_id) = gate.get("gateId").and_then(Value::as_str) else {
            issues.push(issue(
                "UI_QUALITY_GATE_INVALID",
                &format!(
                    "content.frontendExperience.uiQualityContract.qualityGates[{index}].gateId"
                ),
                "qualityGates entries must include gateId.",
            ));
            continue;
        };
        actual_gate_ids.insert(gate_id);
        if !expected_gate_ids.contains(gate_id) {
            issues.push(issue(
                "UI_QUALITY_GATE_INVALID",
                &format!("content.frontendExperience.uiQualityContract.qualityGates[{index}].gateId"),
                "qualityGates.gateId must be one of the gates generated from selected scenario, references, stack, and token plan.",
            ));
        }
        if !gate
            .get("sourceRefId")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("uix."))
        {
            issues.push(issue(
                "UI_QUALITY_GATE_INVALID",
                &format!("content.frontendExperience.uiQualityContract.qualityGates[{index}].sourceRefId"),
                "qualityGates.sourceRefId must reference the UIX source rule such as uix.scenarios.admin-dashboard.",
            ));
        }
        let severity = gate.get("severity").and_then(Value::as_str);
        if !severity.is_some_and(|value| UI_QUALITY_GATE_SEVERITIES.contains(&value)) {
            issues.push(issue(
                "UI_QUALITY_GATE_INVALID",
                &format!(
                    "content.frontendExperience.uiQualityContract.qualityGates[{index}].severity"
                ),
                "qualityGates.severity must be a known UI quality gate severity.",
            ));
        }
        for field in ["appliesToSurfaceRoles", "evidenceRequired"] {
            if !gate
                .get(field)
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty() && items.iter().all(Value::is_string))
            {
                issues.push(issue(
                    "UI_QUALITY_GATE_INVALID",
                    &format!("content.frontendExperience.uiQualityContract.qualityGates[{index}].{field}"),
                    "qualityGates array fields must be non-empty string arrays.",
                ));
            }
        }
        if gate
            .get("expectation")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            issues.push(issue(
                "UI_QUALITY_GATE_INVALID",
                &format!("content.frontendExperience.uiQualityContract.qualityGates[{index}].expectation"),
                "qualityGates.expectation must describe the executable UI quality rule.",
            ));
        }
    }
    for expected_gate_id in expected_gate_ids {
        if !actual_gate_ids.contains(expected_gate_id) {
            issues.push(issue(
                "UI_QUALITY_GATE_REQUIRED",
                "content.frontendExperience.uiQualityContract.qualityGates",
                "uiQualityContract.qualityGates must include every gate generated from selected scenario, references, stack, and token plan.",
            ));
            break;
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

    use super::{
        build_ui_quality_seed, known_ui_reference_groups, scenario_supporting_reference_items,
        UI_CORE_REFERENCE_ITEMS, UI_DESIGN_TOKEN_TEMPLATE_IDS, UI_TOKEN_REFERENCE_ITEMS,
    };
    use crate::{
        ConfidenceLevel, ProjectKind, TechnicalBaselineApproval, TechnicalBaselineApprovalType,
        TechnicalBaselineContract, TechnicalBaselineScope, TechnicalBaselineSource,
        TechnicalBaselineStatus,
    };

    #[test]
    fn known_ui_reference_groups_resolve_to_shared_reference_files() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for (group, item) in known_group_items() {
            let relative = reference_file_for_item(&group, &item);
            let path = repo_root
                .join("plugins/shared/loom/references/uix")
                .join(relative);
            assert!(
                path.exists(),
                "UIX reference {group}.{item} must resolve to {}",
                path.display()
            );
        }
    }

    #[test]
    fn ui_quality_seed_includes_all_core_reference_items() {
        let seed = build_ui_quality_seed(None, None);
        let reference_groups = seed
            .get("requiredReferenceGroups")
            .expect("seed must include requiredReferenceGroups");

        for reference_item in UI_CORE_REFERENCE_ITEMS {
            assert!(
                group_contains(reference_groups, "core", reference_item),
                "uiQualitySeed.requiredReferenceGroups.core must include {reference_item}"
            );
        }
        for reference_item in UI_TOKEN_REFERENCE_ITEMS {
            assert!(
                group_contains(reference_groups, "tokens", reference_item),
                "uiQualitySeed.requiredReferenceGroups.tokens must include {reference_item}"
            );
        }
    }

    #[test]
    fn ui_quality_scenario_supporting_refs_cover_admin_data_and_mobile() {
        let reference_items = scenario_supporting_reference_items("admin_dashboard")
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        for reference_item in [
            ("scenarios", "admin-dashboard"),
            ("scenarios", "data-console"),
            ("focus", "data"),
            ("focus", "mobile"),
        ] {
            assert!(
                reference_items.contains(&reference_item),
                "admin dashboard supporting refs must include {reference_item:?}"
            );
        }
    }

    #[test]
    fn ui_quality_seed_adds_frameworks_when_stack_reference_exists() {
        let baseline = TechnicalBaselineContract {
            schema_version: "1.0".to_string(),
            technical_baseline_id: "tbr-test".to_string(),
            delivery_id: "delivery-test".to_string(),
            phase_id: "phase-test".to_string(),
            status: TechnicalBaselineStatus::Confirmed,
            source: TechnicalBaselineSource::UserConfirmed,
            project_kind: ProjectKind::NewProject,
            scope: TechnicalBaselineScope::Project,
            stack: serde_json::json!("React + Tailwind"),
            constraints: vec![],
            evidence: vec![],
            approval: TechnicalBaselineApproval {
                r#type: TechnicalBaselineApprovalType::UserConfirmed,
                confirmed_at: Some("2026-07-02T00:00:00Z".to_string()),
                reason: Some("test".to_string()),
            },
            confidence: ConfidenceLevel::High,
            requires_user_confirmation: Some(false),
            reasoning_summary: vec![],
            alternatives: vec![],
            created_at: "2026-07-02T00:00:00Z".to_string(),
            updated_at: "2026-07-02T00:00:00Z".to_string(),
        };
        let seed = build_ui_quality_seed(None, Some(&baseline));
        let reference_groups = seed
            .get("requiredReferenceGroups")
            .expect("seed must include requiredReferenceGroups");

        assert!(
            group_contains(reference_groups, "focus", "frameworks"),
            "stack-aware UI seed must include focus.frameworks"
        );
        assert!(
            group_contains(reference_groups, "stacks", "react"),
            "stack-aware UI seed must include stacks.react"
        );
    }

    #[test]
    fn known_ui_token_template_ids_resolve_to_shared_template_files() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for template_id in UI_DESIGN_TOKEN_TEMPLATE_IDS {
            let relative = template_file_for_id(template_id);
            let path = repo_root
                .join("plugins/shared/loom/references/uix")
                .join(relative);
            assert!(
                path.exists(),
                "UIX template id {template_id} must resolve to {}",
                path.display()
            );
        }
    }

    #[test]
    fn focused_uix_references_keep_operational_depth() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for (group, item) in known_group_items() {
            if !matches!(group.as_str(), "scenarios" | "tokens" | "stacks") {
                continue;
            }
            let path = repo_root
                .join("plugins/shared/loom/references/uix")
                .join(reference_file_for_item(&group, &item));
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            let line_count = content.lines().count();
            assert!(
                line_count >= 40,
                "UIX reference {group}.{item} is too thin ({line_count} lines)"
            );
            assert!(
                content.contains("```"),
                "UIX reference {group}.{item} should include a concrete structure or token example"
            );
        }
    }

    fn known_group_items() -> Vec<(String, String)> {
        let groups = known_ui_reference_groups();
        groups
            .as_object()
            .into_iter()
            .flat_map(|object| object.iter())
            .flat_map(|(group, items)| {
                items
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str())
                    .map(|item| (group.clone(), item.to_string()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn group_contains(groups: &serde_json::Value, group: &str, item: &str) -> bool {
        groups
            .get(group)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(item))
    }

    fn reference_file_for_item(group: &str, item: &str) -> String {
        match group {
            "core" if item == "core" => "core.md".to_string(),
            "core" if item == "anti-patterns" => "anti-patterns.md".to_string(),
            "core" => format!("{item}.md"),
            "focus" => format!("{item}.md"),
            "tokens" => format!("tokens/{item}.md"),
            "scenarios" => format!("scenarios/{item}.md"),
            "stacks" => format!("stacks/{item}.md"),
            "templates" => template_file_for_id(item).to_string(),
            _ => panic!("unknown UIX reference group item: {group}.{item}"),
        }
    }

    fn template_file_for_id(template_id: &str) -> &'static str {
        match template_id {
            "tokens-css" => "templates/tokens.css.tpl",
            "tokens-tailwind" => "templates/tokens.tailwind.tpl",
            _ => panic!("unknown UIX token template id: {template_id}"),
        }
    }
}
