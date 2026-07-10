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

pub const UI_QUALITY_RULE_SEVERITIES: [&str; 2] = ["must", "should"];

pub const UI_QUALITY_RULE_STATUSES: [&str; 5] = [
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

pub const UI_CORE_REFERENCE_ITEMS: [&str; 7] = [
    "core",
    "surface-decision",
    "anti-patterns",
    "system",
    "interaction",
    "content",
    "verification",
];

pub const UI_FOCUS_REFERENCE_ITEMS: [&str; 4] =
    ["data", "mobile", "frameworks", "web-implementation"];

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

pub const UI_SURFACE_PATTERN_MODES: [&str; 3] = ["known", "hybrid", "custom"];

pub const UI_KNOWN_SURFACE_PATTERNS: [&str; 12] = [
    "collection_workbench",
    "decision_queue",
    "form_flow",
    "settings_console",
    "analytics_monitor",
    "editor_workspace",
    "support_inbox",
    "developer_console",
    "content_page",
    "marketing_surface",
    "immersive_workspace",
    "mobile_task_flow",
];

pub const UI_SURFACE_CONFIDENCE_LEVELS: [&str; 3] = ["high", "medium", "low"];

pub const UI_USER_JOB_KINDS: [&str; 11] = [
    "browse",
    "search",
    "compare",
    "create",
    "edit",
    "review",
    "decide",
    "monitor",
    "configure",
    "communicate",
    "consume_content",
];

pub const UI_INFORMATION_SHAPES: [&str; 12] = [
    "single_object",
    "record_collection",
    "record_detail",
    "form_fields",
    "hierarchy_tree",
    "timeline",
    "metric_series",
    "chart_set",
    "document_content",
    "conversation",
    "canvas_scene",
    "media_grid",
];

pub const UI_OPERATION_MODELS: [&str; 11] = [
    "read_only",
    "filter_sort_paginate",
    "create_update",
    "approve_reject",
    "batch_action",
    "wizard_step_flow",
    "configuration_save",
    "search_analysis",
    "alert_triage",
    "content_authoring",
    "runtime_operation",
];

pub const UI_RISK_FACTORS: [&str; 8] = [
    "none",
    "destructive",
    "irreversible",
    "financial",
    "privacy",
    "compliance",
    "audit_relevant",
    "business_blocking",
];

pub const UI_NAVIGATION_MODELS: [&str; 7] = [
    "single_surface",
    "module_shell",
    "tabs",
    "master_detail",
    "stepper",
    "drill_down",
    "canvas_panels",
];

pub const UI_DEVICE_POSTURES: [&str; 5] = [
    "desktop_primary",
    "mobile_primary",
    "responsive_web",
    "native_mobile",
    "immersive_canvas",
];

pub const UI_PRODUCT_MODES: [&str; 6] = [
    "internal_business_product",
    "developer_tool",
    "consumer_product",
    "content_product",
    "marketing_or_brand",
    "immersive_product",
];

pub const UI_REGION_ROLES: [&str; 14] = [
    "app_shell",
    "navigation",
    "topbar",
    "primary_work_region",
    "record_results",
    "record_detail",
    "form",
    "action_panel",
    "feedback_area",
    "inspector",
    "editor",
    "preview",
    "metrics",
    "content_body",
];

pub const UI_PRESENTATION_KINDS: [&str; 13] = [
    "table",
    "dense_grid",
    "record_cards",
    "form_sections",
    "detail_panel",
    "drawer",
    "route_detail",
    "chart_panel",
    "timeline",
    "tree",
    "canvas",
    "document",
    "conversation_thread",
];

pub const UI_COMPOSITION_CONSTRAINT_KINDS: [&str; 9] = [
    "no_marketing_hero",
    "no_feature_explainer_wall",
    "no_large_intro_panel",
    "no_decorative_filler_before_workflow",
    "no_card_only_desktop_record_list_when_comparison_required",
    "no_unwired_static_workflow",
    "no_global_only_feedback",
    "no_internal_process_copy",
    "no_inaccessible_primary_action",
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
        "qualityRuleSeverity": UI_QUALITY_RULE_SEVERITIES,
        "qualityRuleStatus": UI_QUALITY_RULE_STATUSES,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiState": UI_REQUIRED_STATES,
        "knownReferenceGroups": known_ui_reference_groups()
    })
}

pub fn ui_surface_decision_enum_refs() -> Value {
    json!({
        "patternMode": UI_SURFACE_PATTERN_MODES,
        "knownPattern": UI_KNOWN_SURFACE_PATTERNS,
        "confidence": UI_SURFACE_CONFIDENCE_LEVELS,
        "userJob": UI_USER_JOB_KINDS,
        "informationShape": UI_INFORMATION_SHAPES,
        "operationModel": UI_OPERATION_MODELS,
        "riskFactor": UI_RISK_FACTORS,
        "navigationModel": UI_NAVIGATION_MODELS,
        "devicePosture": UI_DEVICE_POSTURES,
        "productMode": UI_PRODUCT_MODES,
        "regionRole": UI_REGION_ROLES,
        "presentationKind": UI_PRESENTATION_KINDS,
        "compositionConstraint": UI_COMPOSITION_CONSTRAINT_KINDS,
        "requiredUiState": UI_REQUIRED_STATES
    })
}

pub fn ui_surface_decision_candidate_shape() -> Value {
    json!({
        "patternRankings": [{
            "kind": UI_KNOWN_SURFACE_PATTERNS.join(" | "),
            "score": "0.0-1.0",
            "matchedSignals": ["string"],
            "missingSignals": ["string"],
            "mismatchSignals": ["string"],
            "evidenceRefs": ["frontend dataView/action/workflow/interface/detail refs"]
        }],
        "selectedPattern": {
            "mode": UI_SURFACE_PATTERN_MODES.join(" | "),
            "knownPattern": "known pattern when mode=known, otherwise null",
            "primaryKnownPattern": "known pattern when mode=hybrid, otherwise null",
            "secondaryKnownPatterns": ["known patterns when mode=hybrid"],
            "customPattern": "agent-defined short pattern name when mode=custom, otherwise null",
            "nearestKnownPatterns": ["required when mode=custom"],
            "confidence": UI_SURFACE_CONFIDENCE_LEVELS.join(" | "),
            "rationale": "string",
            "evidenceRefs": ["refs proving the selected pattern"]
        },
        "semanticFacts": ui_surface_semantic_facts_shape(),
        "layoutModel": ui_surface_layout_model_shape(),
        "regionModel": ui_surface_region_model_shape(),
        "informationModel": ui_surface_information_model_shape(),
        "actionModel": ui_surface_action_model_shape(),
        "stateModel": ui_surface_state_model_shape(),
        "compositionConstraints": ui_surface_composition_constraints_shape(),
        "contentBoundary": ui_surface_content_boundary_shape()
    })
}

