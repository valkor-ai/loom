use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::RepairIssue;
use sha2::{Digest, Sha256};

/// The external request protocol stays `outputContract` plus
/// `requestReadPlan.groups`. This descriptor only centralizes how that
/// existing contract is materialized and fingerprinted.
pub const AGENT_WRITE_CONTRACT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentFieldOwner {
    Agent,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentFieldApplicability {
    CurrentRequest,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentFieldPolicy {
    pub owner: AgentFieldOwner,
    pub applicability: AgentFieldApplicability,
    pub empty_policy: String,
    pub preserve_on_repair: bool,
}

impl Default for AgentFieldPolicy {
    fn default() -> Self {
        Self {
            owner: AgentFieldOwner::Agent,
            applicability: AgentFieldApplicability::Unknown,
            empty_policy: "follow_schema_and_applicability".to_string(),
            preserve_on_repair: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskEvidenceApplicability {
    pub frontend_self_check: bool,
    pub frontend_quality_self_check: bool,
    pub runtime_delivery_evidence: bool,
    pub engineering_quality_evidence: bool,
    pub architecture_quality_evidence: bool,
    pub api_contract_evidence: bool,
    pub code_quality_evidence: bool,
}

/// Resolve optional TaskResult evidence from the same structured task shape
/// used by execution, persistence, and review. No prose or technology keyword
/// is consulted here.
pub fn task_evidence_applicability_from_value(task: &Value) -> TaskEvidenceApplicability {
    let frontend = task
        .get("frontendExperienceRequirement")
        .filter(|value| !value.is_null());
    let runtime = task
        .pointer("/runtimeDeliveryRequirement/appliesToThisTask")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    TaskEvidenceApplicability {
        frontend_self_check: frontend.is_some(),
        frontend_quality_self_check: frontend.is_some_and(|requirement| {
            requirement.get("uiSurfaceDecisionContractRef").is_some()
                || requirement
                    .pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
                    .is_some_and(Value::is_object)
        }),
        runtime_delivery_evidence: runtime,
        engineering_quality_evidence: non_empty_array_field(
            task,
            "engineeringQualityRequirementRefs",
        ),
        architecture_quality_evidence: non_empty_array_field(
            task,
            "architectureQualityRequirementRefs",
        ),
        api_contract_evidence: non_empty_array_field(task, "apiContractRequirementRefs"),
        code_quality_evidence: non_empty_array_field(task, "codeQualityRequirementRefs"),
    }
}

fn non_empty_array_field(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

/// Build the compact, tree-shaped field contract from the canonical schema.
/// The full JSON schema remains private; the tree avoids repeating long leaf
/// prefixes in the Agent-facing read group.
pub fn compact_agent_field_contract(
    schema_shape: &Value,
    policies: &BTreeMap<String, AgentFieldPolicy>,
) -> Value {
    compact_agent_field_contract_with_required(schema_shape, &[], policies, None)
}

fn compact_agent_field_contract_with_required(
    schema_shape: &Value,
    required_override: &[String],
    policies: &BTreeMap<String, AgentFieldPolicy>,
    shape_rules: Option<&Value>,
) -> Value {
    let mut contract = json!({
        "version": AGENT_WRITE_CONTRACT_VERSION,
        "authority": "outputContract.schemaShape",
        "defaults": {
            "owner": "agent",
            "applicability": "unknown",
            "emptyPolicy": "follow_schema_and_applicability",
            "preserveOnRepair": true
        },
        "properties": {}
    });
    let required = schema_shape
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .chain(required_override.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(object) = contract.as_object_mut() {
        object.insert(
            "required".to_string(),
            Value::Array(required.iter().map(|field| json!(field)).collect()),
        );
    }
    let Some(output) = contract
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    else {
        return contract;
    };
    let sparse_paths = shape_rules.map(|shape_rules| {
        policies
            .keys()
            .cloned()
            .chain(required.iter().cloned())
            .chain(
                shape_rules
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(path, _)| path.clone()),
            )
            .collect::<BTreeSet<_>>()
    });
    if let Some(properties) = schema_shape.get("properties").and_then(Value::as_object) {
        for (name, schema) in properties {
            let path = name.clone();
            if sparse_paths
                .as_ref()
                .is_some_and(|paths| !path_is_interesting(&path, paths))
            {
                continue;
            }
            output.insert(
                name.clone(),
                compact_schema_node(
                    schema,
                    schema_shape,
                    &path,
                    required.contains(name.as_str()),
                    policies,
                    0,
                    sparse_paths.as_ref(),
                ),
            );
        }
    } else if let Some(properties) = schema_shape.as_object() {
        for (name, shape) in properties {
            if matches!(name.as_str(), "$defs" | "$schema" | "title" | "type") {
                continue;
            }
            let path = name.clone();
            if sparse_paths
                .as_ref()
                .is_some_and(|paths| !path_is_interesting(&path, paths))
            {
                continue;
            }
            output.insert(
                name.clone(),
                compact_manual_node(
                    shape,
                    &path,
                    required.contains(name.as_str()),
                    policies,
                    0,
                    sparse_paths.as_ref(),
                ),
            );
        }
    }
    contract
}

/// Attach the shared contract metadata to the existing outputContract object.
/// Callers may provide field policies for deterministic applicability facts;
/// absent policies default to current-request Agent-owned fields.
pub fn finalize_output_contract(
    output_contract: &mut Value,
    field_policies: &BTreeMap<String, AgentFieldPolicy>,
) {
    let Some(contract) = output_contract.as_object_mut() else {
        return;
    };
    contract.insert(
        "contractVersion".to_string(),
        json!(AGENT_WRITE_CONTRACT_VERSION),
    );
    let mut projection = contract
        .remove("schemaProjection")
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if let Some(projection_object) = projection.as_object_mut() {
        projection_object.remove("fieldContract");
        projection_object.remove("fieldContractByTarget");
    }
    let legacy_shape_rules = projection
        .get("objectShapeRules")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let required_override = projection
        .get("requiredTopLevelFields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let schema_entries = contract
        .iter()
        .filter(|(key, value)| {
            (key.as_str() == "schemaShape" || key.ends_with("SchemaShape")) && value.is_object()
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    let mut field_contracts = Map::new();
    for (key, schema_shape) in schema_entries {
        let mut field_contract = compact_agent_field_contract_with_required(
            &schema_shape,
            if key == "schemaShape" {
                &required_override
            } else {
                &[]
            },
            field_policies,
            Some(&legacy_shape_rules),
        );
        if key == "schemaShape" {
            apply_shape_rule_constraints(&mut field_contract, &legacy_shape_rules);
        }
        field_contracts.insert(schema_target_name(&key), field_contract);
    }
    if let Some(result_contract) = field_contracts.remove("result") {
        projection
            .as_object_mut()
            .expect("schemaProjection is an object")
            .insert("fieldContract".to_string(), result_contract.clone());
        if !field_contracts.is_empty() {
            field_contracts.insert("result".to_string(), result_contract);
        }
    }
    if !field_contracts.is_empty() {
        projection
            .as_object_mut()
            .expect("schemaProjection is an object")
            .insert(
                "fieldContractByTarget".to_string(),
                Value::Object(field_contracts),
            );
    }
    projection
        .as_object_mut()
        .expect("schemaProjection is an object")
        .remove("objectShapeRules");
    contract.insert("schemaProjection".to_string(), projection);
    if let Some(artifact_kind) = contract
        .get("artifactKind")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        let mut normalized_fields = mcp_normalized_fields_for_artifact(&artifact_kind)
            .iter()
            .map(|field| (*field).to_string())
            .collect::<BTreeSet<_>>();
        normalized_fields.extend(
            field_policies
                .iter()
                .filter(|(_, policy)| policy.owner == AgentFieldOwner::Mcp)
                .map(|(path, _)| path.clone()),
        );
        if !normalized_fields.is_empty() {
            contract.insert(
                "mcpNormalizedFields".to_string(),
                Value::Array(normalized_fields.iter().map(|field| json!(field)).collect()),
            );
        }
        let delegated_paths = domain_validation_paths_for_artifact(&artifact_kind);
        if !delegated_paths.is_empty() {
            contract.insert(
                "domainValidationPaths".to_string(),
                Value::Array(delegated_paths.iter().map(|path| json!(path)).collect()),
            );
        }
    }
    let mut fingerprint_source = Value::Object(contract.clone());
    remove_volatile_contract_fields(&mut fingerprint_source);
    contract.insert(
        "contractFingerprint".to_string(),
        json!(contract_fingerprint(&fingerprint_source)),
    );
}

fn schema_target_name(key: &str) -> String {
    if key == "schemaShape" {
        return "result".to_string();
    }
    key.strip_suffix("SchemaShape")
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or("result")
        .to_string()
}

/// Derive only deterministic applicability facts from structured request
/// inputs. This intentionally does not inspect requirement prose or search
/// for technology/API keywords.
pub fn derive_agent_field_policies(root: &Value) -> BTreeMap<String, AgentFieldPolicy> {
    let mut policies = BTreeMap::new();
    let api_applicability = if root
        .get("apiQualitySeed")
        .and_then(|value| value.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            root.get("apiQualitySeed")
                .is_some_and(|value| !value.is_null())
        }) {
        AgentFieldApplicability::CurrentRequest
    } else {
        AgentFieldApplicability::NotApplicable
    };
    // `interfaces` remains the architecture boundary even when no HTTP API is
    // selected. Only the API-specific contract is conditional.
    policies.insert(
        "content.interfaces".to_string(),
        policy_for(AgentFieldApplicability::CurrentRequest),
    );
    policies.insert(
        "content.apiContract".to_string(),
        policy_for(api_applicability),
    );
    derive_architecture_field_policies(root, &mut policies);

    let task = root.get("task").unwrap_or(&Value::Null);
    let applicability = task_evidence_applicability_from_value(task);
    policies.insert(
        "frontendExperienceSelfCheck".to_string(),
        policy_for(if applicability.frontend_self_check {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "frontendQualitySelfCheck".to_string(),
        policy_for(if applicability.frontend_quality_self_check {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "runtimeDeliveryEvidence".to_string(),
        policy_for(if applicability.runtime_delivery_evidence {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "apiContractEvidence".to_string(),
        policy_for(if applicability.api_contract_evidence {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "architectureQualityEvidence".to_string(),
        policy_for(if applicability.architecture_quality_evidence {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies.insert(
        "codeQualityEvidence".to_string(),
        policy_for(if applicability.code_quality_evidence {
            AgentFieldApplicability::CurrentRequest
        } else {
            AgentFieldApplicability::NotApplicable
        }),
    );
    policies
}

fn derive_architecture_field_policies(
    root: &Value,
    policies: &mut BTreeMap<String, AgentFieldPolicy>,
) {
    let persistence = structured_track_applicability(root, "persistence");
    policies.insert(
        "content.dataModel.dataArchitecture.persistenceMode".to_string(),
        policy_for(AgentFieldApplicability::CurrentRequest),
    );
    policies.insert(
        "content.dataModel.dataArchitecture.sourceOfTruth".to_string(),
        policy_for(persistence),
    );
    for field in [
        "ownership",
        "invariants",
        "transactionBoundaries",
        "consistencyRules",
        "migrationImpacts",
        "readModels",
        "lifecyclePolicies",
        "derivedData",
    ] {
        policies.insert(
            format!("content.dataModel.dataArchitecture.{field}"),
            policy_for(persistence),
        );
    }
    let architecture_quality = root
        .pointer("/architectureQualitySeed/required")
        .and_then(Value::as_bool)
        .map(|required| {
            if required {
                AgentFieldApplicability::CurrentRequest
            } else {
                AgentFieldApplicability::NotApplicable
            }
        })
        .unwrap_or(AgentFieldApplicability::Unknown);
    policies.insert(
        "content.architectureQuality".to_string(),
        policy_for(architecture_quality),
    );
}

fn structured_track_applicability(root: &Value, track_name: &str) -> AgentFieldApplicability {
    let track = root
        .pointer(&format!(
            "/contextProjection/technicalBaseline/stack/tracks/{track_name}"
        ))
        .or_else(|| root.pointer(&format!("/technicalBaseline/stack/tracks/{track_name}")));
    match track
        .and_then(Value::as_object)
        .and_then(|track| track.get("status"))
        .and_then(Value::as_str)
    {
        Some("selected" | "user_custom") => AgentFieldApplicability::CurrentRequest,
        Some("not_needed" | "not_applicable") => AgentFieldApplicability::NotApplicable,
        Some(_) | None => AgentFieldApplicability::Unknown,
    }
}

pub fn contract_fingerprint(value: &Value) -> String {
    let canonical = canonical_json(value);
    let digest = Sha256::digest(canonical.as_bytes());
    format!("sha256:{digest:x}")
}

pub fn contract_fingerprint_matches(value: &Value) -> bool {
    let Some(expected) = value.get("contractFingerprint").and_then(Value::as_str) else {
        return false;
    };
    let mut source = value.clone();
    remove_volatile_contract_fields(&mut source);
    contract_fingerprint(&source) == expected
}

pub fn validate_agent_write_contract(
    output_contract: &Value,
    target_id: &str,
    candidate: &Value,
) -> Vec<RepairIssue> {
    let Some(contract) = validation_contract_for_target(output_contract, target_id) else {
        return vec![RepairIssue {
            code: "WRITE_CONTRACT_SCHEMA_MISSING".to_string(),
            message: "The current write contract does not expose a field contract for this target."
                .to_string(),
            target_id: Some(target_id.to_string()),
            field_path: None,
        }];
    };
    let mcp_normalized_fields = output_contract
        .get("mcpNormalizedFields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let domain_validation_paths = output_contract
        .get("domainValidationPaths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    validate_contract_node(
        candidate,
        &contract,
        "candidate",
        true,
        target_id,
        &mcp_normalized_fields,
        &domain_validation_paths,
        &mut issues,
    );
    issues
}

fn mcp_normalized_fields_for_artifact(artifact_kind: &str) -> &'static [&'static str] {
    match artifact_kind {
        "brainstorm_candidate" => &[
            "userConfirmation",
            "clarificationProgress",
            "roadmap.currentPhaseId",
            "phasePlan.current.phaseId",
            "conceptGrounding.glossaryUpdates[].updateId",
        ],
        "repository_context_candidate" => &[
            "source",
            "requestLens",
            "repoOverview",
            "relevantSurfaces",
            "recommendedReadRefs",
            "contextQuality",
            "warnings",
        ],
        "architecture_section_candidate" => &[
            "schemaVersion",
            "requestId",
            "deliveryId",
            "phaseId",
            "section",
            "createdAt",
            "content.source",
            "content.runtimeDelivery.runtimeDependencies",
            "content.architectureQuality.decisions[].decisionId",
            "content.architectureQuality.decisions[].category",
            "content.architectureQuality.decisions[].sourceRefs",
            "content.architectureQuality.decisions[].ownerArtifactRefs",
            "content.architectureQuality.nfrs[].nfrId",
            "content.architectureQuality.nfrs[].category",
            "content.architectureQuality.nfrs[].source",
            "content.architectureQuality.nfrs[].sourceRefs",
            "content.architectureQuality.nfrs[].ownerArtifactRefs",
            "content.architectureQuality.nfrs[].architectureRefs",
            "content.architectureQuality.risks[].riskId",
            "content.architectureQuality.risks[].category",
            "content.architectureQuality.risks[].ownerArtifactRefs",
        ],
        "task_plan_candidate" => &[
            "schemaVersion",
            "requestId",
            "deliveryId",
            "phaseId",
            "taskPlanId",
            "createdAt",
            "groups[].scopeRefs",
            "groups[].acceptanceRefs",
            "group.scopeRefs",
            "group.acceptanceRefs",
            "tasks[].scopeRefs",
            "tasks[].acceptanceRefs",
            "tasks[].requirementDetailRefs",
            "tasks[].writeBoundary.forbiddenPaths",
            "tasks[].writeBoundary.artifactRefs.modules",
            "tasks[].writeBoundary.artifactRefs.entities",
            "tasks[].writeBoundary.artifactRefs.interfaces",
            "tasks[].writeBoundary.artifactRefs.consumedInterfaces",
            "tasks[].writeBoundary.artifactRefs.userFlows",
            "tasks[].writeBoundary.artifactRefs.stateMachines",
            "tasks[].writeBoundary.artifactRefs.decisions",
            "tasks[].writeBoundary.artifactRefs.nfrs",
            "tasks[].writeBoundary.artifactRefs.risks",
            "tasks[].verificationIntents[].acceptanceRefs",
            "tasks[].verificationIntents[].requirementDetailRefs",
            "tasks[].frontendExperienceRequirement",
            "tasks[].runtimeDeliveryRequirement",
            "tasks[].engineeringQualityRequirementRefs",
            "tasks[].architectureQualityRequirementRefs",
            "tasks[].apiContractRequirementRefs",
            "tasks[].codeQualityRequirementRefs",
        ],
        "task_result" | "task_result_repair" => &[
            "schemaVersion",
            "taskResultId",
            "taskPlanId",
            "taskId",
            "createdAt",
            "updatedAt",
            "verificationResults[].verificationId",
            "verificationResults[].evidenceType",
            "implementationObligationResults[].obligationId",
            "implementationObligationResults[].verificationIds",
            "requirementDetailEvidence[].detailId",
            "requirementDetailEvidence[].verificationIds",
            "frontendExperienceSelfCheck.closureRequirementIds",
            "frontendQualitySelfCheck.surfaceDecisionContractRef",
            "frontendQualitySelfCheck.surfaceRegionEvidence[].id",
            "frontendQualitySelfCheck.surfaceActionEvidence[].id",
            "frontendQualitySelfCheck.surfaceStateEvidence[].id",
            "frontendQualitySelfCheck.surfaceQualityRuleEvidence[].id",
            "runtimeDeliveryEvidence.requirementRef",
            "runtimeDeliveryEvidence.checkedFields",
            "runtimeDeliveryEvidence.codeLevelChecks[].checkId",
            "runtimeDeliveryEvidence.codeLevelChecks[].contractField",
            "architectureQualityEvidence[].requirementId",
            "architectureQualityEvidence[].verificationIds",
            "apiContractEvidence[].requirementId",
            "apiContractEvidence[].interfaceRefs",
            "apiContractEvidence[].verificationIds",
            "codeQualityEvidence[].requirementId",
            "codeQualityEvidence[].verificationIds",
        ],
        "review_result" => &[
            "schemaVersion",
            "reviewId",
            "source",
            "createdAt",
            "updatedAt",
            "findings[].findingId",
            "pendingActions[].findingRefs",
            "nextAction.targetTaskIds",
            "nextAction.findingRefs",
            "nextAction.targetPhaseId",
            "nextAction.targetNode",
        ],
        "manual_review_resolution" => &[
            "schemaVersion",
            "manualReviewResolutionId",
            "manualReviewRequestId",
            "deliveryId",
            "phaseId",
            "createdAt",
            "nextAction.targetTaskIds",
            "nextAction.findingRefs",
            "nextAction.targetPhaseId",
        ],
        "technical_baseline_candidate" => &["projectKind", "source"],
        "deploy_execution_repair_result" => &["schemaVersion", "repairId", "deploymentFailureRef"],
        _ => &[],
    }
}

fn domain_validation_paths_for_artifact(artifact_kind: &str) -> &'static [&'static str] {
    match artifact_kind {
        "architecture_section_candidate" => &["content"],
        "task_plan_candidate"
        | "task_result"
        | "task_result_repair"
        | "review_result"
        | "manual_review_resolution"
        | "taskplan_repair"
        | "architecture_artifact_repair"
        | "deploy_execution_repair_result" => &["$"],
        _ => &[],
    }
}

fn validation_contract_for_target(output_contract: &Value, target_id: &str) -> Option<Value> {
    let projected = output_contract
        .pointer(&format!(
            "/schemaProjection/fieldContractByTarget/{target_id}"
        ))
        .or_else(|| output_contract.pointer("/schemaProjection/fieldContract"))?;
    let schema_shape = output_contract
        .get(&format!("{target_id}SchemaShape"))
        .or_else(|| output_contract.get("schemaShape"));
    let mut contract = schema_shape
        .filter(|value| value.is_object())
        .map(|schema| {
            compact_agent_field_contract_with_required(
                schema,
                &required_top_level_fields(output_contract),
                &BTreeMap::new(),
                None,
            )
        })
        .unwrap_or_else(|| projected.clone());
    merge_contract_nodes(&mut contract, projected);

    // The agent-facing projection is intentionally sparse to keep read groups
    // small. The result template is the same contract's concrete tree and
    // supplies nested fields that schemars omitted from the compact projection
    // (including fields intentionally skipped from the Rust schema metadata).
    if let Some(template) = output_contract
        .get("resultTemplate")
        .filter(|value| value.is_object())
    {
        let template_contract =
            compact_manual_node(template, "candidate", false, &BTreeMap::new(), 0, None);
        merge_contract_nodes(&mut contract, &template_contract);
    }
    Some(contract)
}

fn required_top_level_fields(output_contract: &Value) -> Vec<String> {
    output_contract
        .pointer("/schemaProjection/requiredTopLevelFields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn merge_contract_nodes(base: &mut Value, supplement: &Value) {
    let (Some(base_object), Some(supplement_object)) =
        (base.as_object_mut(), supplement.as_object())
    else {
        return;
    };

    for key in [
        "owner",
        "applicability",
        "emptyPolicy",
        "preserveOnRepair",
        "constraints",
    ] {
        if let Some(value) = supplement_object.get(key) {
            base_object.insert(key.to_string(), value.clone());
        }
    }

    if base_object
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "unknown")
    {
        if let Some(kind) = supplement_object.get("type") {
            base_object.insert("type".to_string(), kind.clone());
        }
    }
    if supplement_object
        .get("nullable")
        .and_then(Value::as_bool)
        .is_some_and(|nullable| nullable)
    {
        base_object.insert("nullable".to_string(), Value::Bool(true));
    }

    if let Some(supplement_properties) = supplement_object
        .get("properties")
        .and_then(Value::as_object)
    {
        let base_properties = base_object
            .entry("properties".to_string())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("contract properties must be an object");
        for (name, supplement_child) in supplement_properties {
            match base_properties.get_mut(name) {
                Some(base_child) => merge_contract_nodes(base_child, supplement_child),
                None => {
                    base_properties.insert(name.clone(), supplement_child.clone());
                }
            }
        }
    }

    if !base_object.contains_key("items") {
        if let Some(items) = supplement_object.get("items") {
            base_object.insert("items".to_string(), items.clone());
        }
    } else if let (Some(base_items), Some(supplement_items)) =
        (base_object.get_mut("items"), supplement_object.get("items"))
    {
        merge_contract_nodes(base_items, supplement_items);
    }
}

fn validate_contract_node(
    value: &Value,
    contract: &Value,
    path: &str,
    root: bool,
    target_id: &str,
    mcp_normalized_fields: &BTreeSet<String>,
    domain_validation_paths: &BTreeSet<String>,
    issues: &mut Vec<RepairIssue>,
) {
    if is_mcp_normalized_field(path, mcp_normalized_fields) {
        return;
    }
    if contract
        .get("applicability")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "not_applicable")
        && !value.is_null()
    {
        issues.push(RepairIssue {
            code: "WRITE_CONTRACT_FIELD_NOT_APPLICABLE".to_string(),
            message: format!(
                "{path} is not applicable to the current request and must be omitted."
            ),
            target_id: Some(target_id.to_string()),
            field_path: Some(path.to_string()),
        });
        return;
    }
    let nullable = contract
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if value.is_null() && nullable {
        return;
    }
    let expected_type = contract.get("type").and_then(Value::as_str);
    if let Some(expected_type) = expected_type.filter(|kind| *kind != "unknown") {
        let valid = match expected_type {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ if expected_type.starts_with("object:") => value.is_object(),
            _ => true,
        };
        if !valid {
            issues.push(RepairIssue {
                code: "WRITE_CONTRACT_TYPE_INVALID".to_string(),
                message: format!("{path} must be a {expected_type}."),
                target_id: Some(target_id.to_string()),
                field_path: Some(path.to_string()),
            });
            return;
        }
    }
    if let Some(enum_values) = contract.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|item| item == value) {
            issues.push(RepairIssue {
                code: "WRITE_CONTRACT_ENUM_INVALID".to_string(),
                message: format!(
                    "{path} must use one of the values declared by the current write contract."
                ),
                target_id: Some(target_id.to_string()),
                field_path: Some(path.to_string()),
            });
        }
    }
    if is_domain_validation_path(path, domain_validation_paths) {
        return;
    }
    if let (Some(object), Some(properties)) = (
        value.as_object(),
        contract.get("properties").and_then(Value::as_object),
    ) {
        if let Some(required) = contract.get("required").and_then(Value::as_array) {
            for field in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(field) {
                    issues.push(RepairIssue {
                        code: "WRITE_CONTRACT_FIELD_REQUIRED".to_string(),
                        message: format!(
                            "{path}.{field} is required by the current write contract."
                        ),
                        target_id: Some(target_id.to_string()),
                        field_path: Some(format!("{path}.{field}")),
                    });
                }
            }
        }
        for (field, field_value) in object {
            let child_path = format!("{path}.{field}");
            let Some(child_contract) = properties.get(field) else {
                if (root && crate::MACHINE_OWNED_FIELDS.contains(&field.as_str()))
                    || is_mcp_normalized_field(&child_path, mcp_normalized_fields)
                {
                    continue;
                }
                issues.push(RepairIssue {
                    code: "WRITE_CONTRACT_FIELD_UNKNOWN".to_string(),
                    message: format!("{child_path} is not declared by the current write contract."),
                    target_id: Some(target_id.to_string()),
                    field_path: Some(child_path),
                });
                continue;
            };
            validate_contract_node(
                field_value,
                child_contract,
                &child_path,
                false,
                target_id,
                mcp_normalized_fields,
                domain_validation_paths,
                issues,
            );
        }
    }
    if let (Some(items), Some(item_contract)) = (value.as_array(), contract.get("items")) {
        for (index, item) in items.iter().enumerate() {
            validate_contract_node(
                item,
                item_contract,
                &format!("{path}.{index}"),
                false,
                target_id,
                mcp_normalized_fields,
                domain_validation_paths,
                issues,
            );
        }
    }
}

fn is_mcp_normalized_field(path: &str, fields: &BTreeSet<String>) -> bool {
    let mut normalized: Vec<String> = Vec::new();
    for part in path.strip_prefix("candidate.").unwrap_or(path).split('.') {
        if part.parse::<usize>().is_ok() {
            if let Some(previous) = normalized.last_mut() {
                previous.push_str("[]");
            }
        } else {
            normalized.push(part.to_string());
        }
    }
    fields.contains(&normalized.join("."))
}

fn is_domain_validation_path(path: &str, paths: &BTreeSet<String>) -> bool {
    if paths.contains("$") {
        return true;
    }
    let normalized = path.strip_prefix("candidate.").unwrap_or(path);
    paths.iter().any(|candidate| {
        normalized == candidate || normalized.starts_with(&format!("{candidate}."))
    })
}

fn compact_schema_node(
    schema: &Value,
    root_schema: &Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
    depth: usize,
    sparse_paths: Option<&BTreeSet<String>>,
) -> Value {
    if let Some(shape) = schema.get("shape") {
        return compact_manual_node(shape, path, required, policies, depth, sparse_paths);
    }
    let nullable = schema_allows_null(schema, root_schema, 0);
    let schema = if depth < 32 {
        resolved_schema(schema, root_schema, 0)
    } else {
        schema
    };
    let mut node = Map::new();
    let schema_kind = schema_type(schema);
    node.insert("type".to_string(), json!(schema_kind));
    if nullable {
        node.insert("nullable".to_string(), json!(true));
    }
    if required {
        node.insert("required".to_string(), json!(true));
    }
    let policy = policies
        .get(path)
        .or_else(|| path.strip_suffix("[]").and_then(|base| policies.get(base)))
        .cloned()
        .unwrap_or_else(|| {
            let mut policy = AgentFieldPolicy::default();
            if required {
                policy.applicability = AgentFieldApplicability::CurrentRequest;
            }
            policy
        });
    add_compact_policy_fields(&mut node, &policy);
    if let Some(enum_values) = schema.get("enum") {
        node.insert("enum".to_string(), enum_values.clone());
    }
    if schema.get("type").and_then(Value::as_str) == Some("object") {
        if let Some(required_fields) = schema.get("required") {
            node.insert("requiredFields".to_string(), required_fields.clone());
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            let nested = properties
                .iter()
                .filter(|(name, _)| {
                    sparse_paths
                        .is_none_or(|paths| path_is_interesting(&format!("{path}.{name}"), paths))
                })
                .map(|(name, child)| {
                    let child_path = format!("{path}.{name}");
                    (
                        name.clone(),
                        compact_schema_node(
                            child,
                            root_schema,
                            &child_path,
                            schema
                                .get("required")
                                .and_then(Value::as_array)
                                .is_some_and(|items| {
                                    items.iter().any(|item| item.as_str() == Some(name))
                                }),
                            policies,
                            depth + 1,
                            sparse_paths,
                        ),
                    )
                })
                .collect::<Map<_, _>>();
            node.insert("properties".to_string(), Value::Object(nested));
        }
    }
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get("items") {
            node.insert(
                "items".to_string(),
                compact_schema_node(
                    items,
                    root_schema,
                    &format!("{path}[]"),
                    true,
                    policies,
                    depth + 1,
                    sparse_paths,
                ),
            );
        }
    }
    Value::Object(node)
}

fn resolved_schema<'a>(schema: &'a Value, root_schema: &'a Value, depth: usize) -> &'a Value {
    if depth >= 32 {
        return schema;
    }
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/"))
        .and_then(|reference| root_schema.pointer(&format!("/{reference}")))
    {
        return resolved_schema(reference, root_schema, depth + 1);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            if let Some(variant) = variants
                .iter()
                .find(|variant| variant.get("type").and_then(Value::as_str) != Some("null"))
            {
                return resolved_schema(variant, root_schema, depth + 1);
            }
        }
    }
    schema
}

fn schema_allows_null(schema: &Value, root_schema: &Value, depth: usize) -> bool {
    if depth >= 32 {
        return false;
    }
    if schema.get("type").and_then(Value::as_str) == Some("null") {
        return true;
    }
    if schema
        .get("type")
        .and_then(Value::as_array)
        .is_some_and(|types| types.iter().any(|item| item.as_str() == Some("null")))
    {
        return true;
    }
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/"))
        .and_then(|reference| root_schema.pointer(&format!("/{reference}")))
    {
        return schema_allows_null(reference, root_schema, depth + 1);
    }
    ["anyOf", "oneOf"]
        .iter()
        .filter_map(|key| schema.get(*key).and_then(Value::as_array))
        .flatten()
        .any(|variant| schema_allows_null(variant, root_schema, depth + 1))
}

fn compact_manual_node(
    shape: &Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
    depth: usize,
    sparse_paths: Option<&BTreeSet<String>>,
) -> Value {
    if depth > 32 {
        return json!({"type": "unknown", "required": required});
    }
    let mut node = Map::new();
    let (kind, nested_shape) = match shape {
        Value::Object(object) => ("object", Some(Value::Object(object.clone()))),
        Value::Array(items) => (
            "array",
            items.first().cloned().map(|item| Value::Array(vec![item])),
        ),
        Value::Bool(_) => ("boolean", None),
        Value::Number(_) => ("number", None),
        Value::String(description) => {
            if let Some(base_type) = description.trim().strip_suffix(" or null").map(str::trim) {
                let kind = matches!(
                    base_type,
                    "object" | "string" | "boolean" | "number" | "array"
                )
                .then_some(base_type)
                .unwrap_or("unknown");
                let value = json!({"type": kind, "nullable": true, "required": required});
                return add_policy(value, path, required, policies);
            }
            if matches!(
                description.trim(),
                "object" | "string" | "boolean" | "number" | "array"
            ) {
                return add_policy(
                    json!({"type": description.trim(), "required": required}),
                    path,
                    required,
                    policies,
                );
            }
            let mut value = json!({"type": "string", "required": required});
            if description.contains(" | ") {
                value["enum"] = Value::Array(
                    description
                        .split(" | ")
                        .map(|item| json!(item.trim()))
                        .collect(),
                );
            } else if !description.is_empty() {
                value["description"] = json!(description);
            }
            return add_policy(value, path, required, policies);
        }
        Value::Null => ("null", None),
    };
    node.insert("type".to_string(), json!(kind));
    if required {
        node.insert("required".to_string(), json!(true));
    }
    let policy = policies.get(path).cloned().unwrap_or_else(|| {
        if required {
            AgentFieldPolicy {
                applicability: AgentFieldApplicability::CurrentRequest,
                ..AgentFieldPolicy::default()
            }
        } else {
            AgentFieldPolicy::default()
        }
    });
    add_compact_policy_fields(&mut node, &policy);
    match kind {
        "object" => {
            if let Some(Value::Object(properties)) = nested_shape {
                let nested = properties
                    .iter()
                    .filter(|(name, _)| {
                        sparse_paths.is_none_or(|paths| {
                            path_is_interesting(&format!("{path}.{name}"), paths)
                        })
                    })
                    .map(|(name, child)| {
                        (
                            name.clone(),
                            compact_manual_node(
                                child,
                                &format!("{path}.{name}"),
                                false,
                                policies,
                                depth + 1,
                                sparse_paths,
                            ),
                        )
                    })
                    .collect::<Map<_, _>>();
                node.insert("properties".to_string(), Value::Object(nested));
            }
        }
        "array" => {
            if let Some(Value::Array(items)) = nested_shape {
                if let Some(item) = items.first() {
                    node.insert(
                        "items".to_string(),
                        compact_manual_node(
                            item,
                            &format!("{path}[]"),
                            true,
                            policies,
                            depth + 1,
                            sparse_paths,
                        ),
                    );
                }
            }
        }
        _ => {}
    }
    Value::Object(node)
}

fn path_is_interesting(path: &str, paths: &BTreeSet<String>) -> bool {
    paths.iter().any(|candidate| {
        candidate == path
            || candidate.starts_with(&format!("{path}."))
            || candidate.starts_with(&format!("{path}[]"))
    })
}

fn add_policy(
    mut value: Value,
    path: &str,
    required: bool,
    policies: &BTreeMap<String, AgentFieldPolicy>,
) -> Value {
    let policy = policies.get(path).cloned().unwrap_or_else(|| {
        if required {
            AgentFieldPolicy {
                applicability: AgentFieldApplicability::CurrentRequest,
                ..AgentFieldPolicy::default()
            }
        } else {
            AgentFieldPolicy::default()
        }
    });
    if let Some(object) = value.as_object_mut() {
        add_compact_policy_fields(object, &policy);
    }
    value
}

fn add_compact_policy_fields(node: &mut Map<String, Value>, policy: &AgentFieldPolicy) {
    if policy.owner != AgentFieldOwner::Agent {
        node.insert("owner".to_string(), json!(policy.owner));
    }
    if policy.applicability != AgentFieldApplicability::Unknown {
        node.insert("applicability".to_string(), json!(policy.applicability));
    }
    if policy.empty_policy != "follow_schema_and_applicability" {
        node.insert("emptyPolicy".to_string(), json!(policy.empty_policy));
    }
    if !policy.preserve_on_repair {
        node.insert("preserveOnRepair".to_string(), json!(false));
    }
}

fn apply_shape_rule_constraints(field_contract: &mut Value, shape_rules: &Value) {
    let Some(rules) = shape_rules.as_object() else {
        return;
    };
    for (path, rule) in rules {
        let Some(rule) = rule.as_str() else {
            continue;
        };
        let Some(node) = field_contract_node_mut(field_contract, path) else {
            continue;
        };
        node.as_object_mut()
            .expect("field contract node is an object")
            .entry("constraints".to_string())
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .expect("field contract constraints are an array")
            .push(json!(rule));
    }
}

fn field_contract_node_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut current = root.get_mut("properties").and_then(Value::as_object_mut)?;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        let is_array = part.ends_with("[]");
        let name = part.strip_suffix("[]").unwrap_or(part);
        let node = current.get_mut(name)?;
        if parts.peek().is_none() {
            return Some(node);
        }
        if is_array {
            current = node
                .get_mut("items")
                .and_then(Value::as_object_mut)
                .and_then(|items| items.get_mut("properties"))
                .and_then(Value::as_object_mut)?;
        } else {
            current = node.get_mut("properties").and_then(Value::as_object_mut)?;
        }
    }
    None
}

fn policy_for(applicability: AgentFieldApplicability) -> AgentFieldPolicy {
    AgentFieldPolicy {
        applicability,
        ..AgentFieldPolicy::default()
    }
}

fn schema_type(schema: &Value) -> String {
    schema
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            schema
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.rsplit('/').next())
                .map(|name| format!("object:{name}"))
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn remove_volatile_contract_fields(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for field in [
        "contractFingerprint",
        "writeTargets",
        "resultFile",
        "candidateFile",
        "outlineFile",
        "groupFilePattern",
        "path",
    ] {
        object.remove(field);
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_manual_shapes_accept_null_without_relaxing_object_values() {
        let schema = json!({
            "type": "object",
            "properties": {
                "failure": {
                    "shape": "object or null"
                }
            }
        });
        let contract = compact_agent_field_contract(&schema, &BTreeMap::new());
        let failure_contract = contract
            .pointer("/properties/failure")
            .expect("failure contract");
        assert_eq!(failure_contract["type"], json!("object"));
        assert_eq!(failure_contract["nullable"], json!(true));

        let mut issues = Vec::new();
        validate_contract_node(
            &Value::Null,
            failure_contract,
            "candidate.failure",
            false,
            "candidate",
            &BTreeSet::new(),
            &BTreeSet::new(),
            &mut issues,
        );
        assert!(
            issues.is_empty(),
            "nullable null should be valid: {issues:#?}"
        );

        let mut issues = Vec::new();
        validate_contract_node(
            &json!({"code": "VERIFICATION_FAILED"}),
            failure_contract,
            "candidate.failure",
            false,
            "candidate",
            &BTreeSet::new(),
            &BTreeSet::new(),
            &mut issues,
        );
        assert!(
            issues.is_empty(),
            "nullable object should remain valid: {issues:#?}"
        );
    }

    #[test]
    fn nullable_schema_variants_are_preserved_in_agent_projection() {
        let schema = json!({
            "type": "object",
            "properties": {
                "noChangeReason": {
                    "anyOf": [
                        {"type": "object", "properties": {"summary": {"type": "string"}}},
                        {"type": "null"}
                    ]
                }
            }
        });
        let contract = compact_agent_field_contract(&schema, &BTreeMap::new());
        let reason_contract = contract
            .pointer("/properties/noChangeReason")
            .expect("no-change contract");
        assert_eq!(reason_contract["type"], json!("object"));
        assert_eq!(reason_contract["nullable"], json!(true));
    }
}