pub fn ui_surface_decision_candidate_template() -> Value {
    json!({
        "patternRankings": [{
            "kind": "collection_workbench",
            "score": 0.0,
            "matchedSignals": [],
            "missingSignals": [],
            "mismatchSignals": [],
            "evidenceRefs": []
        }],
        "selectedPattern": {
            "mode": "known",
            "knownPattern": "collection_workbench",
            "primaryKnownPattern": null,
            "secondaryKnownPatterns": [],
            "customPattern": null,
            "nearestKnownPatterns": [],
            "confidence": "medium",
            "rationale": "",
            "evidenceRefs": []
        },
        "semanticFacts": {
            "userJobs": [],
            "informationShapes": [],
            "operationModels": [],
            "riskFactors": [],
            "navigationModel": "single_surface",
            "devicePosture": "responsive_web",
            "productMode": "internal_business_product",
            "customExtensions": {
                "userJobs": [],
                "informationShapes": [],
                "operationModels": [],
                "riskFactors": [],
                "navigationModel": "",
                "devicePosture": "",
                "productMode": ""
            },
            "evidenceRefs": []
        },
        "layoutModel": {
            "layoutBaseline": "custom_product_layout",
            "density": "balanced",
            "primaryWorkRegionId": "region_primary",
            "desktop": {
                "layoutIntent": "",
                "allowedPresentations": [],
                "forbiddenPresentations": []
            },
            "tablet": {
                "layoutIntent": "",
                "allowedPresentations": []
            },
            "mobile": {
                "layoutIntent": "",
                "allowedPresentations": []
            },
            "customLayoutIntent": ""
        },
        "regionModel": [{
            "regionId": "region_primary",
            "role": "primary_work_region",
            "purpose": "",
            "desktopPlacement": "",
            "mobilePlacement": "",
            "requiredContent": [],
            "forbiddenContent": [],
            "dataViewRefs": ["view_1"],
            "actionRefs": ["action_1"],
            "stateRefs": ["loading", "success", "error", "empty", "business_blocking"],
            "evidenceRefs": []
        }],
        "informationModel": {
            "primaryObjects": [],
            "fields": [],
            "identityFields": [],
            "statusFields": [],
            "scanOrder": [],
            "comparisonNeed": "none",
            "detailNeed": "none",
            "longContentPolicy": ""
        },
        "actionModel": [{
            "actionId": "action_1",
            "kind": "create_update",
            "label": "",
            "riskFactors": [],
            "placementRegionId": "region_primary",
            "pendingFeedback": "",
            "successFeedback": "",
            "errorFeedback": "",
            "businessBlockingFeedback": "",
            "postSuccessUpdate": "",
            "evidenceRefs": []
        }],
        "stateModel": [{
            "state": "loading",
            "placementRegionId": "region_primary",
            "placementRule": "",
            "recoveryPath": "",
            "evidenceRefs": []
        }],
        "compositionConstraints": {
            "requiredComposition": [],
            "forbiddenComposition": [],
            "antiDemoRules": [],
            "customRules": []
        },
        "contentBoundary": {
            "allowedUserVisibleContent": [
                "labels",
                "filters",
                "status",
                "actions",
                "validation",
                "business_feedback"
            ],
            "forbiddenUserVisibleContent": [],
            "customForbiddenContent": [],
            "copyRule": "Use product language for the user task; do not show delivery, runtime, stack, validator, or generated artifact language unless the product mode requires it."
        }
    })
}

pub fn ui_surface_decision_contract_shape() -> Value {
    json!({
        "schemaVersion": "1.0",
        "contractKind": "ui_surface_decision_contract",
        "patternDecision": {
            "mode": UI_SURFACE_PATTERN_MODES.join(" | "),
            "knownPattern": "known pattern when mode=known, otherwise null",
            "primaryKnownPattern": "known pattern when mode=hybrid, otherwise null",
            "secondaryKnownPatterns": ["known pattern"],
            "customPattern": "custom pattern name when mode=custom, otherwise null",
            "nearestKnownPatterns": ["known pattern"],
            "confidence": UI_SURFACE_CONFIDENCE_LEVELS.join(" | "),
            "rationale": "string",
            "evidenceRefs": ["refs proving the selected pattern"],
            "rankings": [{
                "kind": UI_KNOWN_SURFACE_PATTERNS.join(" | "),
                "score": "0.0-1.0",
                "matchedSignals": ["string"],
                "missingSignals": ["string"],
                "mismatchSignals": ["string"],
                "evidenceRefs": ["string"]
            }]
        },
        "semanticFacts": ui_surface_semantic_facts_shape(),
        "layoutModel": ui_surface_layout_model_shape(),
        "regionModel": ui_surface_region_model_shape(),
        "informationModel": ui_surface_information_model_shape(),
        "actionModel": ui_surface_action_model_shape(),
        "stateModel": ui_surface_state_model_shape(),
        "compositionConstraints": ui_surface_composition_constraints_shape(),
        "contentBoundary": ui_surface_content_boundary_shape(),
        "semanticTokenPolicy": UI_SEMANTIC_TOKEN_POLICIES.join(" | "),
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
        "referencePlan": [{
            "refId": "uix.core.core",
            "path": "uix/core.md",
            "reason": "MCP-selected UIX reference for this decision contract."
        }],
        "qualityRules": [{
            "ruleId": "string",
            "severity": UI_QUALITY_RULE_SEVERITIES.join(" | "),
            "appliesToRegionIds": ["region id"],
            "appliesToActionIds": ["action id"],
            "appliesToStateKinds": [UI_REQUIRED_STATES.join(" | ")],
            "expectation": "executable quality expectation",
            "evidenceRequired": ["region_evidence | action_evidence | state_evidence | responsive_evidence | composition_evidence | rendered_evidence | source_evidence"]
        }]
    })
}

fn ui_surface_semantic_facts_shape() -> Value {
    json!({
        "userJobs": [UI_USER_JOB_KINDS.join(" | ")],
        "informationShapes": [UI_INFORMATION_SHAPES.join(" | ")],
        "operationModels": [UI_OPERATION_MODELS.join(" | ")],
        "riskFactors": [UI_RISK_FACTORS.join(" | ")],
        "navigationModel": UI_NAVIGATION_MODELS.join(" | "),
        "devicePosture": UI_DEVICE_POSTURES.join(" | "),
        "productMode": UI_PRODUCT_MODES.join(" | "),
        "customExtensions": {
            "userJobs": ["required when user job exceeds enum"],
            "informationShapes": ["required when information shape exceeds enum"],
            "operationModels": ["required when operation model exceeds enum"],
            "riskFactors": ["required when risk factor exceeds enum"],
            "navigationModel": "custom navigation model when enum does not fit",
            "devicePosture": "custom device posture when enum does not fit",
            "productMode": "custom product mode when enum does not fit"
        },
        "evidenceRefs": ["refs supporting semantic facts"]
    })
}

fn ui_surface_layout_model_shape() -> Value {
    json!({
        "layoutBaseline": UI_LAYOUT_BASELINES.join(" | "),
        "density": UI_DENSITIES.join(" | "),
        "primaryWorkRegionId": "region id",
        "desktop": {
            "layoutIntent": "string",
            "allowedPresentations": [UI_PRESENTATION_KINDS.join(" | ")],
            "forbiddenPresentations": [UI_PRESENTATION_KINDS.join(" | composition constraint")]
        },
        "tablet": {
            "layoutIntent": "string",
            "allowedPresentations": [UI_PRESENTATION_KINDS.join(" | ")]
        },
        "mobile": {
            "layoutIntent": "string",
            "allowedPresentations": [UI_PRESENTATION_KINDS.join(" | ")]
        },
        "customLayoutIntent": "required when known presentation kinds do not fit"
    })
}

fn ui_surface_region_model_shape() -> Value {
    json!([{
        "regionId": "string",
        "role": UI_REGION_ROLES.join(" | "),
        "purpose": "string",
        "desktopPlacement": "string",
        "mobilePlacement": "string",
        "requiredContent": ["string"],
        "forbiddenContent": ["string"],
        "dataViewRefs": ["string"],
        "actionRefs": ["string"],
        "stateRefs": [UI_REQUIRED_STATES.join(" | ")],
        "evidenceRefs": ["string"]
    }])
}

fn ui_surface_information_model_shape() -> Value {
    json!({
        "primaryObjects": ["business object name"],
        "fields": ["business field or display field"],
        "identityFields": ["field"],
        "statusFields": ["field"],
        "scanOrder": ["identity | status | decision field | action | custom item"],
        "comparisonNeed": "none | row_comparison | side_by_side | trend_comparison | custom",
        "detailNeed": "none | side_panel | drawer | route | inline_expansion | custom",
        "longContentPolicy": "string"
    })
}

fn ui_surface_action_model_shape() -> Value {
    json!([{
        "actionId": "string",
        "kind": UI_OPERATION_MODELS.join(" | "),
        "label": "business action label",
        "riskFactors": [UI_RISK_FACTORS.join(" | ")],
        "placementRegionId": "region id",
        "pendingFeedback": "string",
        "successFeedback": "string",
        "errorFeedback": "string",
        "businessBlockingFeedback": "string",
        "postSuccessUpdate": "string",
        "evidenceRefs": ["string"]
    }])
}

fn ui_surface_state_model_shape() -> Value {
    json!([{
        "state": UI_REQUIRED_STATES.join(" | validation | disabled | stale"),
        "placementRegionId": "region id",
        "placementRule": "string",
        "recoveryPath": "string",
        "evidenceRefs": ["string"]
    }])
}

fn ui_surface_composition_constraints_shape() -> Value {
    json!({
        "requiredComposition": ["string"],
        "forbiddenComposition": [UI_COMPOSITION_CONSTRAINT_KINDS.join(" | custom composition constraint")],
        "antiDemoRules": [UI_COMPOSITION_CONSTRAINT_KINDS.join(" | custom anti-demo rule")],
        "customRules": ["required when known constraints do not cover the product surface"]
    })
}

fn ui_surface_content_boundary_shape() -> Value {
    json!({
        "allowedUserVisibleContent": [
            "labels",
            "filters",
            "status",
            "actions",
            "validation",
            "business_feedback",
            "help_entry"
        ],
        "forbiddenUserVisibleContent": [UI_FORBIDDEN_USER_VISIBLE_CONTENT.join(" | feature_explanation | custom forbidden copy class")],
        "customForbiddenContent": ["required when product-specific content must be blocked"],
        "copyRule": "Use product language for the user task; do not show delivery, runtime, stack, validator, or generated artifact language unless the product mode requires it."
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
    let quality_rule_preview = ui_quality_rule_preview(
        primary_scenario,
        &required_reference_groups,
        &design_token_seed,
    );
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
        "qualityRulePreview": quality_rule_preview,
        "stackReferenceCandidates": stack_items,
        "designTokenAssetPlan": design_token_seed,
        "forbiddenUserVisibleContent": UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "requiredUiStates": UI_REQUIRED_STATES,
        "selectionRule": "Use these candidates only as hints while writing surfaceDecisionCandidate. Do not write referenceProfile, referenceLoadPlan, or derived rule lists inside the candidate; MCP derives uiSurfaceDecisionContract.qualityRules during submit."
    })
}

pub fn normalize_ui_surface_decision_contract_for_persist(
    frontend_experience: &mut Value,
    ui_quality_seed: &Value,
) -> bool {
    let required = frontend_experience
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !required {
        return false;
    }
    let Some(candidate) = frontend_experience
        .get("surfaceDecisionCandidate")
        .cloned()
        .filter(Value::is_object)
    else {
        return false;
    };

    let design_token_plan = frontend_experience
        .pointer("/surfaceDecisionCandidate/designTokenAssetPlan")
        .cloned()
        .or_else(|| ui_quality_seed.get("designTokenAssetPlan").cloned())
        .unwrap_or_else(default_design_token_asset_plan);
    let semantic_token_policy = ui_quality_seed
        .get("semanticTokenPolicy")
        .cloned()
        .unwrap_or_else(|| json!("semantic_tokens_required"));
    let pattern_decision = normalized_surface_pattern_decision(&candidate);
    let scenario = ui_scenario_for_surface_decision(&pattern_decision, ui_quality_seed);
    let stack_items = ui_quality_seed
        .get("stackReferenceCandidates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let reference_groups = required_reference_groups(&scenario, &stack_items, &design_token_plan);
    let reference_plan = ui_reference_load_plan(&reference_groups);
    let region_model = candidate_model_or_template(&candidate, "regionModel");
    let action_model = candidate_model_or_template(&candidate, "actionModel");
    let quality_rules = ui_quality_rules_for_contract(
        &scenario,
        &reference_groups,
        &design_token_plan,
        &region_model,
        &action_model,
    );
    let layout_model = normalized_surface_layout_model(&candidate, &scenario);
    let contract = json!({
        "schemaVersion": "1.0",
        "contractKind": "ui_surface_decision_contract",
        "patternDecision": pattern_decision,
        "semanticFacts": candidate_model_or_template(&candidate, "semanticFacts"),
        "layoutModel": layout_model,
        "regionModel": region_model,
        "informationModel": candidate_model_or_template(&candidate, "informationModel"),
        "actionModel": action_model,
        "stateModel": candidate_model_or_template(&candidate, "stateModel"),
        "compositionConstraints": normalized_surface_composition_constraints(&candidate),
        "contentBoundary": normalized_surface_content_boundary(&candidate),
        "semanticTokenPolicy": semantic_token_policy,
        "designTokenAssetPlan": design_token_plan,
        "referencePlan": reference_plan,
        "qualityRules": quality_rules
    });

    let Some(object) = frontend_experience.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    set_value_if_changed(object, "uiSurfaceDecisionContract", contract, &mut changed);
    changed
}

fn normalized_surface_pattern_decision(candidate: &Value) -> Value {
    let template = ui_surface_decision_candidate_template();
    let selected = candidate
        .get("selectedPattern")
        .filter(|value| value.is_object())
        .unwrap_or_else(|| {
            template
                .get("selectedPattern")
                .expect("template selectedPattern")
        });
    json!({
        "mode": selected.get("mode").cloned().unwrap_or_else(|| json!("custom")),
        "knownPattern": selected.get("knownPattern").cloned().unwrap_or(Value::Null),
        "primaryKnownPattern": selected.get("primaryKnownPattern").cloned().unwrap_or(Value::Null),
        "secondaryKnownPatterns": selected.get("secondaryKnownPatterns").cloned().unwrap_or_else(|| json!([])),
        "customPattern": selected.get("customPattern").cloned().unwrap_or(Value::Null),
        "nearestKnownPatterns": selected.get("nearestKnownPatterns").cloned().unwrap_or_else(|| json!([])),
        "confidence": selected.get("confidence").cloned().unwrap_or_else(|| json!("low")),
        "rationale": selected.get("rationale").cloned().unwrap_or_else(|| json!("")),
        "evidenceRefs": selected.get("evidenceRefs").cloned().unwrap_or_else(|| json!([])),
        "rankings": candidate.get("patternRankings").cloned().unwrap_or_else(|| json!([]))
    })
}

fn candidate_model_or_template(candidate: &Value, key: &str) -> Value {
    candidate
        .get(key)
        .cloned()
        .filter(|value| value.is_object() || value.is_array())
        .or_else(|| ui_surface_decision_candidate_template().get(key).cloned())
        .unwrap_or(Value::Null)
}

fn normalized_surface_layout_model(candidate: &Value, scenario: &str) -> Value {
    let mut value = candidate_model_or_template(candidate, "layoutModel");
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    if object
        .get("layoutBaseline")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        object.insert(
            "layoutBaseline".to_string(),
            json!(derive_layout_baseline(candidate, scenario)),
        );
    }
    if object
        .get("density")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        object.insert("density".to_string(), json!("balanced"));
    }
    value
}

fn derive_layout_baseline(candidate: &Value, scenario: &str) -> &'static str {
    let shell = candidate
        .pointer("/layoutModel/shell")
        .and_then(Value::as_str)
        .or_else(|| {
            candidate
                .pointer("/layoutModel/desktop/layoutIntent")
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    if shell.contains("sidebar") && shell.contains("topbar") {
        return "sidebar_topbar_table_detail";
    }
    if shell.contains("console") {
        return "data_console";
    }
    if shell.contains("mobile") {
        return "mobile_task_flow";
    }
    layout_baseline_candidates(scenario)
        .into_iter()
        .next()
        .unwrap_or("custom_product_layout")
}

fn normalized_surface_composition_constraints(candidate: &Value) -> Value {
    let mut value = candidate_model_or_template(candidate, "compositionConstraints");
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    append_unique_strings(
        object,
        "antiDemoRules",
        &[
            "no_internal_process_copy",
            "no_feature_explainer_wall",
            "no_decorative_filler_before_workflow",
            "no_unwired_static_workflow",
            "no_global_only_feedback",
        ],
    );
    append_unique_strings(
        object,
        "forbiddenComposition",
        &[
            "no_large_intro_panel",
            "no_decorative_filler_before_workflow",
            "no_inaccessible_primary_action",
        ],
    );
    value
}

fn normalized_surface_content_boundary(candidate: &Value) -> Value {
    let mut value = candidate_model_or_template(candidate, "contentBoundary");
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    append_unique_strings(
        object,
        "forbiddenUserVisibleContent",
        &UI_FORBIDDEN_USER_VISIBLE_CONTENT,
    );
    if object
        .get("copyRule")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        object.insert(
            "copyRule".to_string(),
            json!("Use product language for the user task; do not show delivery, runtime, stack, validator, or generated artifact language unless the product mode requires it."),
        );
    }
    value
}

fn append_unique_strings(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    additions: &[&str],
) {
    let mut items = object
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for addition in additions {
        items.insert((*addition).to_string());
    }
    object.insert(
        key.to_string(),
        Value::Array(items.into_iter().map(Value::String).collect()),
    );
}

fn ui_scenario_for_surface_decision(pattern_decision: &Value, ui_quality_seed: &Value) -> String {
    let mode = pattern_decision
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("custom");
    let pattern = match mode {
        "known" => pattern_decision.get("knownPattern").and_then(Value::as_str),
        "hybrid" => pattern_decision
            .get("primaryKnownPattern")
            .and_then(Value::as_str),
        "custom" => pattern_decision
            .get("nearestKnownPatterns")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        _ => None,
    };
    let scenario = pattern
        .and_then(ui_scenario_for_surface_pattern)
        .or_else(|| {
            ui_quality_seed
                .pointer("/scenarioCandidates/0/kind")
                .and_then(Value::as_str)
        });
    scenario.unwrap_or("custom_product_ui").to_string()
}

fn ui_scenario_for_surface_pattern(pattern: &str) -> Option<&'static str> {
    match pattern {
        "collection_workbench"
        | "decision_queue"
        | "form_flow"
        | "settings_console"
        | "support_inbox" => Some("admin_dashboard"),
        "analytics_monitor" => Some("data_console"),
        "developer_console" => Some("developer_tool"),
        "content_page" => Some("docs_site"),
        "marketing_surface" => Some("marketing_site"),
        "immersive_workspace" => Some("immersive_3d"),
        "mobile_task_flow" => Some("mobile_responsive"),
        "editor_workspace" => Some("custom_product_ui"),
        _ => None,
    }
}

fn ui_quality_rules_for_contract(
    scenario: &str,
    reference_groups: &Value,
    design_token_plan: &Value,
    region_model: &Value,
    action_model: &Value,
) -> Value {
    let region_ids = model_string_ids(region_model, "regionId");
    let action_ids = model_string_ids(action_model, "actionId");
    Value::Array(
        ui_quality_rule_specs(scenario, reference_groups, design_token_plan)
            .into_iter()
            .filter_map(|rule| {
                let rule_id = rule.get("ruleId").and_then(Value::as_str)?;
                Some(json!({
                    "ruleId": rule_id,
                    "sourceRefId": rule.get("sourceRefId").cloned().unwrap_or(Value::Null),
                    "severity": rule.get("severity").cloned().unwrap_or_else(|| json!("must")),
                    "appliesToRegionIds": region_ids.clone(),
                    "appliesToActionIds": action_ids.clone(),
                    "appliesToStateKinds": UI_REQUIRED_STATES,
                    "expectation": rule.get("expectation").cloned().unwrap_or_else(|| json!("")),
                    "evidenceRequired": rule.get("evidenceRequired").cloned().unwrap_or_else(|| json!([]))
                }))
            })
            .collect(),
    )
}

fn model_string_ids(model: &Value, key: &str) -> Vec<String> {
    model
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn set_value_if_changed(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Value,
    changed: &mut bool,
) {
    if object.get(key) != Some(&value) {
        object.insert(key.to_string(), value);
        *changed = true;
    }
}

pub fn validate_ui_surface_decision_contract(frontend_experience: &Value) -> Vec<RepairIssue> {
    let mut issues = Vec::new();
    let required = frontend_experience
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !required {
        return issues;
    }
    if !frontend_experience
        .get("surfaceDecisionCandidate")
        .is_some_and(Value::is_object)
    {
        issues.push(issue(
            "UI_SURFACE_DECISION_CANDIDATE_REQUIRED",
            "content.frontendExperience.surfaceDecisionCandidate",
            "frontendExperience.required=true requires a structured surfaceDecisionCandidate so MCP can derive the authoritative UI surface contract.",
        ));
        return issues;
    }
    let Some(contract) = frontend_experience
        .get("uiSurfaceDecisionContract")
        .filter(|value| value.is_object())
    else {
        issues.push(issue(
            "UI_SURFACE_DECISION_CONTRACT_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract",
            "MCP must derive uiSurfaceDecisionContract from surfaceDecisionCandidate before accepting frontend_experience.",
        ));
        return issues;
    };
    require_string_in(
        contract,
        "/patternDecision/mode",
        "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.mode",
        &UI_SURFACE_PATTERN_MODES,
        "UI_SURFACE_PATTERN_MODE_INVALID",
        &mut issues,
    );
    validate_surface_pattern_decision(contract, &mut issues);
    validate_surface_semantic_facts(contract, &mut issues);
    validate_surface_regions_actions_states(contract, &mut issues);
    validate_surface_content_boundary(contract, &mut issues);
    require_string_in(
        contract,
        "/semanticTokenPolicy",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticTokenPolicy",
        &UI_SEMANTIC_TOKEN_POLICIES,
        "UI_SURFACE_SEMANTIC_TOKEN_POLICY_INVALID",
        &mut issues,
    );
    validate_surface_reference_plan(contract, &mut issues);
    validate_surface_quality_rules(contract, &mut issues);
    issues
}

fn validate_surface_pattern_decision(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(pattern) = contract.get("patternDecision") else {
        issues.push(issue(
            "UI_SURFACE_PATTERN_DECISION_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.patternDecision",
            "uiSurfaceDecisionContract requires patternDecision.",
        ));
        return;
    };
    require_string_in(
        pattern,
        "/confidence",
        "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.confidence",
        &UI_SURFACE_CONFIDENCE_LEVELS,
        "UI_SURFACE_CONFIDENCE_INVALID",
        issues,
    );
    require_non_empty_string(
        pattern,
        "/rationale",
        "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.rationale",
        "UI_SURFACE_PATTERN_RATIONALE_REQUIRED",
        issues,
    );
    match pattern
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "known" => require_string_in(
            pattern,
            "/knownPattern",
            "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.knownPattern",
            &UI_KNOWN_SURFACE_PATTERNS,
            "UI_SURFACE_KNOWN_PATTERN_INVALID",
            issues,
        ),
        "hybrid" => {
            require_string_in(
                pattern,
                "/primaryKnownPattern",
                "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.primaryKnownPattern",
                &UI_KNOWN_SURFACE_PATTERNS,
                "UI_SURFACE_PRIMARY_PATTERN_INVALID",
                issues,
            );
            validate_known_pattern_array(
                pattern,
                "/secondaryKnownPatterns",
                "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.secondaryKnownPatterns",
                "UI_SURFACE_SECONDARY_PATTERNS_INVALID",
                issues,
            );
        }
        "custom" => {
            require_non_empty_string(
                pattern,
                "/customPattern",
                "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.customPattern",
                "UI_SURFACE_CUSTOM_PATTERN_REQUIRED",
                issues,
            );
            validate_known_pattern_array(
                pattern,
                "/nearestKnownPatterns",
                "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.nearestKnownPatterns",
                "UI_SURFACE_NEAREST_PATTERNS_INVALID",
                issues,
            );
        }
        _ => {}
    }
    if !pattern
        .get("rankings")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        issues.push(issue(
            "UI_SURFACE_PATTERN_RANKINGS_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.patternDecision.rankings",
            "patternDecision.rankings must record at least one considered known pattern with evidence.",
        ));
    }
}

fn validate_known_pattern_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(items) = root.pointer(pointer).and_then(Value::as_array) else {
        issues.push(issue(code, field_path, "field must be an array."));
        return;
    };
    if items.is_empty() {
        issues.push(issue(
            code,
            field_path,
            "field must include at least one known pattern.",
        ));
        return;
    }
    for item in items {
        if !item
            .as_str()
            .is_some_and(|value| UI_KNOWN_SURFACE_PATTERNS.contains(&value))
        {
            issues.push(issue(
                code,
                field_path,
                "field must contain only known UI surface patterns.",
            ));
            return;
        }
    }
}

fn validate_surface_semantic_facts(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(facts) = contract.get("semanticFacts") else {
        issues.push(issue(
            "UI_SURFACE_SEMANTIC_FACTS_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts",
            "uiSurfaceDecisionContract requires semanticFacts.",
        ));
        return;
    };
    validate_enum_array(
        facts,
        "/userJobs",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.userJobs",
        &UI_USER_JOB_KINDS,
        "UI_SURFACE_USER_JOBS_INVALID",
        issues,
    );
    validate_enum_array(
        facts,
        "/informationShapes",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.informationShapes",
        &UI_INFORMATION_SHAPES,
        "UI_SURFACE_INFORMATION_SHAPES_INVALID",
        issues,
    );
    validate_enum_array(
        facts,
        "/operationModels",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.operationModels",
        &UI_OPERATION_MODELS,
        "UI_SURFACE_OPERATION_MODELS_INVALID",
        issues,
    );
    validate_enum_array(
        facts,
        "/riskFactors",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.riskFactors",
        &UI_RISK_FACTORS,
        "UI_SURFACE_RISK_FACTORS_INVALID",
        issues,
    );
    require_string_in(
        facts,
        "/navigationModel",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.navigationModel",
        &UI_NAVIGATION_MODELS,
        "UI_SURFACE_NAVIGATION_MODEL_INVALID",
        issues,
    );
    require_string_in(
        facts,
        "/devicePosture",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.devicePosture",
        &UI_DEVICE_POSTURES,
        "UI_SURFACE_DEVICE_POSTURE_INVALID",
        issues,
    );
    require_string_in(
        facts,
        "/productMode",
        "content.frontendExperience.uiSurfaceDecisionContract.semanticFacts.productMode",
        &UI_PRODUCT_MODES,
        "UI_SURFACE_PRODUCT_MODE_INVALID",
        issues,
    );
}

fn validate_enum_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    allowed: &[&str],
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(items) = root.pointer(pointer).and_then(Value::as_array) else {
        issues.push(issue(code, field_path, "field must be an array."));
        return;
    };
    if items.is_empty() {
        issues.push(issue(code, field_path, "field must not be empty."));
        return;
    }
    for item in items {
        if !item.as_str().is_some_and(|value| allowed.contains(&value)) {
            issues.push(issue(
                code,
                field_path,
                "field uses an unsupported enum value.",
            ));
            return;
        }
    }
}

fn validate_surface_regions_actions_states(contract: &Value, issues: &mut Vec<RepairIssue>) {
    validate_required_object_array(
        contract,
        "/regionModel",
        "content.frontendExperience.uiSurfaceDecisionContract.regionModel",
        "regionId",
        "UI_SURFACE_REGIONS_REQUIRED",
        issues,
    );
    validate_required_object_array(
        contract,
        "/actionModel",
        "content.frontendExperience.uiSurfaceDecisionContract.actionModel",
        "actionId",
        "UI_SURFACE_ACTIONS_REQUIRED",
        issues,
    );
    validate_required_object_array(
        contract,
        "/stateModel",
        "content.frontendExperience.uiSurfaceDecisionContract.stateModel",
        "state",
        "UI_SURFACE_STATES_REQUIRED",
        issues,
    );
}

fn validate_required_object_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    identity_key: &str,
    code: &str,
    issues: &mut Vec<RepairIssue>,
) {
    let Some(items) = root.pointer(pointer).and_then(Value::as_array) else {
        issues.push(issue(code, field_path, "field must be an array."));
        return;
    };
    if items.is_empty() {
        issues.push(issue(code, field_path, "field must not be empty."));
        return;
    }
    for item in items {
        if item
            .get(identity_key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            issues.push(issue(
                code,
                field_path,
                "each item must include a non-empty identity field.",
            ));
            return;
        }
    }
}

fn validate_surface_content_boundary(contract: &Value, issues: &mut Vec<RepairIssue>) {
    validate_required_string_array(
        contract,
        "/contentBoundary/forbiddenUserVisibleContent",
        "content.frontendExperience.uiSurfaceDecisionContract.contentBoundary.forbiddenUserVisibleContent",
        &UI_FORBIDDEN_USER_VISIBLE_CONTENT,
        "UI_SURFACE_FORBIDDEN_CONTENT_INVALID",
        issues,
    );
    require_non_empty_string(
        contract,
        "/contentBoundary/copyRule",
        "content.frontendExperience.uiSurfaceDecisionContract.contentBoundary.copyRule",
        "UI_SURFACE_COPY_RULE_REQUIRED",
        issues,
    );
}

fn validate_surface_reference_plan(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(plan) = contract.get("referencePlan").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_SURFACE_REFERENCE_PLAN_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.referencePlan",
            "uiSurfaceDecisionContract.referencePlan must be MCP-derived and non-empty.",
        ));
        return;
    };
    if plan.is_empty() {
        issues.push(issue(
            "UI_SURFACE_REFERENCE_PLAN_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.referencePlan",
            "uiSurfaceDecisionContract.referencePlan must not be empty.",
        ));
    }
    for (index, item) in plan.iter().enumerate() {
        for key in ["refId", "path", "reason"] {
            if item
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                issues.push(issue(
                    "UI_SURFACE_REFERENCE_PLAN_INVALID",
                    &format!(
                        "content.frontendExperience.uiSurfaceDecisionContract.referencePlan[{index}].{key}"
                    ),
                    "referencePlan entries must include refId, path, and reason.",
                ));
            }
        }
    }
}

fn validate_surface_quality_rules(contract: &Value, issues: &mut Vec<RepairIssue>) {
    let Some(rules) = contract.get("qualityRules").and_then(Value::as_array) else {
        issues.push(issue(
            "UI_SURFACE_QUALITY_RULES_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.qualityRules",
            "uiSurfaceDecisionContract.qualityRules must be MCP-derived and non-empty.",
        ));
        return;
    };
    if rules.is_empty() {
        issues.push(issue(
            "UI_SURFACE_QUALITY_RULES_REQUIRED",
            "content.frontendExperience.uiSurfaceDecisionContract.qualityRules",
            "uiSurfaceDecisionContract.qualityRules must not be empty.",
        ));
    }
    for (index, item) in rules.iter().enumerate() {
        for key in ["ruleId", "expectation"] {
            if item
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                issues.push(issue(
                    "UI_SURFACE_QUALITY_RULE_INVALID",
                    &format!(
                        "content.frontendExperience.uiSurfaceDecisionContract.qualityRules[{index}].{key}"
                    ),
                    "qualityRules entries must include ruleId and expectation.",
                ));
            }
        }
        require_string_in(
            item,
            "/severity",
            &format!(
                "content.frontendExperience.uiSurfaceDecisionContract.qualityRules[{index}].severity"
            ),
            &UI_QUALITY_RULE_SEVERITIES,
            "UI_SURFACE_QUALITY_RULE_SEVERITY_INVALID",
            issues,
        );
    }
}

fn ui_quality_rule_preview(
    scenario: &str,
    reference_groups: &Value,
    design_token_plan: &Value,
) -> Value {
    Value::Array(ui_quality_rule_specs(
        scenario,
        reference_groups,
        design_token_plan,
    ))
}

fn ui_quality_rule_specs(
    scenario: &str,
    reference_groups: &Value,
    design_token_plan: &Value,
) -> Vec<Value> {
    let mut rules = Vec::new();
    push_rule(
        &mut rules,
        "surface.contract.evidence_coverage",
        "uix.core.surface-decision",
        "must",
        &[
            "app_shell",
            "page",
            "navigation",
            "record_list",
            "record_detail",
            "form",
            "action_panel",
        ],
        "Task-owned UI regions, actions, states, quality rules, and content boundary must be implemented and proven through frontendQualitySelfCheck surface evidence.",
        &[
            "surface_region_evidence",
            "surface_action_evidence",
            "surface_state_evidence",
            "surface_quality_rule_evidence",
            "content_boundary_evidence",
        ],
    );
    push_rule(
        &mut rules,
        "anti.product_boundary.no_internal_process",
        "uix.core.anti-patterns",
        "must",
        &["app_shell", "page", "navigation", "record_list", "record_detail", "form"],
        "User-visible UI must not expose Loom/MCP terms, delivery progress, runtime commands, verification instructions, stack explanations, request ids, or future-phase planning language.",
        &["changed_files", "source_check", "forbidden_content_check"],
    );
    push_rule(
        &mut rules,
        "verify.rendered_viewports",
        "uix.core.verification",
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form"],
        "When a local preview is available, record desktop and mobile rendered inspection; when unavailable, record blocked_by_environment with the concrete blocker and fallback source checks.",
        &["render_or_environment_reason", "viewport_check", "fallback_source_check"],
    );
    match scenario {
        "admin_dashboard" | "fintech_workstation" => {
            push_admin_rules(&mut rules);
        }
        "data_console" | "developer_tool" => {
            push_data_rules(&mut rules);
        }
        "mobile_responsive" | "consumer_app" | "fintech_consumer_app" => {
            push_mobile_rules(&mut rules);
        }
        _ => {}
    }
    if reference_group_contains(reference_groups, "scenarios", "admin-dashboard") {
        push_admin_rules(&mut rules);
    }
    if reference_group_contains(reference_groups, "focus", "data")
        || reference_group_contains(reference_groups, "scenarios", "data-console")
    {
        push_data_rules(&mut rules);
    }
    if reference_group_contains(reference_groups, "focus", "mobile")
        || reference_group_contains(reference_groups, "scenarios", "mobile-responsive")
        || reference_group_contains(reference_groups, "scenarios", "mobile-native")
    {
        push_mobile_rules(&mut rules);
    }
    if reference_group_contains(reference_groups, "focus", "frameworks") {
        push_rule(
            &mut rules,
            "framework.component_structure",
            "uix.focus.frameworks",
            "must",
            &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
            "Real screens must separate shell, page orchestration, feature components, shared primitives, data/API helpers, and state-specific components when the workflow spans multiple regions.",
            &["changed_files", "component_split_evidence"],
        );
    }
    if reference_group_contains(reference_groups, "focus", "web-implementation") {
        push_web_implementation_rules(&mut rules);
    }
    if reference_group_contains(reference_groups, "stacks", "react") {
        push_rule(
            &mut rules,
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
        push_token_rules(&mut rules, design_token_plan);
    }
    dedupe_rule_specs(rules)
}

fn push_admin_rules(rules: &mut Vec<Value>) {
    push_rule(
        rules,
        "admin.shell.work_surface",
        "uix.scenarios.admin-dashboard",
        "must",
        &["app_shell", "page"],
        "The first viewport must be the working business console with navigation, current page context, a real work region, and primary business action access.",
        &["changed_files", "surface_evidence", "source_check"],
    );
    push_rule(
        rules,
        "admin.topbar.context_actions",
        "uix.scenarios.admin-dashboard",
        "should",
        &["app_shell", "navigation", "page"],
        "Topbar/header content must provide operational context and relevant actions such as search, filters, user/workspace context, or primary action; it must not be filler description.",
        &["changed_files", "surface_evidence"],
    );
    push_rule(
        rules,
        "admin.list.filter_table_detail",
        "uix.scenarios.admin-dashboard",
        "must",
        &["record_list", "record_detail", "page"],
        "Record-management screens must preserve list context across filter, pagination, row selection, detail viewing, and mutations.",
        &["changed_files", "state_coverage", "workflow_evidence"],
    );
}

fn push_data_rules(rules: &mut Vec<Value>) {
    push_rule(
        rules,
        "data.surface.scan_action_path",
        "uix.focus.data",
        "must",
        &["record_list", "record_detail", "page"],
        "Data surfaces must show object identity, status, key fields, and available action in the same scan path, with loading, empty, error, and business-blocking states placed near the affected region.",
        &["changed_files", "state_coverage", "surface_evidence"],
    );
    push_rule(
        rules,
        "admin.state.scoped_feedback",
        "uix.core.interaction",
        "must",
        &["record_list", "record_detail", "form", "action_panel"],
        "Loading, success, validation, error, and business-blocking feedback must appear near the table, form, detail, row, or action they affect instead of only in a generic global message.",
        &["changed_files", "state_coverage", "business_feedback_evidence"],
    );
}

fn push_mobile_rules(rules: &mut Vec<Value>) {
    push_rule(
        rules,
        "admin.mobile.record_fallback",
        "uix.focus.mobile",
        "must",
        &["record_list", "record_detail", "page"],
        "Responsive record-management UI must keep the workflow usable on narrow screens through cards, drawer/detail route, or an explicit source-checked fallback; do not rely only on shrinking a dense table.",
        &["changed_files", "viewport_check", "responsive_source_check"],
    );
}

fn push_web_implementation_rules(rules: &mut Vec<Value>) {
    push_rule(
        rules,
        "web.semantic_accessibility",
        "uix.focus.web-implementation",
        "must",
        &[
            "app_shell",
            "page",
            "navigation",
            "record_list",
            "record_detail",
            "form",
            "action_panel",
        ],
        "Web UI must use native semantics before ARIA, provide accessible names for icon-only controls and form fields, preserve visible focus, and announce scoped async feedback when it changes user state.",
        &["changed_files", "source_check", "accessibility_source_evidence"],
    );
    push_rule(
        rules,
        "web.form_and_state_resilience",
        "uix.focus.web-implementation",
        "must",
        &["form", "action_panel"],
        "Web forms and business actions must include meaningful field metadata, keep input recoverable on failure, show inline errors near affected fields or controls, and avoid blocking paste or double submission.",
        &["changed_files", "source_check", "state_coverage", "form_resilience_evidence"],
    );
    push_rule(
        rules,
        "web.runtime_layout_safety",
        "uix.focus.web-implementation",
        "must",
        &[
            "app_shell",
            "page",
            "record_list",
            "record_detail",
            "form",
            "action_panel",
        ],
        "Web surfaces must handle long content, empty collections, media sizing, large lists, reduced motion, locale formatting, hydration-sensitive values, and restorable navigation state where those concerns are in scope.",
        &["changed_files", "source_check", "state_coverage", "layout_resilience_evidence"],
    );
}

fn push_token_rules(rules: &mut Vec<Value>, design_token_plan: &Value) {
    let template_ref = match design_token_plan.get("templateId").and_then(Value::as_str) {
        Some("tokens-tailwind") => "uix.templates.tokens-tailwind",
        _ => "uix.templates.tokens-css",
    };
    push_rule(
        rules,
        "token.semantic_roles.coverage",
        template_ref,
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
        "Token assets must cover semantic surface, text, border, primary, status, focus, control, shell, table/list, and detail/action roles needed by the implemented UI.",
        &["token_asset_files", "token_consumer_files", "source_check"],
    );
    push_rule(
        rules,
        "token.single_source_consumed",
        "uix.core.system",
        "must",
        &["app_shell", "page", "record_list", "record_detail", "form", "action_panel"],
        "The UI must consume one token/theme source through the project style entry or component system and must not create a parallel token system.",
        &["token_asset_files", "token_consumer_files", "source_check"],
    );
}

fn push_rule(
    rules: &mut Vec<Value>,
    rule_id: &str,
    source_ref_id: &str,
    severity: &str,
    surface_roles: &[&str],
    expectation: &str,
    evidence_required: &[&str],
) {
    rules.push(json!({
        "ruleId": rule_id,
        "sourceRefId": source_ref_id,
        "severity": severity,
        "appliesToSurfaceRoles": surface_roles,
        "expectation": expectation,
        "evidenceRequired": evidence_required
    }));
}

fn dedupe_rule_specs(rules: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for rule in rules {
        let Some(rule_id) = rule.get("ruleId").and_then(Value::as_str) else {
            continue;
        };
        if seen.insert(rule_id.to_string()) {
            deduped.push(rule);
        }
    }
    deduped
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
    if should_load_web_implementation_reference(scenario, stack_items) {
        push_reference_group_item(&mut groups, "focus", "web-implementation");
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

fn should_load_web_implementation_reference(scenario: &str, stack_items: &[String]) -> bool {
    if stack_items
        .iter()
        .any(|item| matches!(item.as_str(), "native-mobile" | "uniapp"))
    {
        return false;
    }
    if stack_items
        .iter()
        .any(|item| matches!(item.as_str(), "react" | "vue" | "svelte" | "plain-html"))
    {
        return true;
    }
    if stack_items.iter().any(|item| item == "threejs") {
        return false;
    }
    matches!(
        scenario,
        "admin_dashboard"
            | "data_console"
            | "fintech_workstation"
            | "fintech_consumer_app"
            | "consumer_app"
            | "mobile_responsive"
            | "marketing_site"
            | "corporate_site"
            | "docs_site"
            | "developer_tool"
    )
}

fn infer_stack_reference_items(baseline: Option<&TechnicalBaselineContract>) -> Vec<String> {
    let stack = baseline
        .map(|item| item.stack.to_string().to_lowercase())
        .unwrap_or_default();
    let mut refs = Vec::new();
    let native_mobile_stack = contains_any(
        &stack,
        &[
            "react native",
            "flutter",
            "swift",
            "kotlin",
            "ios",
            "android",
        ],
    );
    if !native_mobile_stack && contains_any(&stack, &["react", "next", "vite"]) {
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
    if native_mobile_stack {
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
        build_ui_quality_seed, known_ui_reference_groups,
        normalize_ui_surface_decision_contract_for_persist, scenario_supporting_reference_items,
        ui_surface_decision_candidate_shape, ui_surface_decision_candidate_template,
        ui_surface_decision_contract_shape, ui_surface_decision_enum_refs,
        validate_ui_surface_decision_contract, UI_CORE_REFERENCE_ITEMS,
        UI_DESIGN_TOKEN_TEMPLATE_IDS, UI_TOKEN_REFERENCE_ITEMS,
    };
    use crate::{
        ConfidenceLevel, ProjectKind, TechnicalBaselineApproval, TechnicalBaselineApprovalType,
        TechnicalBaselineContract, TechnicalBaselineScope, TechnicalBaselineSource,
        TechnicalBaselineStatus,
    };
    use serde_json::{json, Value};

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
    fn ui_surface_decision_enums_allow_known_hybrid_and_custom() {
        let refs = ui_surface_decision_enum_refs();
        for mode in ["known", "hybrid", "custom"] {
            assert!(
                array_contains(&refs, "patternMode", mode),
                "surface decision enum refs must allow {mode}"
            );
        }
        for pattern in [
            "collection_workbench",
            "decision_queue",
            "editor_workspace",
            "immersive_workspace",
        ] {
            assert!(
                array_contains(&refs, "knownPattern", pattern),
                "surface decision enum refs must include {pattern}"
            );
        }
    }

    #[test]
    fn ui_surface_decision_shapes_keep_custom_structured_not_relaxed() {
        let candidate = ui_surface_decision_candidate_shape();
        let contract = ui_surface_decision_contract_shape();

        assert!(
            candidate
                .pointer("/selectedPattern/customPattern")
                .is_some(),
            "candidate shape must expose a custom pattern name"
        );
        assert!(
            candidate
                .pointer("/selectedPattern/nearestKnownPatterns")
                .is_some(),
            "custom candidates must compare against nearest known patterns"
        );
        for pointer in [
            "/semanticFacts/customExtensions",
            "/layoutModel/customLayoutIntent",
            "/regionModel/0/regionId",
            "/actionModel/0/placementRegionId",
            "/stateModel/0/placementRegionId",
            "/compositionConstraints/customRules",
            "/contentBoundary/customForbiddenContent",
        ] {
            assert!(
                candidate.pointer(pointer).is_some(),
                "custom candidate shape must include {pointer}"
            );
        }

        for pointer in [
            "/patternDecision",
            "/semanticFacts",
            "/layoutModel",
            "/regionModel",
            "/informationModel",
            "/actionModel",
            "/stateModel",
            "/compositionConstraints",
            "/contentBoundary",
            "/referencePlan",
            "/qualityRules",
        ] {
            assert!(
                contract.pointer(pointer).is_some(),
                "decision contract shape must include {pointer}"
            );
        }
    }

    #[test]
    fn ui_surface_decision_template_has_structured_candidate_defaults() {
        let template = ui_surface_decision_candidate_template();

        for pointer in [
            "/patternRankings/0/kind",
            "/selectedPattern/mode",
            "/semanticFacts/customExtensions",
            "/layoutModel/primaryWorkRegionId",
            "/regionModel/0/regionId",
            "/informationModel/primaryObjects",
            "/actionModel/0/actionId",
            "/stateModel/0/state",
            "/compositionConstraints/customRules",
            "/contentBoundary/copyRule",
        ] {
            assert!(
                template.pointer(pointer).is_some(),
                "candidate template must include {pointer}"
            );
        }
    }

    #[test]
    fn ui_surface_decision_normalization_derives_contract_references_and_rules() {
        let seed = build_ui_quality_seed(None, None);
        let mut frontend = json!({
            "required": true,
            "surfaceDecisionCandidate": filled_surface_candidate()
        });

        assert!(
            normalize_ui_surface_decision_contract_for_persist(&mut frontend, &seed),
            "normalization must write uiSurfaceDecisionContract"
        );

        let issues = validate_ui_surface_decision_contract(&frontend);
        assert!(
            issues.is_empty(),
            "normalized known surface contract should validate cleanly: {issues:?}"
        );
        let contract = frontend
            .get("uiSurfaceDecisionContract")
            .expect("surface contract must be written");
        assert_eq!(
            contract
                .pointer("/patternDecision/mode")
                .and_then(Value::as_str),
            Some("known")
        );
        assert!(
            contract
                .get("referencePlan")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            "surface contract must contain MCP-derived references"
        );
        assert!(
            contract
                .get("referencePlan")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|item| item.get("refId").and_then(Value::as_str)
                    == Some("uix.core.surface-decision")),
            "surface contract must load the surface decision reference"
        );
        assert!(
            contract
                .get("qualityRules")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            "surface contract must contain MCP-derived quality rules"
        );
        assert!(
            contract
                .get("qualityRules")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|item| item.get("ruleId").and_then(Value::as_str)
                    == Some("surface.contract.evidence_coverage")),
            "surface contract must include the generic surface evidence quality rule"
        );
        assert!(
            contract
                .pointer("/contentBoundary/forbiddenUserVisibleContent")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|item| item.as_str() == Some("runtime_commands")),
            "MCP must merge universal forbidden content into the surface contract"
        );
    }

    #[test]
    fn ui_surface_decision_custom_mode_is_structured_and_reviewable() {
        let seed = build_ui_quality_seed(None, None);
        let mut candidate = filled_surface_candidate();
        candidate["selectedPattern"]["mode"] = json!("custom");
        candidate["selectedPattern"]["knownPattern"] = Value::Null;
        candidate["selectedPattern"]["customPattern"] = json!("priority_capacity_map");
        candidate["selectedPattern"]["nearestKnownPatterns"] =
            json!(["collection_workbench", "analytics_monitor"]);
        candidate["selectedPattern"]["rationale"] =
            json!("The requested surface mixes dense queue work with monitoring, so no single known pattern is authoritative.");
        let mut frontend = json!({
            "required": true,
            "surfaceDecisionCandidate": candidate
        });

        assert!(
            normalize_ui_surface_decision_contract_for_persist(&mut frontend, &seed),
            "normalization must write a custom uiSurfaceDecisionContract"
        );

        let issues = validate_ui_surface_decision_contract(&frontend);
        assert!(
            issues.is_empty(),
            "normalized custom surface contract should validate cleanly: {issues:?}"
        );
        let contract = frontend
            .get("uiSurfaceDecisionContract")
            .expect("surface contract must be written");
        assert_eq!(
            contract
                .pointer("/patternDecision/mode")
                .and_then(Value::as_str),
            Some("custom")
        );
        assert!(
            contract
                .pointer("/patternDecision/nearestKnownPatterns")
                .and_then(Value::as_array)
                .is_some_and(|items| items.len() >= 2),
            "custom mode must retain nearest known pattern comparison"
        );
        assert!(
            contract
                .get("qualityRules")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            "custom mode must still receive executable quality rules"
        );
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
        let baseline = technical_baseline_with_stack(serde_json::json!("React + Tailwind"));
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
    fn web_stack_ui_quality_seed_loads_web_implementation_reference_and_gates() {
        let baseline =
            technical_baseline_with_stack(serde_json::json!("Next.js + React + TypeScript"));
        let seed = build_ui_quality_seed(None, Some(&baseline));
        let reference_groups = seed
            .get("requiredReferenceGroups")
            .expect("seed must include requiredReferenceGroups");

        assert!(
            group_contains(reference_groups, "focus", "web-implementation"),
            "browser UI seed must include focus.web-implementation"
        );
        let reference_plan = seed
            .get("referenceLoadPlan")
            .and_then(Value::as_array)
            .expect("seed must include referenceLoadPlan");
        assert!(
            reference_plan
                .iter()
                .any(|item| item.get("path").and_then(Value::as_str)
                    == Some("uix/web-implementation.md")),
            "browser UI seed must load uix/web-implementation.md"
        );

        let mut frontend = json!({
            "required": true,
            "surfaceDecisionCandidate": filled_surface_candidate()
        });
        assert!(
            normalize_ui_surface_decision_contract_for_persist(&mut frontend, &seed),
            "normalization must write web-stack surface rules"
        );
        let rule_ids = frontend["uiSurfaceDecisionContract"]
            .get("qualityRules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|rule| rule.get("ruleId").and_then(Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();
        for expected in [
            "web.semantic_accessibility",
            "web.form_and_state_resilience",
            "web.runtime_layout_safety",
        ] {
            assert!(
                rule_ids.contains(expected),
                "browser UI surface contract must include {expected}"
            );
        }

        let mixed_scene_baseline =
            technical_baseline_with_stack(serde_json::json!("React + Three.js"));
        let mixed_scene_seed = build_ui_quality_seed(None, Some(&mixed_scene_baseline));
        assert!(
            group_contains(
                mixed_scene_seed
                    .get("requiredReferenceGroups")
                    .expect("seed must include requiredReferenceGroups"),
                "focus",
                "web-implementation"
            ),
            "React browser UI with a 3D scene still needs web implementation rules"
        );
    }

    #[test]
    fn native_and_threejs_ui_quality_seed_do_not_load_web_implementation_reference() {
        for stack in [
            serde_json::json!("React Native + TypeScript"),
            serde_json::json!("Flutter + Dart"),
            serde_json::json!("Three.js + WebGL"),
        ] {
            let baseline = technical_baseline_with_stack(stack);
            let seed = build_ui_quality_seed(None, Some(&baseline));
            let reference_groups = seed
                .get("requiredReferenceGroups")
                .expect("seed must include requiredReferenceGroups");

            assert!(
                !group_contains(reference_groups, "focus", "web-implementation"),
                "native, mini-app, or primary 3D stacks must not load focus.web-implementation"
            );
        }
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

    fn array_contains(root: &serde_json::Value, key: &str, expected: &str) -> bool {
        root.get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(expected))
    }

    fn filled_surface_candidate() -> Value {
        let mut candidate = ui_surface_decision_candidate_template();
        candidate["patternRankings"][0]["score"] = json!(0.86);
        candidate["patternRankings"][0]["matchedSignals"] = json!([
            "record collection",
            "filterable work queue",
            "row-level action"
        ]);
        candidate["patternRankings"][0]["evidenceRefs"] = json!(["frontend.detail.queue"]);
        candidate["selectedPattern"]["rationale"] =
            json!("The current phase centers on scanning a record collection, comparing status, and taking row-level business action.");
        candidate["selectedPattern"]["evidenceRefs"] = json!(["frontend.detail.queue"]);
        candidate["semanticFacts"]["userJobs"] = json!(["browse", "compare", "create"]);
        candidate["semanticFacts"]["informationShapes"] =
            json!(["record_collection", "record_detail"]);
        candidate["semanticFacts"]["operationModels"] =
            json!(["filter_sort_paginate", "create_update"]);
        candidate["semanticFacts"]["riskFactors"] = json!(["none"]);
        candidate["semanticFacts"]["evidenceRefs"] = json!(["frontend.detail.queue"]);
        candidate["layoutModel"]["desktop"]["layoutIntent"] =
            json!("Show navigation, filters, dense record results, detail context, and primary action without displacing the work region.");
        candidate["layoutModel"]["desktop"]["allowedPresentations"] =
            json!(["table", "detail_panel", "form_sections"]);
        candidate["layoutModel"]["tablet"]["layoutIntent"] =
            json!("Keep record scan first and move detail/actions into a secondary region.");
        candidate["layoutModel"]["tablet"]["allowedPresentations"] =
            json!(["record_cards", "drawer"]);
        candidate["layoutModel"]["mobile"]["layoutIntent"] =
            json!("Stack records and use drill-down detail when comparison is not required.");
        candidate["layoutModel"]["mobile"]["allowedPresentations"] =
            json!(["record_cards", "route_detail"]);
        candidate["regionModel"][0]["purpose"] =
            json!("Primary record queue where users scan work items and start the main action.");
        candidate["regionModel"][0]["desktopPlacement"] =
            json!("Main content column below topbar and beside navigation.");
        candidate["regionModel"][0]["mobilePlacement"] =
            json!("First stacked region after compact page context.");
        candidate["regionModel"][0]["requiredContent"] = json!([
            "record identity",
            "status",
            "primary action",
            "local feedback"
        ]);
        candidate["informationModel"]["primaryObjects"] = json!(["request"]);
        candidate["informationModel"]["fields"] = json!(["id", "status", "owner", "updatedAt"]);
        candidate["informationModel"]["identityFields"] = json!(["id"]);
        candidate["informationModel"]["statusFields"] = json!(["status"]);
        candidate["informationModel"]["scanOrder"] =
            json!(["identity", "status", "decision field", "action"]);
        candidate["actionModel"][0]["label"] = json!("Create request");
        candidate["actionModel"][0]["pendingFeedback"] =
            json!("Show pending state on the submitting action and affected form region.");
        candidate["actionModel"][0]["successFeedback"] =
            json!("Insert the created request into the visible list and show scoped confirmation.");
        candidate["actionModel"][0]["errorFeedback"] =
            json!("Show actionable error near the form fields or list region.");
        candidate["actionModel"][0]["postSuccessUpdate"] =
            json!("Refresh the affected list and clear the completed draft.");
        candidate["stateModel"][0]["placementRule"] = json!(
            "Show loading near the record queue or submitting form, not only in a global banner."
        );
        candidate["stateModel"][0]["recoveryPath"] =
            json!("Keep filters and draft data stable while the user retries.");
        candidate["compositionConstraints"]["requiredComposition"] = json!([
            "business context",
            "record queue",
            "primary action",
            "scoped feedback"
        ]);
        candidate["contentBoundary"]["allowedUserVisibleContent"] = json!([
            "labels",
            "filters",
            "status",
            "actions",
            "validation",
            "business_feedback"
        ]);
        candidate
    }

    fn technical_baseline_with_stack(stack: serde_json::Value) -> TechnicalBaselineContract {
        TechnicalBaselineContract {
            schema_version: "1.0".to_string(),
            technical_baseline_id: "tbr-test".to_string(),
            delivery_id: "delivery-test".to_string(),
            phase_id: "phase-test".to_string(),
            status: TechnicalBaselineStatus::Confirmed,
            source: TechnicalBaselineSource::UserConfirmed,
            project_kind: ProjectKind::NewProject,
            scope: TechnicalBaselineScope::Project,
            stack,
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
        }
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
