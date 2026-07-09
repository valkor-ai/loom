use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
};

use delivery_core::{
    DomainDispatcher, InspectRequestInput, ReadFieldGroupInput, ReadRequestFieldsInput,
    RouteAction, RouteActionKind,
};
use mcp_server::server::LoomMcpServer;
use serde_json::{json, Value};
use state::{store::write_json_atomic, write_native_request, NativeRequestInput};

#[test]
fn submit_tool_returns_repairable_error_for_missing_target_file() {
    let fixture = Fixture::new("submit-missing-target");
    let stored = write_brainstorm_request(&fixture, "req_submit_missing", false);
    let result = call_submit(
        "loom.brainstormAcceptFile",
        &stored.request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error");
    assert_eq!(result["targetFile"], ".loom/agent-writable/candidate.json");
    assert_eq!(result["targetIds"], json!(["candidate"]));
    assert_eq!(result["issues"][0]["code"], "TARGET_MISSING");
    assert_eq!(result["resubmitTool"], "brainstormAcceptFile");
    assert!(result.get("submitCommand").is_none());
}

#[test]
fn brainstorm_clarification_request_has_no_candidate_write_contract() {
    let fixture = Fixture::new("submit-user-gate");
    let request_ref = start_brainstorm_request(&fixture);
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.clone(),
    })
    .expect("inspect clarification request");

    assert_eq!(inspected.request_kind, "brainstorm_clarification_block");
    assert!(inspected.submit_tool.is_none());
    assert!(inspected.write_targets.is_empty());
    assert!(!inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "candidate_write_contract"));
}

#[test]
fn brainstorm_phase_scope_rejects_single_wide_capability_closure_query() {
    let fixture = Fixture::new("brainstorm-phase-scope-single-wide-knowledge");
    let server = LoomMcpServer::default();
    let request_ref = start_brainstorm_request(&fixture);

    for (step_id, query_id) in [
        ("phase_scope_dependency_order", None),
        (
            "phase_scope_capability_closure",
            Some("capability_closure_A"),
        ),
    ] {
        server
            .invoke_tool(
                "loom.knowledgeBrainstormContext",
                Some(
                    json!({
                        "projectRoot": fixture.root_str(),
                        "requestRef": request_ref,
                        "block": "phase_scope",
                        "stepId": step_id,
                        "queryId": query_id,
                        "querySubject": "证券账户阶段边界",
                        "naturalLanguageQuery": "证券账户 开户 挂失 补办 销户 资金账户 交易 依赖 闭环",
                        "semanticFocus": ["证券账户", "开户", "挂失", "补办", "销户", "资金账户", "交易"]
                    })
                    .as_object()
                    .expect("arguments object")
                    .clone(),
                ),
            )
            .expect("knowledge brainstorm context");
    }

    let result = server
        .invoke_tool(
            "loom.brainstormConfirmBlock",
            Some(
                json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "block": "phase_scope",
                    "summary": "确认第一阶段为证券账户模块闭环。",
                    "confirmedData": {
                        "scope": {
                            "included": ["证券账户开户", "证券账户挂失补办", "证券账户销户"],
                            "deferred": ["资金账户", "交易客户端", "中央撮合"],
                            "excluded": []
                        },
                        "recommendation": {
                            "label": "证券账户模块闭环",
                            "reason": "证券账户是资金账户和交易链路的上游基础对象。"
                        }
                    }
                })
                .as_object()
                .expect("arguments object")
                .clone(),
            ),
        )
        .expect("confirm brainstorm block")
        .structured_content
        .expect("structured content");

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "run_loom_tool", "{result:#}");
    assert_eq!(
        result["next"]["toolName"], "loom.knowledgeBrainstormContext",
        "{result:#}"
    );
    assert_eq!(
        result["next"]["retryTool"], "loom.brainstormConfirmBlock",
        "{result:#}"
    );
    let read_groups = result["next"]["readGroups"]
        .as_array()
        .expect("read groups");
    assert_eq!(read_groups.len(), 1, "{result:#}");
    assert_eq!(read_groups[0]["groupId"], "knowledge_context_plan");
    let message = result["agentInstruction"].as_str().expect("message");
    assert!(
        message.contains("phase_scope_capability_closure"),
        "{message}"
    );
    assert!(
        message.contains("once per candidate phase boundary"),
        "{message}"
    );
    assert!(
        message.contains("Do not ask the user to reconfirm"),
        "repair must not ask the user to reconfirm: {message}"
    );
}

#[test]
fn brainstorm_submit_returns_repairable_error_for_schema_invalid_candidate() {
    let fixture = Fixture::new("submit-schema-invalid");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(
        &fixture,
        &request_ref,
        &json!({
            "requestSummary": {
                "title": "坏结构",
                "oneLine": "坏结构",
                "complexity": "small"
            },
            "clarificationProgress": "broken"
        }),
    );

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error");
    assert_eq!(
        result["issues"][0]["code"],
        "CLARIFICATION_PROGRESS_INVALID"
    );
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
}

#[test]
fn brainstorm_submit_repairs_legacy_progress_shape_instead_of_reopening_phase_gate() {
    let fixture = Fixture::new("submit-legacy-progress-shape");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let mut candidate = valid_candidate_json();
    candidate["clarificationProgress"] = json!({
        "mode": "progressive_blocks",
        "completedBlocks": [
            "phase_scope",
            "concept_grounding",
            "frontend_experience",
            "final_summary"
        ],
        "currentBlock": "final_summary",
        "finalSummaryConfirmed": true
    });
    write_candidate_target(&fixture, &request_ref, &candidate);

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(
        result["issues"][0]["code"],
        "CLARIFICATION_PROGRESS_LEGACY_FIELDS"
    );
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
}

#[test]
fn brainstorm_submit_accepts_valid_candidate_and_hands_off_to_batch_eight() {
    let fixture = Fixture::new("submit-success");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "technical_baseline_candidate"
    );
    assert_eq!(
        result["next"]["submitTool"],
        "loom.technicalBaselineAcceptFile"
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("brainstorm/contract.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("brainstorm/phases/phase-1/decision-snapshot.json")
        .exists());
    let technical_baseline_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("technical baseline requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
    })
    .expect("inspect technical baseline request");
    assert_eq!(inspected.request_kind, "technical_baseline_request");
    let baseline_context = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
        group_id: "technical_baseline_context".to_string(),
    })
    .expect("read technical baseline context");
    assert!(baseline_context
        .fields
        .get("brainstormLens.acceptance")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.domainModel")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.roadmap")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.roadmap.phases")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.phasePlan")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.scope")
        .is_none());
    assert!(baseline_context
        .fields
        .get("brainstormLens.frontendExperience")
        .is_none());
    for field in [
        "brainstormLens.scope.included",
        "brainstormLens.scope.deferred",
        "brainstormLens.scope.excluded",
        "brainstormLens.scope.assumptions",
        "brainstormLens.roadmapPhaseIndex",
        "brainstormLens.phasePlan.nextPhasePreview",
        "brainstormLens.domainModel.capabilityGroups",
        "brainstormLens.domainModel.businessFlows",
        "brainstormLens.frontendExperience.required",
        "brainstormLens.frontendExperience.surfaces",
        "brainstormLens.frontendExperience.operationPaths",
    ] {
        assert!(
            baseline_context.fields.get(field).is_none(),
            "technical baseline context must not expose broad source field: {field}"
        );
    }
    for field in [
        "brainstormLens.summary.title",
        "brainstormLens.summary.oneLine",
        "brainstormLens.summary.businessGoal",
        "brainstormLens.scopeIndex.includedIds",
        "brainstormLens.scopeIndex.includedLabels",
        "brainstormLens.scopeIndex.deferredIds",
        "brainstormLens.scopeIndex.deferredLabels",
        "brainstormLens.scopeIndex.excludedIds",
        "brainstormLens.scopeIndex.excludedLabels",
        "brainstormLens.scopeIndex.assumptionTexts",
        "brainstormLens.roadmapSignal.required",
        "brainstormLens.roadmapSignal.currentPhaseId",
        "brainstormLens.roadmapSignal.phaseIds",
        "brainstormLens.roadmapSignal.phaseTitles",
        "brainstormLens.roadmapSignal.phaseGoals",
        "brainstormLens.roadmapSignal.nextPhasePreview.kind",
        "brainstormLens.roadmapSignal.nextPhasePreview.reason",
        "currentPhaseLens.phaseId",
        "currentPhaseLens.goal",
    ] {
        assert!(
            baseline_context.fields.get(field).is_some(),
            "missing field-level baseline context {field}"
        );
    }
    for field in [
        "brainstormLens.domainModel.capabilityNames",
        "brainstormLens.domainModel.businessFlowNames",
        "brainstormLens.frontendTarget.required",
        "brainstormLens.frontendTarget.surfaceNames",
    ] {
        assert!(
            baseline_context.fields.get(field).is_none(),
            "optional absent baseline context field should not be exposed: {field}"
        );
    }
    assert!(baseline_context.fields["brainstormLens.acceptanceIndex"].value[0]["id"].is_string());
    assert!(
        baseline_context.fields["brainstormLens.acceptanceIndex"].value[0]
            .get("statement")
            .is_none()
    );
    let selection_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "technical_baseline_selection_guidance")
        .expect("technical baseline selection guidance group");
    assert!(selection_group.required);
    assert_eq!(
        selection_group.expanded_fields(),
        vec!["selectionGuidance".to_string()]
    );
    let selection = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: technical_baseline_request_ref.to_string(),
        group_id: "technical_baseline_selection_guidance".to_string(),
    })
    .expect("read technical baseline selection guidance");
    let guidance = &selection.fields["selectionGuidance"].value;
    assert!(guidance["trackModel"]["coreTracks"]
        .as_array()
        .expect("core tracks")
        .contains(&json!("backend")));
    assert!(guidance["commonOptions"]["backend"]["examples"]
        .as_array()
        .expect("backend examples")
        .contains(&json!("Java + Spring Boot")));
    assert!(guidance["commonOptions"]["backend"]["examples"]
        .as_array()
        .expect("backend examples")
        .contains(&json!("Python + FastAPI")));
    assert!(guidance["commonOptions"]["dataAccess"]["examples"]
        .as_array()
        .expect("data access examples")
        .contains(&json!("MyBatis Plus")));
    assert!(guidance["shorthandNormalization"]["backend"]
        .as_array()
        .expect("backend shorthand rules")
        .iter()
        .any(
            |rule| rule.as_str().unwrap_or_default().contains("backend=Java")
                && rule
                    .as_str()
                    .unwrap_or_default()
                    .contains("Java + Spring Boot")
        ));
    assert!(
        guidance["userFacingConfirmationProtocol"]["mandatorySections"]
            .as_array()
            .expect("mandatory sections")
            .iter()
            .any(|section| section
                .as_str()
                .unwrap_or_default()
                .contains("Adjustable technology range"))
    );
    assert!(guidance["replyProtocolForUser"]["partialAdjustmentExample"]
        .as_str()
        .expect("partial adjustment example")
        .contains("persistence=PostgreSQL"));
    assert!(guidance["userFacingConfirmationProtocol"]["wordingRules"]
        .as_array()
        .expect("wording rules")
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("Do not mention Loom internals")));
}

#[test]
fn continue_reuses_same_technical_baseline_request_after_brainstorm_accept() {
    let fixture = Fixture::new("continue-technical-baseline");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let submitted = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let continued = continue_delivery(fixture.root_str());

    assert_eq!(submitted["state"], "auto_runnable");
    assert_eq!(continued["state"], "auto_runnable");
    assert_eq!(
        continued["next"]["requestRef"],
        submitted["next"]["requestRef"]
    );
    assert_eq!(
        continued["next"]["artifactKind"],
        "technical_baseline_candidate"
    );
}

#[test]
fn technical_baseline_accept_routes_existing_project_to_repository_context() {
    let fixture = Fixture::new("technical-baseline-existing-project");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({
            "name": "loom-fixture",
            "private": true,
            "scripts": {
                "build": "vite build",
                "test": "vitest run"
            },
            "dependencies": {
                "react": "^19.0.0",
                "vite": "^6.0.0"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }),
    )
    .expect("write package.json");
    write_json_atomic(
        &fixture.root.join("tsconfig.json"),
        &json!({ "compilerOptions": { "strict": true } }),
    )
    .expect("write tsconfig.json");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let inspected_baseline = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: baseline_request_ref.clone(),
    })
    .expect("inspect technical baseline request");
    let repo_evidence_group = inspected_baseline
        .read_groups
        .iter()
        .find(|group| group.group_id == "technical_baseline_repo_evidence")
        .expect("technical baseline repo evidence group");
    assert!(!repo_evidence_group
        .expanded_fields()
        .contains(&"repoEvidence".to_string()));
    for field in [
        "repoEvidence.signals.manifests",
        "repoEvidence.signals.packageManagers",
        "repoEvidence.signals.languages",
        "repoEvidence.signals.frameworks",
    ] {
        assert!(
            repo_evidence_group
                .expanded_fields()
                .contains(&field.to_string()),
            "missing compact repo signal field {field}"
        );
    }
    let repo_evidence = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: baseline_request_ref.clone(),
        group_id: "technical_baseline_repo_evidence".to_string(),
    })
    .expect("read technical baseline repo evidence");
    assert_eq!(
        repo_evidence.fields["repoEvidence.signals.packageManagers"].value,
        json!(["npm"])
    );
    assert!(repo_evidence.fields["repoEvidence.signals.languages"]
        .value
        .as_array()
        .expect("languages")
        .contains(&json!("TypeScript")));
    assert!(repo_evidence.fields["repoEvidence.signals.frameworks"]
        .value
        .as_array()
        .expect("frameworks")
        .contains(&json!("React")));
    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "repository_context_candidate"
    );
    assert_eq!(
        result["next"]["submitTool"],
        "loom.repositoryContextAcceptFile"
    );
    let repository_context_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repository context requestRef");
    let scan_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_scan_contract".to_string(),
    })
    .expect("read repository context scan contract");
    assert_eq!(
        scan_contract.fields["repositoryMode"].value,
        json!("existing_project")
    );
    assert_eq!(
        scan_contract.fields["phaseDevelopmentMode"].value,
        json!("initial_delivery")
    );
    assert!(
        !scan_contract
            .fields
            .contains_key("scanPurpose.completedPhaseSummaries"),
        "initial repository context request must not expose empty completed phase summaries"
    );
    let generation_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_generation_rules".to_string(),
    })
    .expect("read repository context generation rules");
    for field in [
        "enumRefs.repositoryShape",
        "enumRefs.capabilityStatus",
        "enumRefs.surfaceRelevance",
        "enumRefs.suggestedUse",
        "enumRefs.contextCoverage",
        "enumRefs.confidence",
    ] {
        assert!(
            generation_rules.fields.get(field).is_some(),
            "missing RepositoryContext enum field {field}"
        );
    }
    assert_eq!(
        generation_rules.fields["enumRefs.contextCoverage"].value,
        json!(["focused", "partial", "broad", "insufficient"])
    );
    assert_eq!(
        generation_rules.fields["enumRefs.surfaceRelevance"].value,
        json!([
            "implemented_capability",
            "architecture_boundary",
            "extension_point",
            "validation_surface",
            "delivery_context",
            "unrelated"
        ])
    );
    let generation_rule_text = generation_rules.fields["generationRules"].value.to_string();
    assert!(
        generation_rule_text.contains("integration_boundary is not a surface relevance value"),
        "RepositoryContext rules must prevent surface relevance/read reason enum mixing: {generation_rule_text}"
    );
    let write_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_write_contract".to_string(),
    })
    .expect("read repository context write contract");
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["coverage"],
        "focused"
    );
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["warnings"]
            [0]["code"],
        "LOW_CONFIDENCE_REPOSITORY_SCAN"
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["contextQuality"]["warnings"]
            [0]["message"]
            .as_str()
            .expect("contextQuality warning message")
            .contains("Use [] only when there are no warnings")
    );
    assert_eq!(
        write_contract.fields["outputContract.resultTemplate"].value["warnings"][0]["code"],
        "LOW_CONFIDENCE_REPOSITORY_SCAN"
    );
    assert!(
        write_contract.fields["outputContract.bindingRules"].value[0]
            .as_str()
            .expect("binding rule")
            .contains("source.requestRef")
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["source"]
            .get("requestRef")
            .is_some()
    );
    assert!(
        write_contract.fields["outputContract.resultTemplate"].value["source"]
            .get("requestId")
            .is_none()
    );
    assert_eq!(
        write_contract.fields["outputContract.schemaProjection"].value["enumFields"]
            ["contextQuality.coverage"],
        "enumRefs.contextCoverage"
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["technologySignals"]
            .as_str()
            .expect("technologySignals shape rule")
            .contains("object")
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["contextQuality.warnings[]"]
            .as_str()
            .expect("contextQuality warnings shape rule")
            .contains("code and message")
    );
    assert!(
        write_contract.fields["outputContract.schemaProjection"].value["objectShapeRules"]
            ["warnings[]"]
            .as_str()
            .expect("warnings shape rule")
            .contains("code and message")
    );

    let mut repository_candidate = write_contract.fields["outputContract.resultTemplate"]
        .value
        .clone();
    repository_candidate["source"]["requestRef"] = json!(repository_context_request_ref);
    write_candidate_target(
        &fixture,
        repository_context_request_ref,
        &repository_candidate,
    );
    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "auto_runnable",
        "{repository_result:#}"
    );
    assert_eq!(
        repository_result["next"]["artifactKind"],
        "architecture_section_candidate"
    );
}

#[test]
fn repository_context_accept_normalizes_repairable_schema_metadata() {
    let fixture = Fixture::new("repository-context-normalizes-schema-metadata");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    std::fs::create_dir_all(fixture.root.join("src")).expect("create src");
    std::fs::write(
        fixture.root.join("src/main.tsx"),
        "export const app = true;\n",
    )
    .expect("write entrypoint");

    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());
    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let baseline_result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    let repository_context_request_ref = baseline_result["next"]["requestRef"]
        .as_str()
        .expect("repository context requestRef")
        .to_string();
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let brainstorm_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let technical_baseline_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    let mut repository_candidate = repository_context_candidate_json(
        &repository_context_request_ref,
        &brainstorm_contract_ref,
        &technical_baseline_ref,
    );
    repository_candidate["repoOverview"]["repositoryShape"] = json!("multi_app");
    repository_candidate["relevantSurfaces"][0]["kind"] = json!("api");
    repository_candidate["relevantSurfaces"][0]["relevance"] = json!("integration_boundary");
    repository_candidate["relevantSurfaces"][0]["suggestedUse"] = json!("extend");
    repository_candidate["recommendedReadRefs"][0]["reason"] = json!("architecture_boundary");
    repository_candidate["recommendedReadRefs"][0]["priority"] = json!("urgent");
    repository_candidate["contextQuality"]["coverage"] = json!("complete");
    repository_candidate["contextQuality"]
        .as_object_mut()
        .expect("context quality")
        .remove("confidence");
    repository_candidate["contextQuality"]["warnings"] =
        json!(["Repository scan was intentionally narrow."]);
    repository_candidate["warnings"] = json!(["Top-level warning as text."]);

    write_candidate_target(
        &fixture,
        &repository_context_request_ref,
        &repository_candidate,
    );
    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        &repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "auto_runnable",
        "{repository_result:#}"
    );

    let repository_context_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "latestRepositoryContext");
    let persisted_path = fixture.root.join(repository_context_ref);
    let persisted: Value = serde_json::from_str(
        &std::fs::read_to_string(persisted_path).expect("read persisted repository context"),
    )
    .expect("parse persisted repository context");
    assert_eq!(
        persisted["repoOverview"]["repositoryShape"],
        json!("multi_application")
    );
    assert_eq!(
        persisted["relevantSurfaces"][0]["kind"],
        json!("controller")
    );
    assert_eq!(
        persisted["relevantSurfaces"][0]["relevance"],
        json!("architecture_boundary")
    );
    assert_eq!(
        persisted["relevantSurfaces"][0]["suggestedUse"],
        json!("inspect_or_extend")
    );
    assert_eq!(
        persisted["recommendedReadRefs"][0]["reason"],
        json!("integration_boundary")
    );
    assert_eq!(
        persisted["recommendedReadRefs"][0]["priority"],
        json!("medium")
    );
    assert_eq!(persisted["contextQuality"]["coverage"], json!("broad"));
    assert_eq!(persisted["contextQuality"]["confidence"], json!("unknown"));
    assert_eq!(
        persisted["contextQuality"]["warnings"][0]["message"],
        json!("Repository scan was intentionally narrow.")
    );
    assert_eq!(
        persisted["warnings"][0]["message"],
        json!("Top-level warning as text.")
    );
}

#[test]
fn technical_baseline_conflict_with_previous_baseline_requires_user_gate() {
    let fixture = Fixture::new("technical-baseline-previous-conflict");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    write_previous_technical_baseline(&fixture, &delivery_id);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let previous = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: baseline_request_ref.clone(),
        fields: vec!["previousBaselineContext.previousBaselineRef".to_string()],
    })
    .expect("read previous baseline context");
    assert_eq!(
        previous.fields["previousBaselineContext.previousBaselineRef"].value,
        json!(format!(
            ".loom/deliveries/{delivery_id}/contracts/technical-baseline.json"
        ))
    );

    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let conflict = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    assert_eq!(conflict["state"], "user_gate", "{conflict:#}");
    assert_eq!(
        conflict["gate"]["gateId"],
        "previous_baseline_change_confirmation"
    );

    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "user_confirmed"),
    );
    let confirmed = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    assert_eq!(confirmed["state"], "auto_runnable", "{confirmed:#}");
    assert_eq!(
        confirmed["next"]["artifactKind"],
        "repository_context_candidate"
    );
}

#[test]
fn technical_baseline_ignores_derived_command_changes_for_previous_stack() {
    let fixture = Fixture::new("technical-baseline-derived-commands");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    write_previous_technical_baseline(&fixture, &delivery_id);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = technical_baseline_candidate_json("existing_project", "policy_auto_accept");
    candidate["stack"] = json!({
        "frontend": "plain-html",
        "backend": "none",
        "build": "npm run build --workspace app",
        "test": "npm run test --workspace account-service",
        "start": "npm run start --workspace app"
    });
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"],
        "repository_context_candidate"
    );
}

#[test]
fn technical_baseline_repo_signal_conflict_with_previous_stack_requires_user_gate() {
    let fixture = Fixture::new("technical-baseline-repo-signal-conflict");
    std::fs::write(
        fixture.root.join("pom.xml"),
        "<project><modelVersion>4.0.0</modelVersion></project>\n",
    )
    .expect("write pom.xml");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let baseline_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/technical-baseline.json");
    std::fs::create_dir_all(baseline_path.parent().expect("baseline parent"))
        .expect("create baseline parent");
    write_json_atomic(
        &baseline_path,
        &json!({
            "schemaVersion": "1.0",
            "technicalBaselineId": "tb_previous_node",
            "deliveryId": delivery_id,
            "phaseId": "phase-1",
            "status": "confirmed",
            "source": "user_confirmed",
            "projectKind": "existing_project",
            "scope": "project",
            "stack": {
                "runtime": "node",
                "language": "typescript",
                "framework": "react",
                "packageManager": "npm"
            },
            "constraints": [],
            "evidence": [{
                "path": "package.json",
                "reason": "Previous Node baseline was confirmed earlier."
            }],
            "approval": {
                "type": "user_confirmed",
                "reason": "Existing baseline fixture."
            },
            "confidence": "high",
            "requiresUserConfirmation": false,
            "reasoningSummary": ["Previous Node baseline fixture."],
            "alternatives": [],
            "createdAt": "2026-06-24T09:00:00+08:00",
            "updatedAt": "2026-06-24T09:00:00+08:00"
        }),
    )
    .expect("write previous node baseline");
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = technical_baseline_candidate_json("existing_project", "policy_auto_accept");
    candidate["stack"] = json!({
        "runtime": "node",
        "language": "typescript",
        "framework": "react",
        "packageManager": "npm"
    });
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "user_gate", "{result:#}");
    assert_eq!(
        result["gate"]["gateId"],
        "previous_baseline_change_confirmation"
    );
}

#[test]
fn technical_baseline_request_treats_later_phase_without_repo_markers_as_existing_project() {
    let fixture = Fixture::new("technical-baseline-later-phase");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());
    let phase_1_brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        phase_1_brainstorm_result["next"]["artifactKind"], "technical_baseline_candidate",
        "{phase_1_brainstorm_result:#}"
    );
    write_previous_technical_baseline(&fixture, &delivery_id);
    append_phase_with_refs(
        &fixture,
        &delivery_id,
        "phase-2",
        json!({
            "brainstormContract": format!(".loom/deliveries/{delivery_id}/brainstorm/contract.json")
        }),
    );

    let result = planning::materialize_technical_baseline_request(
        fixture.root_str(),
        &delivery_id,
        "phase-2",
    );
    let value = serde_json::to_value(result).expect("technical baseline request result");
    assert_eq!(value["state"], "auto_runnable", "{value:#}");
    let phase_2_request_ref = value["next"]["requestRef"]
        .as_str()
        .expect("phase-2 technical baseline requestRef");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: phase_2_request_ref.to_string(),
        fields: vec![
            "projectKind".to_string(),
            "repoEvidence.detectedProjectKind".to_string(),
            "repoEvidence.baselineExists".to_string(),
            "previousBaselineContext.previousBaselineRef".to_string(),
            "decisionNeeds".to_string(),
        ],
    })
    .expect("read phase-2 technical baseline fields")
    .fields;

    assert_eq!(fields["projectKind"].value, json!("existing_project"));
    let request_root = read_request_root_value(fixture.root_str(), phase_2_request_ref);
    assert_eq!(
        request_root["operation"],
        json!("infer_existing_project_baseline")
    );
    assert_eq!(
        fields["repoEvidence.detectedProjectKind"].value,
        json!("existing_project")
    );
    assert_eq!(fields["repoEvidence.baselineExists"].value, json!(true));
    assert_eq!(
        fields["previousBaselineContext.previousBaselineRef"].value,
        json!(format!(
            ".loom/deliveries/{delivery_id}/contracts/technical-baseline.json"
        ))
    );
    let decision_needs = fields["decisionNeeds"]
        .value
        .as_array()
        .expect("decision needs");
    assert!(decision_needs.iter().any(|item| item
        .as_str()
        .is_some_and(|text| text.contains("reuse the previous TechnicalBaseline unchanged"))));
}

#[test]
fn new_project_technical_baseline_needing_confirmation_uses_user_gate() {
    let fixture = Fixture::new("technical-baseline-new-project-user-gate");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = new_project_technical_baseline_candidate_json();
    candidate["status"] = json!("needs_user_confirmation");
    candidate["approval"] = json!({
        "type": "none",
        "reason": "The recommended stack still needs user confirmation."
    });
    candidate["requiresUserConfirmation"] = json!(true);
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "user_gate", "{result:#}");
    assert_eq!(
        result["gate"]["gateId"],
        "new_project_baseline_confirmation"
    );
}

#[test]
fn new_project_technical_baseline_autofills_confirmed_at() {
    let fixture = Fixture::new("technical-baseline-new-project-confirmed-at");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = new_project_technical_baseline_candidate_json();
    candidate["approval"]
        .as_object_mut()
        .expect("approval object")
        .remove("confirmedAt");
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &baseline_request_ref);
    let baseline_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(baseline_ref)).unwrap())
            .expect("parse technical baseline");
    assert_eq!(persisted["approval"]["type"], json!("user_confirmed"));
    assert!(persisted["approval"]["confirmedAt"]
        .as_str()
        .is_some_and(|value| !value.trim().is_empty()));
}

#[test]
fn technical_baseline_accepts_legacy_greenfield_project_kind_alias() {
    let fixture = Fixture::new("technical-baseline-greenfield-alias");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = new_project_technical_baseline_candidate_json();
    candidate["projectKind"] = json!("greenfield");
    candidate["source"] = json!("agent_recommended_for_greenfield");
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &baseline_request_ref);
    let baseline_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(baseline_ref)).unwrap())
            .expect("parse technical baseline");
    assert_eq!(persisted["projectKind"], json!("new_project"));
    assert_eq!(
        persisted["source"],
        json!("agent_recommended_for_new_project")
    );
}

#[test]
fn new_project_technical_baseline_requires_complete_track_model() {
    let fixture = Fixture::new("technical-baseline-new-project-tracks");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    let mut candidate = new_project_technical_baseline_candidate_json();
    candidate["stack"] = json!({
        "frontend": "vite-react",
        "backend": "spring-boot",
        "database": "sqlite"
    });
    write_candidate_target(&fixture, &baseline_request_ref, &candidate);

    let result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(
            |issue| issue["code"] == "NEW_PROJECT_BASELINE_TRACKS_INCOMPLETE"
                && issue["fieldPath"] == "stack.tracks"
        ));
}

#[test]
fn repository_context_accept_persists_pgc_and_hands_off_to_architecture() {
    let fixture = Fixture::new("repository-context-pgc");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
    })
    .expect("inspect architecture request");

    assert_eq!(inspected.request_kind, "architecture_sections_generation");
    assert_eq!(
        inspected.write_targets[0]["targetId"],
        json!("foundation"),
        "{inspected:#?}"
    );
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.architectureSectionSubmitFile")
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let planning_contract = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/planning/phase-1/pgc.json");
    assert!(planning_contract.exists());
}

#[test]
fn planning_contract_create_reroutes_existing_project_without_repository_context() {
    let fixture = Fixture::new("pgc-requires-repository-context");
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    write_candidate_target(
        &fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let baseline_result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        baseline_result["next"]["artifactKind"],
        "repository_context_candidate"
    );

    let rerouted = planning::create_contract_and_route(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        workflow::WorkflowDomainDispatcher,
    );
    let rerouted_value = serde_json::to_value(&rerouted).expect("serialize rerouted result");

    assert_eq!(
        rerouted_value["state"], "auto_runnable",
        "{rerouted_value:#}"
    );
    assert_eq!(
        rerouted_value["next"]["artifactKind"],
        "repository_context_candidate"
    );
    assert_eq!(
        rerouted_value["next"]["submitTool"],
        "loom.repositoryContextAcceptFile"
    );
    assert!(!fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/planning/phase-1/pgc.json")
        .exists());
}

#[test]
fn architecture_read_groups_follow_current_section() {
    let fixture = Fixture::new("architecture-read-groups-by-section");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);

    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "frontend_experience");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_frontend_context",
        ],
    );
    let frontend_core_fields = architecture_group_fields(
        &fixture,
        &architecture_request_ref,
        "architecture_core_context",
    );
    assert!(
        !frontend_core_fields.contains(
            &"contextProjection.requirementDetailTransfer.requirementDetails".to_string()
        ),
        "frontend_experience must not inherit broad requirement detail reads"
    );
    assert!(
        !frontend_core_fields
            .contains(&"contextProjection.phaseScope.acceptanceCandidates".to_string()),
        "frontend_experience must rely on its focused frontend context instead of broad acceptance candidates"
    );
    assert_architecture_scope_summary_fields(&frontend_core_fields);
    let frontend_context_fields = architecture_group_fields(
        &fixture,
        &architecture_request_ref,
        "architecture_frontend_context",
    );
    for required in [
        "uiQualitySeed.required",
        "uiQualitySeed.scenarioCandidates",
        "uiQualitySeed.qualityLevel",
        "uiQualitySeed.requiredReferenceGroups",
        "uiQualitySeed.designTokenAssetPlan",
        "uiQualitySeed.requiredUiStates",
        "uiQualitySeed.qualityGatePreview",
        "uiQualitySeed.selectionRule",
    ] {
        assert!(
            frontend_context_fields.contains(&required.to_string()),
            "frontend_experience must expose field-level UI quality seed field {required}"
        );
    }
    let frontend_template =
        architecture_section_contract(&fixture, &architecture_request_ref, "frontend_experience");
    let architecture_output_contract = private_output_contract(&fixture, &architecture_request_ref);
    assert_eq!(
        architecture_output_contract["writeTargets"][0]["targetId"],
        json!("frontend_experience")
    );
    assert!(
        architecture_output_contract["schemaShape"]
            .get("properties")
            .is_none(),
        "architecture outputContract schemaShape must be the current section shape, not the full Rust candidate schema"
    );
    assert!(
        architecture_output_contract["schemaShape"]
            .pointer("/content/frontendExperience/uiQualityContract/referenceProfile")
            .is_none(),
        "architecture outputContract schemaShape must not expose MCP-owned uiQuality referenceProfile"
    );
    let ui_quality_contract = frontend_template["resultTemplate"]["content"]["frontendExperience"]
        ["uiQualityContract"]
        .as_object()
        .expect("uiQualityContract template");
    assert_eq!(
        ui_quality_contract["designTokenAssetPlan"]["strategy"],
        json!("create_css_tokens")
    );
    assert_eq!(
        ui_quality_contract["designTokenAssetPlan"]["templateId"],
        json!("tokens-css")
    );
    assert_eq!(
        ui_quality_contract["designTokenAssetPlan"]["duplicationPolicy"],
        json!("do_not_create_parallel_token_system")
    );
    assert!(
        ui_quality_contract.get("semanticTokenPolicy").is_none(),
        "semanticTokenPolicy is MCP-owned and should not be agent-writable"
    );
    assert!(
        ui_quality_contract.get("referenceProfile").is_none(),
        "referenceProfile is MCP-owned and should not be agent-writable"
    );
    assert!(
        ui_quality_contract.get("qualityGates").is_none(),
        "qualityGates are MCP-owned and should not be agent-writable"
    );
    assert!(
        ui_quality_contract
            .get("forbiddenUserVisibleContent")
            .is_none(),
        "forbiddenUserVisibleContent is MCP-owned and should not be agent-writable"
    );
    assert!(frontend_template["enumRefs"]
        .pointer("/uiQuality/scenarioKind")
        .and_then(Value::as_array)
        .is_some());

    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let runtime_core_fields = architecture_group_fields(
        &fixture,
        &architecture_request_ref,
        "architecture_core_context",
    );
    for forbidden in [
        "contextProjection.phaseScope.included",
        "contextProjection.phaseScope.deferred",
        "contextProjection.phaseScope.excluded",
        "contextProjection.phaseScope.acceptanceCandidates",
        "contextProjection.requirementDetailTransfer.requirementDetails",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.businessFlows",
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.requirementDetailIds",
    ] {
        assert!(
            !runtime_core_fields.contains(&forbidden.to_string()),
            "runtime_delivery must not read non-runtime field {forbidden}"
        );
    }
    assert_architecture_scope_summary_fields(&runtime_core_fields);

    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let coverage_core_fields = architecture_group_fields(
        &fixture,
        &architecture_request_ref,
        "architecture_core_context",
    );
    assert!(
        coverage_core_fields.contains(
            &"contextProjection.requirementDetailTransfer.requirementDetails".to_string()
        ),
        "coverage still needs the detail index for detailCoverage"
    );
    assert_architecture_scope_summary_fields(&coverage_core_fields);
}

#[test]
fn architecture_request_omits_previous_runtime_fields_without_previous_runtime() {
    let fixture = Fixture::new("architecture-runtime-no-previous");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let request_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);

    assert!(request_root
        .pointer("/sourceRefs/previousRuntimeDeliveryRef")
        .is_none());
    assert!(
        request_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    let runtime_contract =
        architecture_section_contract(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_eq!(
        runtime_contract
            .pointer("/enumRefs/runtimeDeliveryStatus")
            .cloned()
            .unwrap(),
        json!(["modified", "not_applicable"])
    );
    assert!(runtime_contract
        .pointer("/schemaShape/content/runtimeDelivery/basis/previousRuntimeDeliveryRef")
        .is_none());
    assert_eq!(
        runtime_contract
            .pointer("/schemaShape/content/runtimeDelivery/httpProbes/expectedStatus")
            .cloned()
            .unwrap(),
        json!("2xx_or_3xx")
    );
    assert_eq!(
        runtime_contract
            .pointer("/resultTemplate/content/runtimeDelivery/httpProbes/expectedStatus")
            .cloned()
            .unwrap(),
        json!("2xx_or_3xx")
    );
    assert!(runtime_contract
        .pointer("/resultTemplate/content/runtimeDelivery/start/port")
        .is_none());
    assert_eq!(
        runtime_contract
            .pointer(
                "/resultTemplate/content/runtimeDelivery/taskPlanningGuidance/verificationBoundary"
            )
            .cloned()
            .unwrap(),
        json!("code_level_only")
    );
    assert!(runtime_contract
        .pointer("/resultTemplate/content/runtimeDelivery/api")
        .is_none());
    assert!(runtime_contract
        .pointer("/generationRules")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("do not use unchanged")));
    assert!(runtime_contract
        .pointer("/generationRules")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("Omit unknown optional runtime fields")));
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
    );

    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &architecture_request_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let core_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert!(!core_group
        .fields
        .contains_key("sourceRefs.previousRuntimeDeliveryRef"));
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref,
    })
    .expect("inspect architecture request");
    let inspect_core_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "architecture_core_context")
        .expect("architecture core read group")
        .expanded_fields();
    assert!(!inspect_core_fields.contains(&"sourceRefs.previousRuntimeDeliveryRef".to_string()));
}

#[test]
fn architecture_request_exposes_previous_runtime_only_when_available() {
    let fixture = Fixture::new("architecture-runtime-with-previous");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let previous_runtime_ref = format!(
        ".loom/deliveries/{delivery_id}/contracts/architecture/phase-0/aac.json#/runtimeDelivery"
    );
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    let active_phase = index["phases"]
        .as_array_mut()
        .expect("phases")
        .iter_mut()
        .find(|phase| phase["phaseId"].as_str() == Some("phase-1"))
        .expect("phase-1");
    let latest_refs = active_phase["latestRefs"]
        .as_object_mut()
        .expect("latestRefs object");
    latest_refs.insert(
        "runtimeDelivery".to_string(),
        Value::String(previous_runtime_ref.clone()),
    );
    latest_refs.remove("architectureRequestRef");
    write_json_atomic(&index_path, &index).expect("write index with previous runtime");

    let result = architecture::ArchitectureDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &RouteAction {
            kind: RouteActionKind::ArchitectureArtifactContract,
            source: "test".to_string(),
            reason: "refresh_architecture_request".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let result = serde_json::to_value(result).expect("serialize architecture result");
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let refreshed_ref = result["next"]["requestRef"]
        .as_str()
        .expect("architecture requestRef")
        .to_string();
    assert_ne!(refreshed_ref, architecture_request_ref);
    let request_root = read_request_root_value(fixture.root_str(), &refreshed_ref);
    let source_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref.clone(),
        fields: vec!["sourceRefs.previousRuntimeDeliveryRef".to_string()],
    })
    .expect("read previous runtime ref")
    .fields;

    assert_eq!(
        source_fields["sourceRefs.previousRuntimeDeliveryRef"].value,
        json!(previous_runtime_ref)
    );
    assert!(
        request_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    let runtime_contract =
        architecture_section_contract(&fixture, &refreshed_ref, "runtime_delivery");
    assert_eq!(
        runtime_contract
            .pointer("/enumRefs/runtimeDeliveryStatus")
            .cloned()
            .unwrap(),
        json!(["modified", "unchanged", "not_applicable"])
    );
    assert_eq!(
        runtime_contract
            .pointer("/schemaShape/content/runtimeDelivery/basis/previousRuntimeDeliveryRef")
            .cloned()
            .unwrap(),
        json!("string")
    );

    advance_architecture_to_section(&fixture, &refreshed_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &refreshed_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let core_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture core group");
    assert_eq!(
        core_group.fields["sourceRefs.previousRuntimeDeliveryRef"].value,
        json!(previous_runtime_ref)
    );
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_ref,
    })
    .expect("inspect architecture request");
    let inspect_core_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "architecture_core_context")
        .expect("architecture core read group")
        .expanded_fields();
    assert!(inspect_core_fields.contains(&"sourceRefs.previousRuntimeDeliveryRef".to_string()));
}

#[test]
fn architecture_section_submit_advances_same_request_to_next_section() {
    let fixture = Fixture::new("architecture-next-section");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let architecture_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    assert!(
        architecture_root.get("sectionOutputs").is_none(),
        "architecture request root must not expose all section contracts"
    );
    assert!(architecture_root["currentSectionContract"]["resultTemplate"]["content"].is_object());
    let architecture_rules =
        architecture_root["currentSectionContract"]["generationRules"].to_string();
    assert!(architecture_rules.contains("existing project and technical baseline shape"));
    assert!(architecture_rules.contains("avoid pass-through wrappers"));
    let coverage_template =
        architecture_section_contract(&fixture, &architecture_request_ref, "coverage")
            ["resultTemplate"]["content"]
            .clone();
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["acceptanceId"],
        json!("acc_1")
    );
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["statement"],
        json!("工作人员可以完成证券账户开户并得到成功反馈。")
    );
    assert!(
        coverage_template["acceptanceMatrix"][0]
            .get("artifactRefs")
            .is_none(),
        "acceptanceMatrix must not template artifactRefs"
    );
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["coverage"][0]["type"],
        json!("module")
    );
    assert!(coverage_template["acceptanceMatrix"][0]
        .get("reason")
        .is_none());
    assert!(coverage_template["acceptanceMatrix"][0]["coverage"][0]["refs"].is_array());
    assert!(coverage_template["acceptanceMatrix"][0]["coverage"][0]["description"].is_string());
    assert_eq!(
        coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["kind"],
        json!("manual")
    );
    assert!(
        coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["description"].is_string()
    );
    assert!(
        coverage_template["detailCoverage"][0]["detailId"]
            .as_str()
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "detailCoverage rows must be pre-keyed by detailId"
    );
    assert!(coverage_template["detailCoverage"][0]
        .get("reason")
        .is_none());
    let artifact_refs = &coverage_template["detailCoverage"][0]["artifactRefs"];
    for key in [
        "modules",
        "entities",
        "fields",
        "constraints",
        "interfaces",
        "userFlows",
        "stateMachines",
        "frontendDataViews",
        "frontendActions",
        "frontendOperationPaths",
        "acceptanceMatrix",
    ] {
        assert!(artifact_refs[key].is_array(), "missing artifactRefs.{key}");
    }
    let frontend_authority_ref = architecture_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            architecture_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
                .and_then(Value::as_str)
        })
        .expect("frontend authority ref");
    let frontend_template =
        architecture_section_contract(&fixture, &architecture_request_ref, "frontend_experience");
    let frontend_template_refs = frontend_template["resultTemplate"]["content"]
        ["frontendExperience"]["sourceRefs"]
        .as_object()
        .expect("frontend sourceRefs");
    assert_eq!(frontend_template_refs.len(), 1);
    assert_eq!(
        frontend_template_refs["brainstormFrontendExperienceRef"],
        json!(frontend_authority_ref)
    );
    assert!(
        frontend_template["resultTemplate"]["content"]["frontendExperience"]
            .get("uiQualityContract")
            .is_some()
    );

    write_candidate_target(
        &fixture,
        &architecture_request_ref,
        &architecture_section_candidate_json(&fixture, &architecture_request_ref),
    );

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["requestRef"],
        json!(architecture_request_ref),
        "{result:#}"
    );
    assert_eq!(
        result["next"]["writeTargets"][0]["targetId"],
        "domain_contract"
    );

    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "auto_runnable", "{continued:#}");
    assert_eq!(
        continued["next"]["requestRef"], result["next"]["requestRef"],
        "{continued:#}"
    );
    assert_eq!(
        continued["next"]["writeTargets"][0]["targetId"],
        "domain_contract"
    );
}

#[test]
fn architecture_frontend_submit_repairs_missing_ui_quality_contract() {
    let fixture = Fixture::new("architecture-frontend-ui-quality-required");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "frontend_experience");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["frontendExperience"]
        .as_object_mut()
        .expect("frontend object")
        .remove("uiQualityContract");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == json!("UI_QUALITY_CONTRACT_REQUIRED")));
}

#[test]
fn architecture_frontend_submit_derives_machine_owned_ui_quality_fields() {
    let fixture = Fixture::new("architecture-frontend-ui-quality-machine-owned");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "frontend_experience");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
    })
    .expect("inspect frontend architecture request");
    let target_path = inspected.write_targets[0]["path"]
        .as_str()
        .expect("candidate target path")
        .to_string();
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    let ui_quality_contract = candidate["content"]["frontendExperience"]["uiQualityContract"]
        .as_object_mut()
        .expect("uiQualityContract");
    ui_quality_contract.insert("semanticTokenPolicy".to_string(), json!("wrong_policy"));
    ui_quality_contract.insert(
        "forbiddenUserVisibleContent".to_string(),
        json!(["agent_invented_forbidden_item"]),
    );
    ui_quality_contract.insert(
        "referenceProfile".to_string(),
        json!({
            "loadMode": "manual",
            "groups": {
                "focus": ["unknown-reference"]
            },
            "referenceLoadPlan": []
        }),
    );
    ui_quality_contract.insert(
        "qualityGates".to_string(),
        json!([{
            "gateId": "invented.agent.gate",
            "sourceRefId": "uix.fake",
            "severity": "must",
            "appliesToSurfaceRoles": ["page"],
            "evidenceRequired": ["source_check"],
            "expectation": "This agent-authored gate must be replaced."
        }]),
    );
    ui_quality_contract
        .get_mut("scenario")
        .and_then(Value::as_object_mut)
        .expect("scenario")
        .insert("kind".to_string(), json!("admin_dashboard"));
    ui_quality_contract
        .get_mut("scenario")
        .and_then(Value::as_object_mut)
        .expect("scenario")
        .insert(
            "reference".to_string(),
            json!({"group": "wrong", "item": "wrong"}),
        );
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let persisted = state::store::read_json_value(&fixture.root.join(target_path))
        .expect("read normalized frontend candidate");
    let persisted_contract = &persisted["content"]["frontendExperience"]["uiQualityContract"];
    assert_eq!(
        persisted_contract["semanticTokenPolicy"],
        json!("semantic_tokens_required")
    );
    assert!(persisted_contract["forbiddenUserVisibleContent"]
        .as_array()
        .expect("forbidden user-visible content")
        .contains(&json!("runtime_commands")));
    assert!(!persisted_contract["forbiddenUserVisibleContent"]
        .as_array()
        .expect("forbidden user-visible content")
        .contains(&json!("agent_invented_forbidden_item")));
    assert_eq!(
        persisted_contract["scenario"]["reference"],
        json!({"group": "scenarios", "item": "admin-dashboard"})
    );
    assert_eq!(
        persisted_contract["referenceProfile"]["loadMode"],
        json!("mcp_reference_load_plan")
    );
    assert!(persisted_contract["referenceProfile"]["groups"]["focus"]
        .as_array()
        .expect("focus references")
        .contains(&json!("web-implementation")));
    assert!(persisted_contract["referenceProfile"]["referenceLoadPlan"]
        .as_array()
        .expect("referenceLoadPlan")
        .iter()
        .any(|item| item["path"] == json!("uix/web-implementation.md")));
    let gate_ids = persisted_contract["qualityGates"]
        .as_array()
        .expect("qualityGates")
        .iter()
        .filter_map(|gate| gate["gateId"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(gate_ids.contains("web.semantic_accessibility"));
    assert!(gate_ids.contains("web.runtime_layout_safety"));
    assert!(!gate_ids.contains("invented.agent.gate"));
}

#[test]
fn architecture_runtime_delivery_submit_repairs_missing_contract_fields() {
    let fixture = Fixture::new("architecture-runtime-contract-repair");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "runtime_delivery");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["runtimeDelivery"]["build"]
        .as_object_mut()
        .expect("build object")
        .remove("codeLevelExpectations");
    candidate["content"]["runtimeDelivery"]
        .as_object_mut()
        .expect("runtime object")
        .remove("httpProbes");
    candidate["content"]["runtimeDelivery"]["taskPlanningGuidance"]
        .as_object_mut()
        .expect("guidance object")
        .remove("requireRuntimeDeliveryRequirementWhenTaskTouches");
    candidate["content"]["runtimeDelivery"]["taskPlanningGuidance"]["verificationBoundary"] =
        json!("deploy_success");
    candidate["content"]["runtimeDelivery"]["frontend"]
        .as_object_mut()
        .expect("frontend object")
        .remove("outputDir");
    candidate["content"]["runtimeDelivery"]["start"]["port"] = Value::Null;
    candidate["content"]["runtimeDelivery"]["api"]["entry"] = json!("");
    candidate["content"]["runtimeDelivery"]["api"]["probePaths"] = json!([]);
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    let issues = result["issues"].as_array().expect("issues");
    for field_path in [
        "content.runtimeDelivery.build.codeLevelExpectations",
        "content.runtimeDelivery.httpProbes.previewPath",
        "content.runtimeDelivery.taskPlanningGuidance.requireRuntimeDeliveryRequirementWhenTaskTouches",
        "content.runtimeDelivery.taskPlanningGuidance.verificationBoundary",
        "content.runtimeDelivery.frontend.outputDir",
        "content.runtimeDelivery.start.port",
        "content.runtimeDelivery.api",
    ] {
        assert!(
            issues
                .iter()
                .any(|issue| issue["fieldPath"] == json!(field_path)),
            "missing issue for {field_path}: {issues:#?}"
        );
    }
}

#[test]
fn planning_contract_preserves_brainstorm_requirement_detail_index() {
    let fixture = Fixture::new("pgc-detail-index");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        candidate_with_planning_details_json(),
    );
    let architecture_root = read_request_root_value(fixture.root_str(), &architecture_request_ref);
    let source_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        fields: vec!["sourceRefs.planningContractRef".to_string()],
    })
    .expect("read planning contract ref")
    .fields;
    let planning_contract_ref = source_fields["sourceRefs.planningContractRef"]
        .value
        .as_str()
        .expect("planning contract ref");
    let planning_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(planning_contract_ref))
            .expect("read planning contract"),
    )
    .expect("parse planning contract");

    assert_eq!(
        planning_contract["planningInputs"]["actors"][0]["id"],
        json!("actor_staff")
    );
    assert_eq!(
        planning_contract["planningInputs"]["capabilityGroups"][0]["id"],
        json!("cap_account_opening")
    );

    let details = planning_contract["requirementDetails"]["items"]
        .as_array()
        .expect("requirement details");
    for expected_detail_id in [
        "detail.scope.scope_1.1",
        "detail.scope.scope_1.2",
        "detail.deferred.deferred_1.1",
        "detail.excluded.excluded_1.1",
        "detail.acceptance.acc_1",
        "detail.businessFlow.flow_account_opening",
        "detail.concept.concept_security_account",
        "detail.frontend.view.view_account_list",
        "detail.frontend.action.action_open_account",
        "detail.frontend.path_account_lifecycle",
        "detail.assumption.assumption_1",
    ] {
        assert!(
            details
                .iter()
                .any(|detail| detail["detailId"] == json!(expected_detail_id)),
            "missing PGC requirement detail {expected_detail_id}"
        );
    }
    let allowed_impact_tags = [
        "scope",
        "data_model",
        "business_flow",
        "frontend",
        "interface",
        "acceptance",
        "runtime",
    ];
    let allowed_lifecycle_stages = [
        "create",
        "query_select",
        "view",
        "update",
        "approve_or_process",
        "state_change",
        "terminate_or_cancel",
        "blocking_or_exception",
        "not_applicable",
    ];
    let allowed_qualities = ["thin", "usable", "rich"];
    for detail in details {
        assert!(
            allowed_qualities.contains(&detail["quality"].as_str().expect("quality")),
            "invalid PGC detail quality: {detail:#}"
        );
        assert!(
            allowed_lifecycle_stages
                .contains(&detail["lifecycleStage"].as_str().expect("lifecycleStage")),
            "invalid PGC detail lifecycleStage: {detail:#}"
        );
        for tag in detail["impactTags"].as_array().expect("impactTags") {
            assert!(
                allowed_impact_tags.contains(&tag.as_str().expect("impact tag")),
                "invalid PGC detail impactTag: {detail:#}"
            );
        }
    }
    assert!(details.iter().any(|detail| {
        detail["detailId"] == json!("detail.concept.concept_security_account")
            && detail["conceptRefs"] == json!(["concept_security_account"])
    }));
    let first_scope_detail = details
        .iter()
        .find(|detail| detail["detailId"] == json!("detail.scope.scope_1.1"))
        .expect("first scope detail");
    assert_eq!(
        first_scope_detail["sourceFieldRefs"],
        json!(["brainstorm.scope.included[0].items[0]"])
    );

    let core_group = architecture_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups")
        .iter()
        .find(|group| group["groupId"] == json!("architecture_core_context"))
        .expect("architecture core group");
    let core_fields = read_group_fields_from_json(core_group);
    assert!(core_fields
        .contains(&"contextProjection.requirementDetailTransfer.requirementDetails".to_string()));
    assert!(core_fields
        .contains(&"contextProjection.requirementDetailTransfer.businessFlows".to_string()));
    assert!(!core_fields.contains(&"contextProjection".to_string()));
    assert!(!core_fields.contains(&"contextProjection.requirementDetailTransfer".to_string()));
    assert!(!core_fields.contains(
        &"contextProjection.requirementDetailTransfer.frontendExperienceDetails".to_string()
    ));
    let domain_group = architecture_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups")
        .iter()
        .find(|group| group["groupId"] == json!("architecture_domain_model_context"))
        .expect("domain model group");
    assert_eq!(
        read_group_fields_from_json(domain_group),
        vec![
            "contextProjection.requirementDetailTransfer.actors",
            "contextProjection.requirementDetailTransfer.capabilityGroups"
        ]
    );
    let projected_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: architecture_request_ref.clone(),
        fields: vec!["contextProjection.requirementDetailTransfer.requirementDetails".to_string()],
    })
    .expect("read projected requirement details")
    .fields;
    let projected_details =
        &projected_fields["contextProjection.requirementDetailTransfer.requirementDetails"].value;
    assert!(projected_details["items"][0]
        .get("sourceFieldRefs")
        .is_none());
    assert!(projected_details.get("extractionWarnings").is_none());
    assert!(
        projected_details["extractionWarningCount"].is_number(),
        "projected details did not expose extractionWarningCount: {projected_details:#}"
    );
    assert_eq!(
        projected_details["fullDetailSource"],
        "sourceRefs.planningContractRef#/requirementDetails"
    );
}

#[test]
fn architecture_coverage_submit_repairs_missing_acceptance_statement() {
    let fixture = Fixture::new("architecture-coverage-missing-statement");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["acceptanceMatrix"][0]
        .as_object_mut()
        .expect("acceptance row")
        .remove("statement");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["issues"][0]["code"], "ACCEPTANCE_MATRIX_INVALID");
    assert_eq!(
        result["issues"][0]["fieldPath"],
        "content.acceptanceMatrix[0].statement"
    );
}

#[test]
fn architecture_coverage_submit_repairs_string_verification_hints() {
    let fixture = Fixture::new("architecture-coverage-string-hints");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["acceptanceMatrix"][0]["verificationHints"] =
        json!(["Verify the flow manually."]);
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "ACCEPTANCE_MATRIX_INVALID"
            && issue["fieldPath"] == "content.acceptanceMatrix[0].verificationHints[0]"
    }));
}

#[test]
fn architecture_coverage_submit_repairs_schema_shape_before_assembly() {
    let fixture = Fixture::new("architecture-coverage-contract-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["detailCoverage"][0]["artifactRefs"]["modules"] =
        json!(["module.account-service", 42]);
    candidate["content"]["acceptanceMatrix"][0]["coverage"] = json!(["module.account-service"]);
    candidate["content"]["handoff"]["readyForTaskPlan"] = json!("yes");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    let issues = result["issues"].as_array().unwrap();
    assert!(issues.iter().any(|issue| {
        issue["code"] == "DETAIL_COVERAGE_INVALID"
            && issue["fieldPath"] == "content.detailCoverage[0].artifactRefs.modules[1]"
    }));
    assert!(issues.iter().any(|issue| {
        issue["code"] == "ACCEPTANCE_MATRIX_INVALID"
            && issue["fieldPath"] == "content.acceptanceMatrix[0].coverage[0].type"
    }));
    assert!(issues.iter().any(|issue| {
        issue["code"] == "COVERAGE_HANDOFF_INVALID"
            && issue["fieldPath"] == "content.handoff.readyForTaskPlan"
    }));
}

#[test]
fn architecture_coverage_submit_repairs_invalid_type_and_missing_reason() {
    let fixture = Fixture::new("architecture-coverage-type-reason");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["acceptanceMatrix"][0]["coverage"][0]["type"] = json!("modules");
    candidate["content"]["acceptanceMatrix"][0]["coverageStatus"] = json!("partial");
    candidate["content"]["acceptanceMatrix"][0]
        .as_object_mut()
        .expect("acceptance row")
        .remove("reason");
    candidate["content"]["detailCoverage"][0]["coverageStatus"] = json!("uncovered");
    candidate["content"]["detailCoverage"][0]
        .as_object_mut()
        .expect("detail row")
        .remove("reason");
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    let issues = result["issues"].as_array().expect("issues");
    for field_path in [
        "content.acceptanceMatrix[0].coverage[0].type",
        "content.acceptanceMatrix[0].reason",
        "content.detailCoverage[0].reason",
    ] {
        assert!(
            issues
                .iter()
                .any(|issue| issue["fieldPath"] == json!(field_path)),
            "missing issue for {field_path}: {issues:#?}"
        );
    }
}

#[test]
fn architecture_coverage_submit_normalizes_deferred_detail_reason() {
    let fixture = Fixture::new("architecture-coverage-deferred-reason-normalization");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        candidate_with_planning_details_json(),
    );
    advance_architecture_to_section(&fixture, &architecture_request_ref, "coverage");
    let mut candidate = architecture_section_candidate_json(&fixture, &architecture_request_ref);
    candidate["content"]["detailCoverage"]
        .as_array_mut()
        .expect("detail coverage")
        .push(json!({
            "detailId": "detail.deferred.deferred_1.1",
            "coverageStatus": "deferred",
            "artifactRefs": {
                "modules": [],
                "entities": [],
                "fields": [],
                "constraints": [],
                "interfaces": [],
                "userFlows": [],
                "stateMachines": [],
                "frontendDataViews": [],
                "frontendActions": [],
                "frontendOperationPaths": [],
                "acceptanceMatrix": []
            }
        }));
    write_candidate_target(&fixture, &architecture_request_ref, &candidate);

    let result = call_submit(
        "loom.architectureSectionSubmitFile",
        &architecture_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let aac_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(aac_ref)).unwrap())
            .expect("parse persisted AAC");
    let deferred_row = persisted["detailCoverage"]
        .as_array()
        .expect("detail coverage")
        .iter()
        .find(|row| {
            row["detailId"] == json!("detail.deferred.deferred_1.1")
                && row["reason"].as_str().is_some_and(|reason| {
                    reason.contains("Deferred by the confirmed phase boundary")
                })
        })
        .expect("deferred detail row");
    assert_eq!(deferred_row["coverageStatus"], json!("deferred"));
    assert!(deferred_row["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("Deferred by the confirmed phase boundary")));
}

#[test]
fn architecture_coverage_submit_persists_aac_and_routes_to_taskplan_generation() {
    let fixture = Fixture::new("architecture-full-chain");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let result = complete_architecture_sections(&fixture, &architecture_request_ref);

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "task_plan_candidate");
    assert_eq!(result["next"]["submitTool"], "loom.taskPlanAcceptFile");

    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    assert_eq!(
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact"),
        format!(".loom/deliveries/{delivery_id}/contracts/architecture/phase-1/aac.json")
    );
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/architecture/phase-1/aac.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("contracts/architecture/phase-1/latest.json")
        .exists());
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    assert_eq!(inspected.request_kind, "taskplan_generation_request");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.taskPlanAcceptFile")
    );
    assert_eq!(inspected.write_targets.len(), 2);
    let taskplan_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        group_id: "taskplan_generation_rules".to_string(),
    })
    .expect("read taskplan rules");
    let taskplan_rules_text =
        serde_json::to_string(&taskplan_rules.fields).expect("serialize taskplan generation rules");
    assert!(taskplan_rules_text.contains("Keep next-phase seeds"));
    assert!(taskplan_rules_text.contains("smallest stable verification signal"));
    let compact_taskplan_root = read_request_root_value(fixture.root_str(), taskplan_request_ref);
    assert_no_root_submit_metadata(&compact_taskplan_root);
    let taskplan_core_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "taskplan_core_context")
        .expect("taskplan core group");
    let taskplan_core_fields = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        group_id: "taskplan_core_context".to_string(),
    })
    .expect("read taskplan core group")
    .fields;
    assert!(taskplan_core_fields
        .iter()
        .filter(|(field, _)| field.starts_with("sourceRefs."))
        .all(|(_, field)| !field.value.is_null()));
    for field in [
        "sourceRefs.repositoryContextRef",
        "sourceRefs.phaseConceptGroundingRef",
        "sourceRefs.deliveryConceptGlossaryRef",
    ] {
        assert_eq!(
            taskplan_core_group
                .expanded_fields()
                .contains(&field.to_string()),
            taskplan_core_fields.contains_key(field)
        );
    }
    let planning_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "planningContract");
    let planning_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(&planning_contract_ref))
            .expect("read planning contract"),
    )
    .expect("parse planning contract");
    assert_eq!(
        taskplan_core_fields["sourceRefs.repositoryContextRef"].value,
        planning_contract["contextRefs"]["repositoryContextRef"],
        "TaskPlan must carry the current phase RepositoryContext ref from PGC"
    );
    assert!(compact_taskplan_root.get("repositoryContext").is_none());
    assert!(compact_taskplan_root
        .get("latestRepositoryContext")
        .is_none());
    assert!(compact_taskplan_root
        .pointer("/contextProjection/frontendExperienceProjection")
        .is_none());
    assert!(compact_taskplan_root
        .pointer("/contextProjection/runtimeDeliveryProjection")
        .is_none());
    assert!(!inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_optional_projection"));
    assert!(inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(
            |field| field != "contextProjection.frontendExperienceProjection"
                && field != "contextProjection.runtimeDeliveryProjection"
        ));
    let taskplan_contract_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "outputContract.outlineResultTemplate".to_string(),
            "outputContract.groupResultTemplate".to_string(),
            "outputContract.frontendExperienceRequirementTemplate".to_string(),
            "outputContract.runtimeDeliveryRequirementTemplate".to_string(),
            "outputContract.runtimeDeliveryClosureTaskTemplate".to_string(),
            "outputContract.engineeringQualityRequirementTemplate".to_string(),
            "generationRules.runtimeDeliveryRules".to_string(),
            "generationRules.verificationEvidenceRules".to_string(),
            "generationRules.engineeringQualityRules".to_string(),
        ],
    })
    .expect("read taskplan contract fields")
    .fields;
    assert!(
        taskplan_contract_fields["outputContract.outlineResultTemplate"].value["groups"][0]
            .is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["group"].is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            .is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptResponsibilities"][0]
            .is_object()
    );
    assert!(
        taskplan_contract_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptVerificationIntents"][0]
            .is_object()
    );
    let frontend_requirement_template =
        &taskplan_contract_fields["outputContract.frontendExperienceRequirementTemplate"].value;
    assert_eq!(
        frontend_requirement_template["uiQualityContract"]["semanticTokenPolicy"],
        json!("semantic_tokens_required")
    );
    assert_eq!(
        frontend_requirement_template["uiQualityContract"]["designTokenAssetPlan"]["templateId"],
        json!("tokens-css")
    );
    assert!(
        frontend_requirement_template["uiQualityContract"]["referenceProfile"]["groups"]["tokens"]
            .as_array()
            .expect("ui quality token refs")
            .contains(&json!("spacing"))
    );
    let runtime_requirement_template =
        &taskplan_contract_fields["outputContract.runtimeDeliveryRequirementTemplate"].value;
    assert!(runtime_requirement_template.is_object());
    assert!(
        runtime_requirement_template["requiredCodeLevelChecks"][0]["checkId"].is_string(),
        "{runtime_requirement_template:#}"
    );
    assert!(
        runtime_requirement_template["requiredCodeLevelChecks"][0]["acceptableEvidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "{runtime_requirement_template:#}"
    );
    let runtime_closure_template =
        &taskplan_contract_fields["outputContract.runtimeDeliveryClosureTaskTemplate"].value;
    assert_eq!(
        runtime_closure_template["groupPlacement"]["position"],
        json!("final_group")
    );
    assert!(runtime_closure_template["groupPlacement"]["taskIdsRule"]
        .as_str()
        .is_some_and(|rule| rule.contains("exactly this one runtime_delivery_closure task")));
    let closure_requirement = &runtime_closure_template["runtimeDeliveryRequirement"];
    assert!(closure_requirement["affectedContractFields"]
        .as_array()
        .expect("affectedContractFields")
        .contains(&json!("httpProbes")));
    assert!(closure_requirement["affectedContractFields"]
        .as_array()
        .expect("affectedContractFields")
        .contains(&json!("frontend")));
    let closure_checks = closure_requirement["requiredCodeLevelChecks"]
        .as_array()
        .expect("requiredCodeLevelChecks");
    assert!(closure_checks
        .iter()
        .any(|check| check["checkId"] == json!("rd-closure-httpprobes")
            && check["contractField"] == json!("httpProbes")));
    assert_eq!(
        closure_checks.len(),
        closure_requirement["affectedContractFields"]
            .as_array()
            .expect("affectedContractFields")
            .len(),
        "{closure_requirement:#}"
    );
    let runtime_rules = &taskplan_contract_fields["generationRules.runtimeDeliveryRules"].value;
    assert!(runtime_rules["closureGroupRule"]
        .as_str()
        .is_some_and(|rule| rule.contains("only task in its group")
            && rule.contains("final outline.groups entry")));
    let verification_rules =
        &taskplan_contract_fields["generationRules.verificationEvidenceRules"].value;
    let verification_rules_text =
        serde_json::to_string(verification_rules).expect("serialize verification rules");
    assert!(verification_rules_text.contains("Every covered current-phase detailId"));
    assert!(verification_rules_text.contains("same parent task.requirementDetailRefs"));
    let engineering_template =
        &taskplan_contract_fields["outputContract.engineeringQualityRequirementTemplate"].value;
    assert_eq!(
        engineering_template["kind"],
        json!("persistence_mapping"),
        "{engineering_template:#}"
    );
    assert!(engineering_template["stackSignals"]
        .as_object()
        .expect("stackSignals")
        .contains_key("persistence"));
    assert_eq!(
        engineering_template["taskRefRule"],
        json!("Tasks reference this by engineeringQualityRequirementRefs; do not inline or duplicate the full object in each task.")
    );
    let engineering_rules =
        &taskplan_contract_fields["generationRules.engineeringQualityRules"].value;
    assert!(engineering_rules["acceptNormalization"]
        .as_str()
        .is_some_and(|rule| rule.contains("do not duplicate full quality requirements")));
    assert!(inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_core_context"));
    assert!(inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(|field| field != "outputContract.outlineSchemaShape"
            && field != "outputContract.groupSchemaShape"));
}

#[test]
fn taskplan_request_keeps_deferred_scope_out_of_current_scope_refs() {
    let fixture = Fixture::new("taskplan-scope-boundary");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        candidate_with_planning_details_json(),
    );
    let result = complete_architecture_sections(&fixture, &architecture_request_ref);
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "allowedRefs.scopeRefs".to_string(),
            "allowedRefs.deferredScopeRefs".to_string(),
            "allowedRefs.excludedScopeRefs".to_string(),
            "contextProjection.requirementDetailTransfer.requirementDetailAssignment".to_string(),
        ],
    })
    .expect("read taskplan scope fields")
    .fields;

    assert_eq!(fields["allowedRefs.scopeRefs"].value, json!(["scope_1"]));
    assert_eq!(
        fields["allowedRefs.deferredScopeRefs"].value,
        json!(["deferred_1"])
    );
    assert_eq!(
        fields["allowedRefs.excludedScopeRefs"].value,
        json!(["excluded_1"])
    );
    let assignment =
        &fields["contextProjection.requirementDetailTransfer.requirementDetailAssignment"].value;
    let item = &assignment["items"][0];
    assert!(item.get("coverage").is_none());
    assert!(item["quality"].is_string());
    assert!(item["coverageStatus"].is_string());
    assert!(item["artifactRefs"].is_object() || item["artifactRefs"].is_null());
    assert!(item["coverageReason"].is_null() || item["coverageReason"].is_string());
    assert!(assignment["verificationRule"]
        .as_str()
        .is_some_and(|rule| rule.contains("must be referenced")));
    assert!(assignment["verificationSubsetRule"]
        .as_str()
        .is_some_and(|rule| rule.contains("same parent task.requirementDetailRefs")));
}

#[test]
fn taskplan_request_derives_frontend_workflow_closure_requirements() {
    let fixture = Fixture::new("taskplan-workflow-closure");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_json,
    );
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let taskplan_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    let closure_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "contextProjection.requirementDetailTransfer.workflowClosureRequirements".to_string(),
            "generationRules.workflowClosureRules".to_string(),
        ],
    })
    .expect("read workflow closure fields")
    .fields;
    let requirements = closure_fields
        ["contextProjection.requirementDetailTransfer.workflowClosureRequirements"]
        .value
        .as_array()
        .expect("workflow closure requirements");
    assert_eq!(requirements.len(), 1, "{requirements:#?}");
    assert_eq!(
        requirements[0]["closureId"],
        json!("closure:flow.account-lifecycle:step.submit-open-account")
    );
    assert_eq!(
        requirements[0]["interfaceRefs"],
        json!(["api.account.open"])
    );
    assert_eq!(
        requirements[0]["operationPathRefs"],
        json!(["path_account_lifecycle"])
    );
    assert_eq!(
        requirements[0]["requiredEvidence"],
        json!([
            "user_action",
            "declared_interface_invocation",
            "state_or_persistence_change",
            "success_or_blocking_feedback"
        ])
    );
    assert_eq!(
        closure_fields["generationRules.workflowClosureRules"].value["requirementSource"],
        json!("contextProjection.requirementDetailTransfer.workflowClosureRequirements")
    );
}

#[test]
fn task_execution_request_carries_task_scoped_frontend_closure_guidance() {
    let fixture = Fixture::new("task-exec-frontend-guidance");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, taskplan_request_ref);
    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_eq!(accepted["next"]["kind"], "execute_task");
    assert_eq!(accepted["next"]["submitTool"], "loom.recordTaskResultFile");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");

    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
    })
    .expect("inspect execution request");
    let core_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_core")
        .expect("core group");
    let core_fields = core_group.expanded_fields();
    assert!(core_fields.contains(&"task.writeBoundary.artifactRefs".to_string()));
    assert!(!core_fields
        .contains(&"sourceContext.architectureArtifactProjection.interfaces".to_string()));
    let frontend_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_frontend_context")
        .expect("frontend context group");
    let frontend_fields = frontend_group.expanded_fields();
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs".to_string()
    ));
    assert!(frontend_fields
        .contains(&"task.frontendExperienceRequirement.executionGuidance.uiQuality".to_string()));
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.executionGuidance.uiProductionBrief".to_string()
    ));
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.executionGuidance.styleAssetPlan".to_string()
    ));
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups".to_string()
    ));
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"
            .to_string()
    ));
    assert!(frontend_fields.contains(
        &"task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"
            .to_string()
    ));
    assert!(!frontend_fields.contains(
        &"task.frontendExperienceRequirement.uiQualityContract.referenceProfile".to_string()
    ));
    assert!(!frontend_fields.contains(
        &"task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan".to_string()
    ));
    assert!(frontend_fields
        .contains(&"executionRules.frontendImplementationOrganizationRules".to_string()));
    assert!(
        frontend_fields.contains(&"executionRules.interactiveVerificationProbePolicy".to_string())
    );
    let runtime_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_runtime_context")
        .expect("runtime context group");
    assert!(runtime_group
        .expanded_fields()
        .contains(&"executionRules.controlledRuntimeProbeRules".to_string()));
    let architecture_group = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_architecture_context")
        .expect("architecture context group");
    assert!(architecture_group
        .expanded_fields()
        .contains(&"sourceContext.architectureArtifactProjection.interfaces".to_string()));
    assert!(!frontend_fields
        .contains(&"sourceContext.architectureArtifactProjection.frontendExperience".to_string()));

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        fields: vec![
            "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"
                .to_string(),
            "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"
                .to_string(),
            "task.frontendExperienceRequirement.executionGuidance.surfacesInScope".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.actionsInScope".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.styleAssetPlan".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.uiQuality".to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.scenario".to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups"
                .to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"
                .to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"
                .to_string(),
            "sourceContext.architectureArtifactProjection.interfaces".to_string(),
            "executionRules.frontendImplementationOrganizationRules".to_string(),
            "executionRules.interactiveVerificationProbePolicy".to_string(),
            "executionRules.controlledRuntimeProbeRules".to_string(),
            "outputContract.schemaShape.properties.frontendQualitySelfCheck".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "outputContract.resultTemplate".to_string(),
            "outputContract.resultRules".to_string(),
        ],
    })
    .expect("read execution fields")
    .fields;
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"].value
            [0]["closureId"],
        json!("closure:flow.account-lifecycle:step.submit-open-account")
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"]
            .value[0]["interfaces"][0]["interfaceId"],
        json!("api.account.open")
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.surfacesInScope"].value[0]
            ["surfaceId"],
        json!("surface_account_admin")
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.actionsInScope"].value[0]
            ["actionId"],
        json!("action_open_account")
    );
    assert!(
        fields["task.frontendExperienceRequirement.executionGuidance.uiProductionBrief"].value
            ["forbiddenUserVisibleContent"]
            .as_array()
            .expect("forbidden user-visible content")
            .contains(&json!("runtime_commands"))
    );
    assert!(
        fields["task.frontendExperienceRequirement.executionGuidance.styleAssetPlan"].value
            ["referenceGroups"]["tokens"]
            .as_array()
            .expect("style token reference items")
            .contains(&json!("spacing"))
    );
    let interfaces = &fields["sourceContext.architectureArtifactProjection.interfaces"].value;
    assert_eq!(interfaces.as_array().expect("interfaces").len(), 1);
    assert_eq!(interfaces[0]["interfaceId"], json!("api.account.open"));
    assert!(serde_json::to_string(
        &fields["executionRules.frontendImplementationOrganizationRules"].value
    )
    .unwrap()
    .contains("reachable entry"));
    assert!(serde_json::to_string(
        &fields["executionRules.interactiveVerificationProbePolicy"].value
    )
    .unwrap()
    .contains("smallest applicable probe plan"));
    assert!(
        serde_json::to_string(&fields["executionRules.controlledRuntimeProbeRules"].value)
            .unwrap()
            .contains("Never run long-lived runtime")
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendExperienceSelfCheck"]
            ["closureRequirementIds"],
        json!(["closure:flow.account-lifecycle:step.submit-open-account"])
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.executionGuidance.uiQuality"].value
            ["selfCheckField"],
        json!("frontendQualitySelfCheck")
    );
    assert!(
        fields["task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups"]
            .value["tokens"]
            .as_array()
            .expect("ui quality token reference items")
            .contains(&json!("spacing"))
    );
    assert!(
        fields["task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"]
            .value
            .as_array()
            .expect("ui reference load plan")
            .iter()
            .any(|item| item["path"] == json!("uix/tokens/spacing.md"))
    );
    assert_eq!(
        fields["task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"]
            .value,
        json!("tokens-css")
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]["scenarioKind"],
        fields["task.frontendExperienceRequirement.uiQualityContract.scenario"].value["kind"]
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]["qualityLevel"],
        json!("production_internal_product")
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["referenceGroupsChecked"]["tokens"]
            .as_array()
            .expect("reference group items checked")
            .contains(&json!("spacing"))
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["referenceFilesChecked"]
            .as_array()
            .expect("reference files checked")
            .contains(&json!("uix/tokens/spacing.md"))
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]["statesCovered"]
            [0]
        .is_object()
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["businessUiRulesChecked"][0]
            .is_object()
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["surfacesCovered"][0]["surfaceId"],
        json!("surface_account_admin")
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["surfacesCovered"][0]["files"]
            .as_array()
            .expect("surface files")
            .contains(&json!("replace_with_ui_file_path_for_this_surface"))
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["surfacesCovered"][0]["states"]
            .as_array()
            .expect("surface states")
            .contains(&json!("business_blocking"))
    );
    assert!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["surfacesCovered"][0]["businessActions"]
            .as_array()
            .expect("surface actions")
            .contains(&json!("action_open_account"))
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["frontendQualitySelfCheck"]
            ["designTokenEvidence"]["templateIdUsed"],
        json!("tokens-css")
    );
    assert!(fields["outputContract.resultRules"]
        .value
        .as_array()
        .expect("result rules")
        .iter()
        .any(|rule| rule
            .as_str()
            .is_some_and(|text| text.contains("do not leave replace_with_* values"))));
    assert!(
        fields["outputContract.schemaShape.properties.frontendQualitySelfCheck"]
            .value
            .is_object()
    );
    assert!(fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendQualitySelfCheck")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut result);
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write task result");
    let record_result = call_submit(
        "loom.recordTaskResultFile",
        execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(record_result["state"], "auto_runnable", "{record_result:#}");
}

#[test]
fn task_result_repair_carries_frontend_quality_contract_fields() {
    let fixture = Fixture::new("task-result-frontend-quality-repair");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, taskplan_request_ref);
    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution contract")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/App.tsx"]);
    result
        .as_object_mut()
        .expect("result object")
        .remove("frontendQualitySelfCheck");
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write task result");

    let record_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(record_result["state"], "auto_runnable", "{record_result:#}");
    assert_eq!(
        record_result["next"]["artifactKind"], "task_result_repair",
        "{record_result:#}"
    );
    let repair_request_ref = record_result["next"]["requestRef"]
        .as_str()
        .expect("repair requestRef")
        .to_string();
    let repair_root = read_request_root_value(fixture.root_str(), &repair_request_ref);
    let repair_read_fields = repair_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("repair read groups")
        .iter()
        .flat_map(read_group_fields_from_json)
        .collect::<BTreeSet<_>>();
    assert!(repair_read_fields
        .contains("task.frontendExperienceRequirement.executionGuidance.uiQuality"));
    assert!(repair_read_fields
        .contains("task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups"));
    assert!(repair_read_fields.contains(
        "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"
    ));
    assert!(repair_read_fields.contains(
        "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"
    ));
    assert!(!repair_read_fields
        .contains("task.frontendExperienceRequirement.uiQualityContract.referenceProfile"));
    assert!(!repair_read_fields
        .contains("task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan"));
    assert!(repair_read_fields
        .contains("outputContract.schemaShape.properties.frontendQualitySelfCheck"));
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref,
        fields: vec![
            "repairContract.issueConflicts".to_string(),
            "repairContract.minimalRepairRules".to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups"
                .to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"
                .to_string(),
            "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"
                .to_string(),
            "outputContract.resultTemplate".to_string(),
            "outputContract.schemaShape.properties.frontendQualitySelfCheck".to_string(),
        ],
    })
    .expect("read repair contract fields")
    .fields;
    assert!(repair_fields["repairContract.issueConflicts"]
        .value
        .as_array()
        .expect("issue conflicts")
        .iter()
        .any(|issue| issue["code"] == "TASK_RESULT_FRONTEND_QUALITY_INVALID"));
    assert!(
        serde_json::to_string(&repair_fields["repairContract.minimalRepairRules"].value)
            .expect("serialize rules")
            .contains("frontendQualitySelfCheck")
    );
    assert!(repair_fields
        ["task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups"]
        .value["tokens"]
        .as_array()
        .expect("reference group items")
        .contains(&json!("spacing")));
    assert!(repair_fields
        ["task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan"]
        .value
        .as_array()
        .expect("reference load plan")
        .iter()
        .any(|item| item["path"] == json!("uix/tokens/spacing.md")));
    assert_eq!(
        repair_fields
            ["task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId"]
            .value,
        json!("tokens-css")
    );
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("frontendQualitySelfCheck")
        .is_some());
    assert!(
        repair_fields["outputContract.schemaShape.properties.frontendQualitySelfCheck"]
            .value
            .is_object()
    );
}

#[test]
fn task_result_submit_normalizes_machine_owned_refs_before_validation() {
    let fixture = Fixture::new("task-result-machine-ref-normalization");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates(&fixture, taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["taskKind"] = json!("ui_flow_increment");
    group_value["tasks"][0]["implementationActions"] =
        json!(["wire_reference_in_api_or_ui", "add_or_update_tests"]);
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["interfaces"] =
        json!(["api.account.open"]);
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["userFlows"] =
        json!(["flow.account-lifecycle"]);
    group_value["tasks"][0]["verificationIntents"][0]["preferredEvidence"] =
        json!(["runtime_api_check"]);
    group_value["tasks"][0]["verificationIntents"][0]["acceptableEvidence"] = json!([
        "automated_test",
        "runtime_api_check",
        "manual_command_output"
    ]);
    group_value["tasks"][0]["conceptRefs"] = json!(["concept-account-ui"]);
    group_value["tasks"][0]["frontendExperienceRequirement"] =
        frontend_requirement_template_from_taskplan_request(&fixture, taskplan_request_ref);
    write_json_atomic(&group_path, &group_value).expect("write enriched group file");

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "task.requirementDetailRefs".to_string(),
            "task.verificationIntents".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"
                .to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read task execution fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let expected_task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId")
        .to_string();
    let expected_task_id = fields["source.taskId"]
        .value
        .as_str()
        .expect("taskId")
        .to_string();
    let expected_detail_id = fields["task.requirementDetailRefs"].value[0]
        .as_str()
        .expect("detail id")
        .to_string();
    let expected_verification_id = fields["task.verificationIntents"].value[0]["verificationId"]
        .as_str()
        .expect("verification id")
        .to_string();
    let expected_closure_id = fields
        ["task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"]
        .value[0]["closureId"]
        .as_str()
        .expect("closure id")
        .to_string();
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["taskPlanId"] = json!("wrong-taskplan");
    result["taskId"] = json!("wrong-task");
    result["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut result);
    result["verificationResults"][0]["verificationId"] = json!("wrong-verification");
    result["verificationResults"][0]["evidenceType"] = json!("runtime_api_check");
    result["failure"] = json!({
        "code": "STALE_FAILURE",
        "summary": "This stale failure must not force a repair for a completed result."
    });
    result["requirementDetailEvidence"][0]["detailId"] = json!("wrong-detail");
    result["requirementDetailEvidence"][0]["verificationIds"] = json!(["wrong-verification"]);
    result["conceptEvidence"][0]["conceptRef"] = json!("wrong-concept");
    result["frontendExperienceSelfCheck"]["closureRequirementIds"] = json!(["wrong-closure"]);
    write_json_atomic(&fixture.root.join(result_file), &result)
        .expect("write machine-ref-invalid task result");

    let accepted_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        accepted_result["state"], "auto_runnable",
        "{accepted_result:#}"
    );
    assert_ne!(
        accepted_result["next"]["artifactKind"],
        json!("task_result_repair"),
        "{accepted_result:#}"
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    let persisted_ref = index["phases"][0]["latestRefs"]["latestTaskResult"]
        .as_str()
        .expect("latest task result ref");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(persisted_ref)).unwrap())
            .expect("parse persisted task result");
    assert_eq!(persisted["taskPlanId"], json!(expected_task_plan_id));
    assert_eq!(persisted["taskId"], json!(expected_task_id));
    assert_eq!(
        persisted["verificationResults"][0]["verificationId"],
        json!(expected_verification_id)
    );
    assert!(persisted.get("failure").is_none());
    assert_eq!(
        persisted["requirementDetailEvidence"][0]["detailId"],
        json!(expected_detail_id)
    );
    assert_eq!(
        persisted["requirementDetailEvidence"][0]["verificationIds"],
        json!([expected_verification_id])
    );
    assert_eq!(
        persisted["conceptEvidence"][0]["conceptRef"],
        json!("concept-account-ui")
    );
    assert_eq!(
        persisted["frontendExperienceSelfCheck"]["closureRequirementIds"],
        json!([expected_closure_id])
    );
    assert!(
        persisted["frontendExperienceSelfCheck"]["dataBinding"]
            .get("closureRequirementIds")
            .is_none(),
        "dataBinding must not duplicate top-level closure ids"
    );
}

#[test]
fn task_result_contract_omits_and_strips_non_applicable_architecture_quality_evidence() {
    let fixture = Fixture::new("task-result-strip-non-applicable-architecture-quality");
    let execution_request_ref =
        start_frontend_quality_task_execution_without_architecture_quality(&fixture);
    let request_id = request_id_from_ref(&execution_request_ref);
    let output_contract_ref =
        state::request_manifest::request_storage_ref(&fixture.root, &request_id, "outputContract")
            .expect("output contract storage lookup")
            .expect("output contract storage ref");
    let output_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(output_contract_ref))
            .expect("read output contract"),
    )
    .expect("parse output contract");
    assert!(
        output_contract["schemaShape"]["properties"]
            .get("architectureQualityEvidence")
            .is_none(),
        "non-applicable architectureQualityEvidence must not be exposed in schemaShape: {output_contract:#}"
    );
    assert!(
        output_contract["resultTemplate"]
            .get("architectureQualityEvidence")
            .is_none(),
        "non-applicable architectureQualityEvidence must not be exposed in resultTemplate"
    );

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution contract")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["taskResultId"] = json!("result-with-extra-architecture-quality");
    result["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut result);
    result["architectureQualityEvidence"] = json!([{
        "requirementId": "not-assigned",
        "status": "satisfied",
        "verificationIds": ["verify-account-ui-001"],
        "changedFiles": ["src/App.tsx"],
        "summary": "This field is not applicable to the task and should be stripped before validation."
    }]);
    write_json_atomic(&fixture.root.join(result_file), &result)
        .expect("write task result with non-applicable evidence");

    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_ne!(
        accepted["next"]["artifactKind"],
        json!("task_result_repair")
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let persisted_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "latestTaskResult");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(persisted_ref)).unwrap())
            .expect("parse persisted task result");
    assert!(persisted.get("architectureQualityEvidence").is_none());
}

#[test]
fn task_result_submit_routes_invalid_frontend_surface_shape_to_quality_repair() {
    let fixture = Fixture::new("task-result-invalid-frontend-surface-shape");
    let execution_request_ref =
        start_frontend_quality_task_execution_without_architecture_quality(&fixture);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution contract")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["taskResultId"] = json!("result-invalid-surface-shape");
    result["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut result);
    result["frontendQualitySelfCheck"]["surfacesCovered"] = json!(["surface_account_admin"]);
    write_json_atomic(&fixture.root.join(result_file), &result)
        .expect("write task result with invalid surface shape");

    let rejected = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(rejected["state"], "auto_runnable", "{rejected:#}");
    assert_eq!(rejected["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = rejected["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["repairContract.issueConflicts".to_string()],
    })
    .expect("read repair issue conflicts")
    .fields;
    assert!(repair_fields["repairContract.issueConflicts"]
        .value
        .as_array()
        .expect("issue conflicts")
        .iter()
        .any(
            |issue| issue["code"] == "TASK_RESULT_FRONTEND_QUALITY_INVALID"
                && issue["fieldPath"] == "frontendQualitySelfCheck.surfacesCovered"
        ));
}

#[test]
fn task_result_submit_fills_single_intent_detail_verification_ids() {
    let fixture = Fixture::new("task-result-single-intent-detail-fallback");
    let execution_request_ref = start_planned_task_execution(&fixture);
    let request_id = execution_request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let task_ref = state::request_manifest::request_storage_ref(&fixture.root, request_id, "task")
        .expect("task storage ref")
        .expect("task ref");
    let task_path = fixture.root.join(task_ref);
    let mut task_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&task_path).expect("read task ref"))
            .expect("parse task ref");
    let verification_id = task_value["verificationIntents"][0]["verificationId"]
        .as_str()
        .expect("single verification id")
        .to_string();
    let legacy_detail_id = "detail.legacy.single_intent";
    task_value["requirementDetailRefs"]
        .as_array_mut()
        .expect("task detail refs")
        .push(json!(legacy_detail_id));
    assert!(
        !task_value["verificationIntents"][0]["requirementDetailRefs"]
            .as_array()
            .expect("verification detail refs")
            .iter()
            .any(|item| item == &json!(legacy_detail_id))
    );
    write_json_atomic(&task_path, &task_value).expect("write legacy-style task ref");

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
            "outputContract.resultRules".to_string(),
        ],
    })
    .expect("read execution fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let result_rules_text = fields["outputContract.resultRules"].value.to_string();
    assert!(result_rules_text.contains("do not leave verificationIds empty"));
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["taskResultId"] = json!("result-single-intent-fallback");
    result["taskPlanId"] = fields["source.taskPlanId"].value.clone();
    result["taskId"] = fields["source.taskId"].value.clone();
    result["changedFiles"] = json!(["src/main.tsx"]);
    if let Some(self_check) = result
        .get_mut("frontendExperienceSelfCheck")
        .and_then(Value::as_object_mut)
    {
        self_check.insert("evidenceRefs".to_string(), json!(["src/main.tsx"]));
        self_check.insert(
            "summary".to_string(),
            json!("The task-owned frontend flow evidence remains satisfied while legacy detail evidence is normalized."),
        );
    }
    complete_frontend_quality_token_evidence_for_test(&mut result);
    result["requirementDetailEvidence"]
        .as_array_mut()
        .expect("detail evidence")
        .push(json!({
            "detailId": legacy_detail_id,
            "status": "satisfied",
            "verificationIds": [],
            "evidenceRefs": [],
            "summary": "Legacy detail is covered by the task's only verification intent."
        }));
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write task result");

    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_ne!(
        accepted["next"]["artifactKind"],
        json!("task_result_repair"),
        "{accepted:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let latest_result_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "latestTaskResult");
    let persisted: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(latest_result_ref)).expect("read task result"),
    )
    .expect("parse persisted task result");
    let legacy_evidence = persisted["requirementDetailEvidence"]
        .as_array()
        .expect("persisted detail evidence")
        .iter()
        .find(|item| item["detailId"] == json!(legacy_detail_id))
        .expect("legacy evidence");
    assert_eq!(legacy_evidence["verificationIds"], json!([verification_id]));
}

#[test]
fn task_result_submit_normalizes_invalid_no_change_reason_type() {
    let fixture = Fixture::new("task-result-no-change-reason-type");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_task_result_candidate(&fixture, &execution_request_ref);
    mutate_task_result_candidate(&fixture, &execution_request_ref, |result| {
        result["noChangeReason"] = json!("verification task changed no files");
    });

    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_ne!(
        accepted["next"]["artifactKind"],
        json!("task_result_repair"),
        "{accepted:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let latest_result_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "latestTaskResult");
    let persisted: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(latest_result_ref)).expect("read task result"),
    )
    .expect("parse persisted task result");
    assert_eq!(persisted["noChangeReason"], Value::Null);
}

#[test]
fn task_result_rejects_placeholder_jvm_production_package() {
    let fixture = Fixture::new("task-result-placeholder-jvm-package");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_with_persistence_quality(&fixture, taskplan_request_ref);
    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");
    let quality = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        group_id: "task_execution_quality_context".to_string(),
    })
    .expect("read execution quality context")
    .fields;
    assert!(!quality.contains_key("sourceContext.codeQualityRequirements"));
    assert!(
        quality["sourceContext.codeQualityExecutionContext"].value[0]["packageNamingPolicy"]
            ["forbiddenPackagePrefixes"]
            .as_array()
            .expect("forbidden package prefixes")
            .contains(&json!("com.example"))
    );

    let java_file = "server/src/main/java/com/example/replenishment/api/HealthController.java";
    let java_path = fixture.root.join(java_file);
    std::fs::create_dir_all(java_path.parent().expect("java parent"))
        .expect("create java source parent");
    std::fs::write(
        &java_path,
        "package com.example.replenishment.api;\n\npublic class HealthController {}\n",
    )
    .expect("write placeholder java package");

    write_task_result_candidate(&fixture, execution_request_ref);
    mutate_task_result_candidate(&fixture, execution_request_ref, |result| {
        result["changedFiles"] = json!([java_file]);
        if let Some(items) = result
            .get_mut("codeQualityEvidence")
            .and_then(Value::as_array_mut)
        {
            for item in items {
                item["changedFiles"] = json!([java_file]);
                item["summary"] = json!(
                    "The Java file was checked against the selected code quality references."
                );
            }
        }
    });

    let result = call_submit(
        "loom.recordTaskResultFile",
        execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repair requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["repairContract.issueConflicts".to_string()],
    })
    .expect("read task result repair issues")
    .fields;
    assert!(repair_fields["repairContract.issueConflicts"]
        .value
        .as_array()
        .expect("issue conflicts")
        .iter()
        .any(|issue| {
            issue["code"] == "TASK_RESULT_CODE_QUALITY_INVALID"
                && issue["fieldPath"] == "changedFiles"
                && issue["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("com.example.replenishment.api"))
        }));
}

#[test]
fn task_result_rejects_satisfied_render_gate_without_mobile_viewport() {
    let fixture = Fixture::new("task-result-render-gate-viewports");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_task_result_candidate(&fixture, &execution_request_ref);
    mutate_task_result_candidate(&fixture, &execution_request_ref, |result| {
        let gates = result["frontendQualitySelfCheck"]["gateResults"]
            .as_array_mut()
            .expect("gate results");
        let render_gate = gates
            .iter_mut()
            .find(|gate| gate["gateId"] == json!("verify.rendered_viewports"))
            .expect("render gate");
        render_gate["status"] = json!("satisfied");
        render_gate["viewportsChecked"] = json!(["desktop 1280x800"]);
    });

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repair requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["repairContract.issueConflicts".to_string()],
    })
    .expect("read task result repair issues")
    .fields;
    assert!(repair_fields["repairContract.issueConflicts"]
        .value
        .as_array()
        .expect("issue conflicts")
        .iter()
        .any(|issue| {
            issue["code"] == "TASK_RESULT_FRONTEND_QUALITY_INVALID"
                && issue["fieldPath"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".viewportsChecked"))
        }));
}

#[test]
fn taskplan_accept_materializes_task_execution_and_task_result_routes_review() {
    let fixture = Fixture::new("taskplan-execution-chain");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    assert_eq!(
        execution_result["stopAllowed"],
        json!(false),
        "{execution_result:#}"
    );
    assert!(execution_result.get("continuationPolicy").is_none());
    assert!(execution_result["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("Do not stop at a progress recap"));
    assert_eq!(execution_result["next"]["kind"], "execute_task");
    assert_eq!(
        execution_result["next"]["submitTool"],
        "loom.recordTaskResultFile"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let resumed_execution = continue_delivery(fixture.root_str());
    assert_eq!(resumed_execution["state"], "auto_runnable");
    assert_eq!(resumed_execution["next"]["kind"], "execute_task");
    assert_eq!(
        resumed_execution["next"]["requestRef"],
        json!(execution_request_ref)
    );
    assert_eq!(
        resumed_execution["stopAllowed"],
        json!(false),
        "{resumed_execution:#}"
    );
    assert!(resumed_execution.get("continuationPolicy").is_none());
    let execution_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
    })
    .expect("inspect execution request");
    assert_eq!(execution_inspected.request_kind, "task_execution_request");
    let execution_request_root =
        read_request_root_value(fixture.root_str(), &execution_request_ref);
    assert!(
        execution_request_root
            .pointer("/sourceContext/dependencyResults")
            .is_none(),
        "{execution_request_root:#}"
    );
    let execution_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "source.taskId".to_string(),
            "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource"
                .to_string(),
            "executionRules.verificationCommandSchedulingRules".to_string(),
            "executionRules.boundaryRules".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution result template")
    .fields;
    assert_eq!(
        execution_fields["outputContract.resultTemplate"].value["taskId"],
        execution_fields["source.taskId"].value
    );
    assert!(
        execution_fields["outputContract.resultTemplate"].value["verificationResults"][0]
            .is_object()
    );
    assert!(
        execution_fields["outputContract.resultTemplate"].value["executionContinuity"].is_object()
    );
    let workflow_closure_detail_source = &execution_fields
        ["task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource"]
        .value;
    assert!(
        workflow_closure_detail_source.get("sourcePaths").is_none(),
        "{workflow_closure_detail_source:#}"
    );
    assert!(
        workflow_closure_detail_source.get("readWhen").is_none(),
        "{workflow_closure_detail_source:#}"
    );
    assert!(
        workflow_closure_detail_source
            .get("detailAuthority")
            .is_some(),
        "{workflow_closure_detail_source:#}"
    );
    assert!(execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(!execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!execution_fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("frontendExperienceSelfCheck")
        .is_some());
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("runtimeDeliveryEvidence")
        .is_none());
    assert!(execution_fields["outputContract.resultTemplate"]
        .value
        .get("conceptEvidence")
        .is_none());
    let execution_rules_text =
        serde_json::to_string(&execution_fields).expect("serialize execution rules");
    assert!(execution_rules_text.contains("write-producing verification commands"));
    assert!(execution_rules_text.contains("confirmed business language"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "task_execution_result_contract"));
    assert!(!execution_inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "task_execution_optional_refs"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(|field| field != "outputContract.schemaShape"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(
            |field| !field.starts_with("sourceRefs.") && field != "sourceContext.dependencyResults"
        ));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(
            |field| field != "executionRules.runtimeDeliveryExecutionRules"
                && field != "task.runtimeDeliveryRequirement"
                && field != "outputContract.schemaShape.properties.runtimeDeliveryEvidence"
                && field != "outputContract.schemaShape.properties.conceptEvidence"
        ));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "executionRules.frontendImplementationOrganizationRules"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "executionRules.interactiveVerificationProbePolicy"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "executionRules.controlledRuntimeProbeRules"));
    assert!(execution_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "outputContract.schemaShape.properties.frontendExperienceSelfCheck"));

    write_task_result_candidate_without_requirement_detail_evidence(
        &fixture,
        &execution_request_ref,
    );
    let invalid_task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        invalid_task_result["state"], "auto_runnable",
        "{invalid_task_result:#}"
    );
    assert_eq!(invalid_task_result["next"]["kind"], "write_artifact");
    assert_eq!(
        invalid_task_result["next"]["artifactKind"],
        "task_result_repair"
    );
    assert_eq!(
        invalid_task_result["next"]["submitTool"],
        "loom.repairSubmitFile"
    );

    let task_result_repair_action_ref = invalid_task_result["next"]["requestRef"]
        .as_str()
        .expect("task result repair action requestRef")
        .to_string();
    let resumed_task_result_repair = continue_delivery(fixture.root_str());
    assert_eq!(
        resumed_task_result_repair["state"], "auto_runnable",
        "{resumed_task_result_repair:#}"
    );
    assert_eq!(
        resumed_task_result_repair["next"]["requestRef"],
        json!(task_result_repair_action_ref),
        "{resumed_task_result_repair:#}"
    );
    assert_eq!(
        resumed_task_result_repair["next"]["artifactKind"],
        "task_result_repair"
    );
    let task_result_repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: task_result_repair_action_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read task result repair template")
    .fields;
    assert!(
        task_result_repair_fields["outputContract.resultTemplate"].value["verificationResults"][0]
            .is_object()
    );
    assert_eq!(
        task_result_repair_fields["outputContract.resultTemplate"].value["changedFiles"],
        json!(["src/main.tsx"])
    );
    assert!(
        task_result_repair_fields["outputContract.resultTemplate"].value
            ["requirementDetailEvidence"][0]
            .is_object()
    );
    let task_result_repair_root =
        read_request_root_value(fixture.root_str(), &task_result_repair_action_ref);
    assert!(task_result_repair_root["source"].get("issues").is_none());
    let task_result_repair_read_plan_fields = task_result_repair_root["requestReadPlan"]["groups"]
        .as_array()
        .expect("repair read groups")
        .iter()
        .flat_map(read_group_fields_from_json)
        .collect::<Vec<_>>();
    assert!(
        task_result_repair_read_plan_fields.contains(&"outputContract.resultTemplate".to_string())
    );
    assert!(
        task_result_repair_read_plan_fields.contains(&"repairContract.issueConflicts".to_string())
    );
    assert!(task_result_repair_read_plan_fields
        .contains(&"repairContract.minimalRepairRules".to_string()));
    assert!(!task_result_repair_read_plan_fields.contains(&"source.issues".to_string()));
    let repair_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: task_result_repair_action_ref.clone(),
        group_id: "task_result_repair_context".to_string(),
    })
    .expect("read task result repair context");
    assert_eq!(
        repair_contract.fields["repairContract.profile"].value,
        json!("minimal_task_result_repair")
    );
    assert!(repair_contract.fields["repairContract.issueConflicts"]
        .value
        .is_array());
    write_large_task_result_candidate(&fixture, &task_result_repair_action_ref);
    let task_result = call_submit(
        "loom.repairSubmitFile",
        &task_result_repair_action_ref,
        fixture.root_str(),
    );

    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    assert_eq!(task_result["next"]["kind"], "execute_task");
    assert_eq!(
        task_result["next"]["submitTool"],
        "loom.recordTaskResultFile"
    );
    let closure_execution_ref = task_result["next"]["requestRef"]
        .as_str()
        .expect("closure execution requestRef")
        .to_string();
    write_task_result_candidate(&fixture, &closure_execution_ref);
    let task_result = call_submit(
        "loom.recordTaskResultFile",
        &closure_execution_ref,
        fixture.root_str(),
    );
    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    assert_eq!(task_result["next"]["kind"], "write_artifact");
    assert_eq!(task_result["next"]["artifactKind"], "review_result");
    assert_eq!(task_result["next"]["submitTool"], "loom.reviewAcceptFile");
    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    let latest_refs = index["phases"][0]["latestRefs"]
        .as_object()
        .expect("latest refs");
    assert!(
        !latest_refs.contains_key("activeTaskResultRepairActionRef"),
        "accepted repair must clear stale activeTaskResultRepairActionRef: {latest_refs:#?}"
    );
    let review_request_ref = task_result["next"]["requestRef"]
        .as_str()
        .expect("review requestRef");
    let review_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
    })
    .expect("inspect review request");
    assert_eq!(review_inspected.request_kind, "review_request");
    let review_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read review result template")
    .fields;
    let review_root = read_request_root_value(fixture.root_str(), review_request_ref);
    assert!(
        review_root.get("reviewPacket").is_none(),
        "review request root must not expose full reviewPacket: {review_root:#}"
    );
    assert!(
        review_root.get("changeContext").is_none(),
        "review request root must not expose full changeContext: {review_root:#}"
    );
    assert!(
        review_root.get("changeSet").is_none(),
        "review request root must not expose full changeSet: {review_root:#}"
    );
    assert!(
        review_root.get("reviewSignals").is_none(),
        "review request root must not duplicate outputContract.reviewSignals: {review_root:#}"
    );
    let review_request_id = request_id_from_ref(review_request_ref);
    let review_storage_manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(
            fixture
                .root
                .join(format!(".loom/requests/{review_request_id}.manifest.json")),
        )
        .expect("read review private storage manifest"),
    )
    .expect("parse review private storage manifest");
    let review_manifest_refs = review_storage_manifest["refs"]
        .as_object()
        .expect("review manifest refs");
    assert!(review_manifest_refs.contains_key("reviewPacket"));
    assert!(review_manifest_refs.contains_key("changeContext"));
    assert!(review_manifest_refs.contains_key("outputContract"));
    assert_eq!(
        review_fields["outputContract.resultTemplate"].value["source"]["requestId"],
        review_root["requestId"]
    );
    let review_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_rules".to_string(),
    })
    .expect("read review rules");
    let review_rules_text =
        serde_json::to_string(&review_rules.fields).expect("serialize review rules");
    assert!(review_rules_text.contains("spec fidelity and project standards"));
    assert!(review_rules_text.contains("smallest repair"));
    assert!(review_fields["outputContract.resultTemplate"].value["coverageAssessment"].is_object());
    let template_evidence_ref =
        &review_fields["outputContract.resultTemplate"].value["findings"][0]["evidenceRefs"][0];
    assert_eq!(template_evidence_ref["type"], "task_result");
    assert!(template_evidence_ref["ref"].is_string());
    assert!(template_evidence_ref["reason"].is_string());
    let review_group_ids = review_inspected
        .read_groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<Vec<_>>();
    let review_packet_group = review_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "review_packets")
        .expect("review_packets group");
    let review_packet_fields = review_packet_group.expanded_fields();
    assert!(review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.groupSummaries"));
    assert!(review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.taskSummaries"));
    assert!(review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.taskResultSummaries"));
    assert!(!review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.groups"));
    assert!(!review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.tasks"));
    assert!(!review_packet_fields
        .iter()
        .any(|field| field == "reviewPacket.taskResults"));
    let review_matrices_group = review_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "review_matrices")
        .expect("review_matrices group");
    let review_matrices_fields = review_matrices_group.expanded_fields();
    assert!(review_matrices_fields
        .iter()
        .any(|field| field == "outputContract.reviewSignals.items"));
    assert!(!review_matrices_fields
        .iter()
        .any(|field| field.starts_with("reviewSignals.")));
    assert!(!review_matrices_fields.iter().any(|field| {
        field == "outputContract.reviewSignals.requirementDetailEvidence"
            || field == "outputContract.reviewSignals.frontendWorkflowClosure"
    }));
    let review_quality_group = review_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "review_quality_profile")
        .expect("review_quality_profile group");
    let review_quality_fields = review_quality_group.expanded_fields();
    assert!(review_quality_fields
        .iter()
        .any(|field| field == "reviewQualityProfile.referenceLoadPlan"));
    assert!(review_quality_fields
        .iter()
        .any(|field| field == "reviewQualityProfile.reviewStageOrder"));
    assert!(!review_quality_fields
        .iter()
        .any(|field| field == "reviewQualityProfile"));
    let review_quality = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_quality_profile".to_string(),
    })
    .expect("read review quality profile");
    let review_references = review_quality.fields["reviewQualityProfile.referenceLoadPlan"]
        .value
        .as_array()
        .expect("review reference load plan");
    assert!(review_references
        .iter()
        .any(|item| item["path"] == json!("tech/review/core.md")));
    assert!(review_references
        .iter()
        .any(|item| item["path"] == json!("tech/review/spec-compliance.md")));
    assert!(review_references
        .iter()
        .any(|item| item["path"] == json!("tech/review/defect-patterns.md")));
    assert!(review_references
        .iter()
        .any(|item| item["path"] == json!("tech/review/test-evidence.md")));
    assert!(review_references
        .iter()
        .any(|item| item["path"] == json!("tech/review/finding-quality.md")));
    let review_quality_text =
        serde_json::to_string(&review_quality.fields).expect("serialize review profile");
    assert!(!review_quality_text.contains("SQL Injection"));
    assert!(!review_quality_text.contains("Full Review Report Template"));
    let review_matrices = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_matrices".to_string(),
    })
    .expect("read review matrices");
    let review_matrices_text =
        serde_json::to_string(&review_matrices.fields).expect("serialize review matrices");
    assert!(!review_matrices_text.contains("referenceLoadPlan"));
    assert!(!review_matrices_text.contains("referenceFilesChecked"));
    assert!(!review_matrices_text.contains("referenceGroupsChecked"));
    let review_signals = review_matrices.fields["outputContract.reviewSignals.items"]
        .value
        .as_array()
        .expect("review signals");
    assert!(review_signals
        .iter()
        .any(|signal| signal["kind"] == json!("task_run_summary")));
    assert!(review_signals
        .iter()
        .any(|signal| signal["kind"] == json!("requirement_detail_evidence")));
    let review_packets = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.to_string(),
        group_id: "review_packets".to_string(),
    })
    .expect("read review packets");
    assert!(review_packets.fields["reviewPacket.groupSummaries"]
        .value
        .is_array());
    assert!(review_packets.fields["reviewPacket.taskSummaries"]
        .value
        .is_array());
    assert!(review_packets.fields["reviewPacket.taskResultSummaries"]
        .value
        .is_array());
    let review_packets_text =
        serde_json::to_string(&review_packets.fields).expect("serialize review packets");
    assert!(!review_packets_text.contains("very large execution note"));
    assert!(!review_packets_text.contains("very large verification summary"));
    assert!(!review_packets_text.contains("very large detail evidence summary"));
    assert!(!review_packets_text.contains("referenceFilesChecked"));
    assert!(!review_packets_text.contains("referenceGroupsChecked"));
    assert!(!review_packets_text.contains("evidenceRefs"));
    assert!(
        serde_json::to_vec_pretty(&review_packets.fields["reviewPacket.taskResultSummaries"].value)
            .expect("serialize task result summaries")
            .len()
            < 32 * 1024
    );
    assert!(!review_packets.fields.contains_key("reviewPacket.groups"));
    assert!(!review_packets.fields.contains_key("reviewPacket.tasks"));
    assert!(!review_packets
        .fields
        .contains_key("reviewPacket.taskResults"));
    assert_eq!(
        review_group_ids,
        vec![
            "review_scope",
            "review_packets",
            "change_context",
            "review_matrices",
            "review_quality_profile",
            "review_rules",
            "review_write_contract"
        ]
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    assert_eq!(index["phases"][0]["nextAction"]["kind"], "review");
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("tasks/phase-1/taskplans/latest.json")
        .exists());
    assert!(fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("tasks/phase-1/runs/latest.json")
        .exists());
    assert_no_read_plan_size_warnings(&fixture);
}

#[test]
fn taskplan_accept_normalizes_forbidden_paths_before_execution() {
    let fixture = Fixture::new("taskplan-normalizes-forbidden-paths");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);

    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["writeBoundary"]["forbiddenPaths"] = json!(["node_modules", "dist"]);
    write_json_atomic(&group_path, &group_value).expect("write incomplete forbidden paths");

    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");
    let execution_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        fields: vec!["task.writeBoundary.forbiddenPaths".to_string()],
    })
    .expect("read execution write boundary")
    .fields;
    assert_eq!(
        execution_fields["task.writeBoundary.forbiddenPaths"].value,
        json!([".loom"])
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    assert!(persisted["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .all(|task| task["writeBoundary"]["forbiddenPaths"] == json!([".loom"])));
}

#[test]
fn taskplan_accept_materializes_persistence_engineering_quality_requirements() {
    let fixture = Fixture::new("taskplan-engineering-quality");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_with_persistence_quality(&fixture, taskplan_request_ref);

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_eq!(accepted["next"]["kind"], "execute_task");

    let delivery_id = request_delivery_id(fixture.root_str(), taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let taskplan: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(taskplan_ref)).expect("read taskplan"),
    )
    .expect("parse taskplan");

    let requirements = taskplan["engineeringQualityRequirements"]
        .as_array()
        .expect("engineering quality requirements");
    assert_eq!(requirements.len(), 1, "{taskplan:#}");
    let requirement = &requirements[0];
    assert_eq!(
        requirement["requirementId"],
        json!("eqr-persistence-mapping-001")
    );
    assert_eq!(requirement["kind"], json!("persistence_mapping"));
    assert_eq!(
        requirement["appliesToTaskIds"],
        json!(["task-backend-data-001", "task-backend-api-001"])
    );
    assert!(requirement["stackSignals"].get("persistence").is_some());
    assert!(
        requirement["stackSignals"].get("migrationTool").is_none(),
        "absent migration tool must not be invented: {requirement:#}"
    );
    assert!(requirement["alignmentTargets"]
        .as_array()
        .expect("alignment targets")
        .contains(&json!("storage_schema_field")));
    assert!(requirement["riskFieldKinds"]
        .as_array()
        .expect("risk fields")
        .contains(&json!("datetime")));

    let tasks = taskplan["tasks"].as_array().expect("tasks");
    let backend_data = tasks
        .iter()
        .find(|task| task["taskId"] == json!("task-backend-data-001"))
        .expect("backend data task");
    let backend_api = tasks
        .iter()
        .find(|task| task["taskId"] == json!("task-backend-api-001"))
        .expect("backend api task");
    let frontend = tasks
        .iter()
        .find(|task| task["taskId"] == json!("task-frontend-001"))
        .expect("frontend task");
    assert_eq!(
        backend_data["engineeringQualityRequirementRefs"],
        json!(["eqr-persistence-mapping-001"])
    );
    assert_eq!(
        backend_api["engineeringQualityRequirementRefs"],
        json!(["eqr-persistence-mapping-001"])
    );
    assert_eq!(
        backend_api["apiContractRequirementRefs"],
        json!(["api-contract-task-backend-api-001"])
    );
    assert!(
        frontend.get("engineeringQualityRequirementRefs").is_none(),
        "pure frontend task must not receive persistence quality refs: {frontend:#}"
    );
    assert!(
        frontend.get("apiContractRequirementRefs").is_none(),
        "frontend API binding task must not receive API contract owner refs: {frontend:#}"
    );
    let code_requirements = taskplan["codeQualityRequirements"]
        .as_array()
        .expect("code quality requirements");
    let backend_data_code_requirement = code_requirements
        .iter()
        .find(|requirement| {
            requirement["appliesToTaskIds"]
                .as_array()
                .is_some_and(|items| items.contains(&json!("task-backend-data-001")))
        })
        .expect("backend data code quality requirement");
    assert_eq!(
        backend_data_code_requirement["packageNamingPolicy"]["fallbackPackageTemplate"],
        json!("app.<project_slug>")
    );
    assert_eq!(
        backend_data_code_requirement["packageNamingPolicy"]["absoluteFallbackPackage"],
        json!("app.generated")
    );
    assert!(
        backend_data_code_requirement["packageNamingPolicy"]["forbiddenPackagePrefixes"]
            .as_array()
            .expect("forbidden package prefixes")
            .contains(&json!("com.example"))
    );
    assert!(backend_data_code_requirement["implementationObligations"]
        .as_array()
        .expect("implementation obligations")
        .iter()
        .any(|obligation| obligation
            .as_str()
            .is_some_and(|text| text.contains("app.<project_slug>"))));

    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
    })
    .expect("inspect execution request");
    let core_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_core")
        .expect("task execution core")
        .expanded_fields();
    assert!(!core_fields.contains(&"task.engineeringQualityRequirementRefs".to_string()));
    assert!(!core_fields.contains(&"engineeringQualityRequirements".to_string()));
    let quality_fields = inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "task_execution_quality_context")
        .expect("task execution quality context")
        .expanded_fields();
    assert!(quality_fields.contains(&"task.engineeringQualityRequirementRefs".to_string()));
    assert!(quality_fields.contains(&"sourceContext.engineeringQualityRequirements".to_string()));
    assert!(quality_fields.contains(&"executionRules.engineeringQualityExecutionRules".to_string()));

    let core = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        group_id: "task_execution_quality_context".to_string(),
    })
    .expect("read task execution quality context")
    .fields;
    assert_eq!(
        core["task.engineeringQualityRequirementRefs"].value,
        json!(["eqr-persistence-mapping-001"])
    );
    assert_eq!(
        core["sourceContext.engineeringQualityRequirements"].value[0]["appliesToTaskIds"],
        json!(["task-backend-data-001", "task-backend-api-001"])
    );
    assert!(
        core["executionRules.engineeringQualityExecutionRules"].value["implementationRules"]
            .as_array()
            .is_some_and(|rules| !rules.is_empty())
    );
}

#[test]
fn review_request_carries_engineering_quality_signals() {
    let fixture = Fixture::new("review-engineering-quality");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_with_persistence_quality(&fixture, taskplan_request_ref);
    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");

    let mut execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let mut review_request_ref = None;
    for _ in 0..8 {
        write_task_result_candidate(&fixture, &execution_request_ref);
        let result = call_submit(
            "loom.recordTaskResultFile",
            &execution_request_ref,
            fixture.root_str(),
        );
        assert_eq!(result["state"], "auto_runnable", "{result:#}");
        if result["next"]["artifactKind"] == json!("review_result") {
            review_request_ref = Some(
                result["next"]["requestRef"]
                    .as_str()
                    .expect("review requestRef")
                    .to_string(),
            );
            break;
        }
        assert_eq!(result["next"]["kind"], "execute_task", "{result:#}");
        execution_request_ref = result["next"]["requestRef"]
            .as_str()
            .expect("next execution requestRef")
            .to_string();
    }
    let review_request_ref = review_request_ref.expect("execution reaches review request");

    let review_matrices = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref,
        group_id: "review_matrices".to_string(),
    })
    .expect("read review matrices")
    .fields;
    let matrix = review_matrices["reviewMatrixSummary.engineeringQuality"]
        .value
        .as_array()
        .expect("engineering quality matrix summary");
    assert_eq!(matrix.len(), 2, "{matrix:#?}");
    assert!(matrix.iter().all(|item| {
        item["requirementId"] == json!("eqr-persistence-mapping-001")
            && item["qualitySatisfied"] == json!(true)
            && item["recommendedNextAction"] == json!("none")
    }));
    assert!(!matrix
        .iter()
        .any(|item| item["taskId"] == json!("task-frontend-001")));
    let signals = review_matrices["outputContract.reviewSignals.items"]
        .value
        .as_array()
        .expect("review signals");
    assert!(signals.iter().any(|signal| {
        signal["kind"] == json!("engineering_quality")
            && signal["requirementId"] == json!("eqr-persistence-mapping-001")
            && signal["qualitySatisfied"] == json!(true)
    }));
}

#[test]
fn taskplan_accept_normalizes_runtime_closure_into_final_group() {
    let fixture = Fixture::new("taskplan-normalizes-runtime-closure-group");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec![
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan file fields")
    .fields;
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file")
        .to_string();
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group pattern")
        .to_string();
    let outline_path = fixture.root.join(&outline_file);
    let mut outline: Value =
        serde_json::from_str(&std::fs::read_to_string(&outline_path).expect("read outline"))
            .expect("parse outline");
    let groups = outline["groups"].as_array().expect("outline groups");
    assert!(
        groups.len() >= 2,
        "runtime closure fixture should create separate groups"
    );
    let implementation_group_id = groups[0]["groupId"]
        .as_str()
        .expect("implementation group id")
        .to_string();
    let closure_group_id = groups
        .iter()
        .find_map(|group| {
            let group_id = group["groupId"].as_str()?;
            let group_file = group_pattern.replace("{groupId}", group_id);
            let group_value: Value =
                serde_json::from_str(&std::fs::read_to_string(fixture.root.join(group_file)).ok()?)
                    .ok()?;
            group_value["tasks"]
                .as_array()
                .is_some_and(|tasks| {
                    tasks
                        .iter()
                        .any(|task| task["taskKind"] == json!("runtime_delivery_closure"))
                })
                .then_some(group_id.to_string())
        })
        .expect("closure group id");
    let closure_group_file = group_pattern.replace("{groupId}", &closure_group_id);
    let closure_group_value: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(&closure_group_file))
            .expect("read closure group"),
    )
    .expect("parse closure group");
    let mut closure_task = closure_group_value["tasks"][0].clone();
    let closure_task_id = closure_task["taskId"]
        .as_str()
        .expect("closure task id")
        .to_string();
    closure_task["groupId"] = json!(implementation_group_id);

    let implementation_group_file = group_pattern.replace("{groupId}", &implementation_group_id);
    let implementation_group_path = fixture.root.join(&implementation_group_file);
    let mut implementation_group_value: Value = serde_json::from_str(
        &std::fs::read_to_string(&implementation_group_path).expect("read implementation group"),
    )
    .expect("parse implementation group");
    implementation_group_value["group"]["taskIds"]
        .as_array_mut()
        .expect("implementation group task ids")
        .push(json!(closure_task_id.clone()));
    implementation_group_value["tasks"]
        .as_array_mut()
        .expect("implementation group tasks")
        .push(closure_task);
    write_json_atomic(&implementation_group_path, &implementation_group_value)
        .expect("write mixed implementation group");
    outline["groups"] = json!([implementation_group_value["group"].clone()]);
    write_json_atomic(&outline_path, &outline).expect("write outline with mixed closure group");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    let persisted_groups = persisted["groups"].as_array().expect("persisted groups");
    let final_group = persisted_groups.last().expect("final group");
    assert_eq!(final_group["taskIds"], json!([closure_task_id]));
    assert_eq!(
        final_group["groupId"],
        json!("group-runtime-delivery-closure")
    );
    assert!(final_group["dependsOn"]
        .as_array()
        .expect("final group dependencies")
        .iter()
        .any(|item| item == &json!(implementation_group_id)));
    let closure_task = persisted["tasks"]
        .as_array()
        .expect("persisted tasks")
        .iter()
        .find(|task| task["taskKind"] == json!("runtime_delivery_closure"))
        .expect("persisted closure task");
    assert_eq!(closure_task["groupId"], final_group["groupId"]);
    assert!(closure_task["dependsOn"]
        .as_array()
        .map_or(true, |items| items.is_empty()));
}

#[test]
fn runtime_task_execution_request_uses_field_level_runtime_rules() {
    let fixture = Fixture::new("task-exec-runtime-rules");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["runtimeDeliveryRequirement"] = json!({
        "appliesToThisTask": true,
        "reason": "This task changes runtime delivery wiring.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "checkId": "check-runtime-wiring",
            "contractField": "runtimeSurfaces",
            "objective": "Verify runtime surface wiring still works.",
            "acceptableEvidence": ["runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": [],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write runtime group file");

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
    })
    .expect("inspect runtime execution request");
    let read_fields = inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .collect::<Vec<_>>();
    assert!(read_fields.contains(&"executionRules.controlledRuntimeProbeRules".to_string()));
    assert!(read_fields.contains(&"executionRules.runtimeDeliveryExecutionRules".to_string()));
    assert!(read_fields
        .contains(&"task.runtimeDeliveryRequirement.requiredCodeLevelChecks".to_string()));
    assert!(!read_fields
        .contains(&"task.runtimeDeliveryRequirement.evidenceExpectedInTaskResult".to_string()));
    assert!(!read_fields.contains(&"task.runtimeDeliveryRequirement.forbiddenActions".to_string()));
    assert!(!read_fields.contains(&"task.runtimeDeliveryRequirement".to_string()));
    assert!(!read_fields
        .contains(&"executionRules.frontendImplementationOrganizationRules".to_string()));

    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.to_string(),
        fields: vec![
            "task.runtimeDeliveryRequirement.requiredCodeLevelChecks".to_string(),
            "executionRules.controlledRuntimeProbeRules".to_string(),
            "executionRules.runtimeDeliveryExecutionRules".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.requiredTopLevelFields".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read runtime execution fields")
    .fields;
    assert_eq!(
        fields["task.runtimeDeliveryRequirement.requiredCodeLevelChecks"].value[0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(
        serde_json::to_string(&fields["executionRules.controlledRuntimeProbeRules"].value)
            .unwrap()
            .contains("foreground blocking verification commands")
    );
    assert_eq!(
        fields["outputContract.resultTemplate"].value["runtimeDeliveryEvidence"]["codeLevelChecks"]
            [0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("runtimeDeliveryEvidence")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("frontendExperienceSelfCheck")));
    assert!(!fields["outputContract.requiredTopLevelFields"]
        .value
        .as_array()
        .expect("required top-level fields")
        .contains(&json!("conceptEvidence")));
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/runtime.ts"]);
    complete_architecture_quality_evidence_for_test(&mut result);
    result["verificationResults"][0]["evidenceType"] = json!("static_check");
    result["runtimeDeliveryEvidence"]["requirementRef"] = json!("wrong-runtime-ref");
    result["runtimeDeliveryEvidence"]["checkedFields"] = json!(["wrong-field"]);
    result["runtimeDeliveryEvidence"]["codeLevelChecks"][0]["checkId"] =
        json!("wrong-runtime-check");
    write_json_atomic(&fixture.root.join(result_file), &result)
        .expect("write runtime machine-ref-invalid task result");

    let accepted_result = call_submit(
        "loom.recordTaskResultFile",
        execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        accepted_result["state"], "auto_runnable",
        "{accepted_result:#}"
    );
    assert_ne!(
        accepted_result["next"]["artifactKind"],
        json!("task_result_repair"),
        "{accepted_result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), execution_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    let persisted_ref = index["phases"][0]["latestRefs"]["latestTaskResult"]
        .as_str()
        .expect("latest task result ref");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(persisted_ref)).unwrap())
            .expect("parse persisted task result");
    assert_eq!(
        persisted["runtimeDeliveryEvidence"]["requirementRef"],
        json!("sourceRefs.architectureArtifactContractRef#/runtimeDelivery")
    );
    assert_eq!(
        persisted["runtimeDeliveryEvidence"]["checkedFields"],
        json!(["runtimeSurfaces"])
    );
    assert_eq!(
        persisted["runtimeDeliveryEvidence"]["codeLevelChecks"][0]["checkId"],
        json!("check-runtime-wiring")
    );
}

#[test]
fn task_result_repair_template_restores_missing_runtime_evidence() {
    let fixture = Fixture::new("task-result-repair-missing-runtime-template");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["runtimeDeliveryRequirement"] = json!({
        "appliesToThisTask": true,
        "reason": "This task changes runtime delivery wiring.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "checkId": "check-runtime-wiring",
            "contractField": "runtimeSurfaces",
            "objective": "Verify runtime surface wiring still works.",
            "acceptableEvidence": ["runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": [],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write runtime group file");

    let accepted = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    let execution_request_ref = accepted["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read runtime result template")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/runtime.ts"]);
    result["runtimeDeliveryEvidence"]["codeLevelChecks"] = json!([]);
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write bad task result");

    let invalid = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(invalid["state"], "auto_runnable", "{invalid:#}");
    assert_eq!(invalid["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = invalid["next"]["requestRef"]
        .as_str()
        .expect("repair request ref")
        .to_string();
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref,
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "repairContract.issueConflicts".to_string(),
        ],
    })
    .expect("read task result repair template")
    .fields;

    assert_eq!(
        repair_fields["outputContract.resultTemplate"].value["runtimeDeliveryEvidence"]
            ["codeLevelChecks"][0]["checkId"],
        json!("check-runtime-wiring")
    );
    assert!(
        serde_json::to_string(&repair_fields["repairContract.issueConflicts"].value)
            .expect("serialize issue conflicts")
            .contains("check-runtime-wiring")
    );
}

#[test]
fn taskplan_request_omits_null_optional_projection_reads() {
    let fixture = Fixture::new("taskplan-no-optional-projections");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let aac_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact");
    let aac_path = fixture.root.join(&aac_ref);
    let mut aac: Value =
        serde_json::from_str(&std::fs::read_to_string(&aac_path).expect("read AAC"))
            .expect("parse AAC");
    aac["frontendExperience"] = Value::Null;
    aac["runtimeDelivery"] = Value::Null;
    write_json_atomic(&aac_path, &aac).expect("write AAC without optional projections");

    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    let latest_refs = index["phases"]
        .as_array_mut()
        .expect("phases")
        .iter_mut()
        .find(|phase| phase["phaseId"].as_str() == Some("phase-1"))
        .expect("phase-1")["latestRefs"]
        .as_object_mut()
        .expect("latestRefs object");
    latest_refs.remove("taskPlanRequestId");
    latest_refs.remove("taskPlanRequestRef");
    write_json_atomic(&index_path, &index).expect("write index without taskplan request");

    let result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &RouteAction {
            kind: RouteActionKind::TaskplanGeneration,
            source: "test".to_string(),
            reason: "regenerate_taskplan_request".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let result = serde_json::to_value(result).expect("serialize taskplan result");
    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    assert!(!inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_optional_projection"));
    assert!(!inspected.read_groups.iter().any(|group| group
        .expanded_fields()
        .iter()
        .any(|field| field == "outputContract.runtimeDeliveryClosureTaskTemplate")));
}

#[test]
fn failed_task_result_routes_to_delivery_execution_repair_before_review() {
    let fixture = Fixture::new("failed-task-result-repair");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_failed_task_result_candidate(&fixture, &execution_request_ref);

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "task_failure");
    assert_eq!(
        result["next"]["repairContext"]["repairOrigin"],
        "task_failure"
    );
    assert_eq!(
        result["next"]["repairContext"]["sourceTaskId"],
        result["next"]["taskId"]
    );
    assert!(
        result["next"]["repairContext"]
            .get("repairRequestRef")
            .is_none(),
        "{result:#}"
    );
    assert!(result["next"]["repairContext"]["failedTaskResultRef"]
        .as_str()
        .unwrap()
        .contains("/tasks/phase-1/results/"));
    assert_eq!(result["next"]["repairContext"]["attemptCount"], 1);
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(
        resumed["next"]["executionKind"],
        "delivery_execution_repair"
    );
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "outputContract.blockedReasonOptions".to_string(),
        ],
    })
    .expect("read execution repair result template")
    .fields;
    assert!(
        repair_fields["outputContract.resultTemplate"].value["verificationResults"][0].is_object()
    );
    assert!(repair_fields["outputContract.blockedReasonOptions"]
        .value
        .is_array());
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(repair_root.get("taskConceptGrounding").is_none());
    assert!(repair_root.get("blockedOutput").is_none());
    assert!(repair_root["repairContext"].get("attemptCount").is_some());
    assert!(repair_root["repairContext"].get("sourceRef").is_some());
    assert!(repair_root["repairContext"].get("findingRefs").is_none());
    let inspected_repair = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
    })
    .expect("inspect task failure repair request");
    let repair_core = inspected_repair
        .read_groups
        .iter()
        .find(|group| group.group_id == "repair_execution_core")
        .expect("repair execution core group");
    let repair_core_fields = repair_core.expanded_fields();
    assert!(repair_core_fields.contains(&"repairContext.attemptCount".to_string()));
    assert!(!repair_core_fields.contains(&"repairContext.findingRefs".to_string()));
    remove_task_result_optional_validation_fields(&fixture, repair_request_ref);
    write_task_result_candidate(&fixture, repair_request_ref);
    let repaired_result = call_submit(
        "loom.recordTaskResultFile",
        repair_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repaired_result["state"], "auto_runnable",
        "{repaired_result:#}"
    );
    assert_ne!(
        repaired_result["error"]["code"], "FIELD_NOT_ALLOWED",
        "{repaired_result:#}"
    );
}

#[test]
fn task_result_requires_notes_for_unknown_long_running_work() {
    let fixture = Fixture::new("task-result-unknown-long-running-work");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_task_result_candidate(&fixture, &execution_request_ref);
    mutate_task_result_candidate(&fixture, &execution_request_ref, |result| {
        result["status"] = json!("completed_with_notes");
        result["notes"] = json!([]);
        result["executionContinuity"]["agentOwnedLongRunningWork"] = json!("unknown");
        result["executionContinuity"]["notes"] = json!([]);
    });

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["artifactKind"], "task_result_repair");
    let repair_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repair requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["repairContract.issueConflicts".to_string()],
    })
    .expect("read task result repair issue conflicts")
    .fields;
    assert!(repair_fields["repairContract.issueConflicts"]
        .value
        .as_array()
        .expect("issue conflicts")
        .iter()
        .any(|issue| {
            issue["code"] == "EXECUTION_CONTINUITY_REQUIRED"
                && issue["fieldPath"] == "executionContinuity.notes"
        }));
}

#[test]
fn failed_task_result_routes_to_review_after_four_failed_attempts() {
    let fixture = Fixture::new("failed-task-result-retry-budget");
    let mut request_ref = start_planned_task_execution(&fixture);

    for attempt in 1..=3 {
        write_failed_task_result_candidate_with_id(
            &fixture,
            &request_ref,
            &format!("result-failed-task-account-{attempt:03}"),
        );
        let result = call_submit(
            "loom.recordTaskResultFile",
            &request_ref,
            fixture.root_str(),
        );
        assert_eq!(
            result["state"], "auto_runnable",
            "attempt {attempt}: {result:#}"
        );
        assert_eq!(
            result["next"]["executionKind"], "delivery_execution_repair",
            "attempt {attempt}: {result:#}"
        );
        assert_eq!(
            result["next"]["repairContext"]["attemptCount"], attempt,
            "attempt {attempt}: {result:#}"
        );
        request_ref = result["next"]["requestRef"]
            .as_str()
            .expect("next repair requestRef")
            .to_string();
    }

    write_failed_task_result_candidate_with_id(
        &fixture,
        &request_ref,
        "result-failed-task-account-004",
    );
    let result = call_submit(
        "loom.recordTaskResultFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"], "review_result",
        "{result:#}"
    );
    assert_eq!(result["next"]["submitTool"], "loom.reviewAcceptFile");
}

#[test]
fn blocked_task_result_routes_to_taskplan_repair() {
    let fixture = Fixture::new("blocked-task-result-taskplan");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_blocked_task_result_candidate(
        &fixture,
        &execution_request_ref,
        "TASKPLAN_INVALID",
        "taskplan_repair",
    );

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(resumed["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "outputContract.outlineResultTemplate".to_string(),
            "outputContract.groupResultTemplate".to_string(),
        ],
    })
    .expect("read taskplan repair templates")
    .fields;
    assert!(repair_fields["outputContract.outlineResultTemplate"].value["groups"][0].is_object());
    assert!(repair_fields["outputContract.groupResultTemplate"].value["group"].is_object());
    assert!(repair_fields["outputContract.groupResultTemplate"].value["tasks"][0].is_object());
    assert!(
        repair_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptResponsibilities"][0]
            .is_object()
    );
    assert!(
        repair_fields["outputContract.groupResultTemplate"].value["tasks"][0]
            ["conceptVerificationIntents"][0]
            .is_object()
    );
}

#[test]
fn blocked_task_result_routes_to_architecture_repair() {
    let fixture = Fixture::new("blocked-task-result-architecture");
    let execution_request_ref = start_planned_task_execution(&fixture);
    write_blocked_task_result_candidate(
        &fixture,
        &execution_request_ref,
        "DESIGN_INSUFFICIENT",
        "architecture_artifact_repair",
    );

    let result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(
        result["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let resumed = continue_delivery(fixture.root_str());
    assert_eq!(resumed["state"], "auto_runnable", "{resumed:#}");
    assert_eq!(
        resumed["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(resumed["next"]["requestRef"], json!(repair_request_ref));
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(
        repair_root.get("sectionOutputs").is_none(),
        "architecture repair request root must not expose all section contracts"
    );
    assert!(repair_root["currentSectionContract"]["resultTemplate"]["content"].is_object());
}

#[test]
fn review_accept_approved_marks_delivery_done() {
    let fixture = Fixture::new("review-approved");
    let review_request_ref = complete_task_execution_to_review(&fixture);

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "done", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    assert_eq!(
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "reviewResult"),
        format!(".loom/deliveries/{delivery_id}/reviews/phase-1/results/review-phase-1.json")
    );
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read delivery index"))
            .expect("parse delivery index");
    assert_eq!(index["status"], "completed");
    let status: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/status.json")).expect("read status"),
    )
    .expect("parse status");
    assert_eq!(status["activeDeliveryId"], Value::Null);
    assert_eq!(status["lastCompletedDeliveryId"], delivery_id);
    assert_eq!(status["deliveries"][0]["status"], "completed");
    let continued = continue_delivery(fixture.root_str());
    assert_eq!(continued["state"], "done", "{continued:#}");
}

#[test]
fn review_accept_rejects_continue_without_next_phase_preview() {
    let fixture = Fixture::new("review-continue-without-preview");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    append_done_phase(&fixture, &delivery_id, "phase-2");

    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "approved",
        "continue_to_next_phase",
        vec![],
    );
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "nextAction.type"
    }));
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-1".to_string()
    );
}

#[test]
fn review_request_refreshes_when_task_result_repair_changes_snapshot() {
    let fixture = Fixture::new("review-refreshes-after-task-result-repair");
    let execution_request_ref =
        start_frontend_quality_task_execution_without_architecture_quality(&fixture);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution result template")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut stale_result = fields["outputContract.resultTemplate"].value.clone();
    stale_result["taskResultId"] = json!("result-before-frontend-repair");
    stale_result["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut stale_result);
    stale_result["frontendQualitySelfCheck"]["status"] = json!("partial");
    stale_result["frontendQualitySelfCheck"]["knownGaps"] =
        json!(["UI density needed one more repair pass."]);
    write_json_atomic(&fixture.root.join(result_file), &stale_result)
        .expect("write stale task result");

    let first_review = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(first_review["state"], "auto_runnable", "{first_review:#}");
    assert_eq!(first_review["next"]["artifactKind"], "review_result");
    let first_review_ref = first_review["next"]["requestRef"]
        .as_str()
        .expect("first review request ref")
        .to_string();
    let first_review_packets = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: first_review_ref.clone(),
        group_id: "review_packets".to_string(),
    })
    .expect("read first review packet");
    assert_eq!(
        first_review_packets.fields["reviewPacket.taskResultSummaries"].value[0]["taskResultId"],
        json!("result-before-frontend-repair")
    );

    let mut repaired_result = fields["outputContract.resultTemplate"].value.clone();
    repaired_result["taskResultId"] = json!("result-before-frontend-repair");
    repaired_result["changedFiles"] = json!(["src/App.tsx", "src/styles/global.css"]);
    complete_frontend_quality_token_evidence_for_test(&mut repaired_result);
    write_json_atomic(&fixture.root.join(result_file), &repaired_result)
        .expect("write repaired task result");

    let refreshed_review = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        refreshed_review["state"], "auto_runnable",
        "{refreshed_review:#}"
    );
    assert_eq!(refreshed_review["next"]["artifactKind"], "review_result");
    let refreshed_review_ref = refreshed_review["next"]["requestRef"]
        .as_str()
        .expect("refreshed review request ref")
        .to_string();
    assert_ne!(refreshed_review_ref, first_review_ref);
    let refreshed_review_packets = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: refreshed_review_ref,
        group_id: "review_packets".to_string(),
    })
    .expect("read refreshed review packet");
    assert_eq!(
        refreshed_review_packets.fields["reviewPacket.taskResultSummaries"].value[0]
            ["taskResultId"],
        json!("result-before-frontend-repair")
    );
    assert_eq!(
        refreshed_review_packets.fields["reviewPacket.taskResultSummaries"].value[0]
            ["frontendQualitySelfCheck"]["knownGapCount"],
        json!(0)
    );
}

#[test]
fn review_accept_approved_materializes_next_phase_from_preview() {
    let fixture = Fixture::new("review-auto-next-phase");
    let review_request_ref = complete_task_execution_to_review_with_candidate(
        &fixture,
        candidate_with_next_phase_preview(),
    );
    let template_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read review template")
    .fields;
    assert_eq!(
        template_fields["outputContract.resultTemplate"].value["nextAction"]["type"],
        "continue_to_next_phase"
    );
    assert_eq!(
        template_fields["outputContract.resultTemplate"].value["nextAction"]["targetPhaseId"],
        "phase-2"
    );

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"], "repository_context_candidate",
        "{result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-2".to_string()
    );
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    assert_eq!(index["status"], "planning");
    let phase_2 = index["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|phase| phase["phaseId"] == "phase-2")
        .expect("phase-2");
    assert_eq!(
        index["phases"]
            .as_array()
            .expect("phases")
            .iter()
            .filter(|phase| phase["phaseId"] == "phase-2")
            .count(),
        1,
        "existing phase-2 must be reused, not duplicated"
    );
    assert_eq!(phase_2["nextAction"]["kind"], "repository_context_request");
    assert!(phase_2["latestRefs"]["brainstormRequestRef"].is_null());
    assert!(phase_2["latestRefs"]["technicalBaseline"].is_string());

    let repository_context_request_ref = result["next"]["requestRef"]
        .as_str()
        .expect("repository context request ref");
    let phase_2_scan_contract = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_scan_contract".to_string(),
    })
    .expect("read phase-2 repository context scan contract");
    assert_eq!(
        phase_2_scan_contract.fields["repositoryMode"].value,
        json!("existing_project")
    );
    assert_eq!(
        phase_2_scan_contract.fields["phaseDevelopmentMode"].value,
        json!("incremental_delivery")
    );
    let completed_summaries = phase_2_scan_contract.fields["scanPurpose.completedPhaseSummaries"]
        .value
        .as_array()
        .expect("completed phase summaries");
    assert_eq!(completed_summaries.len(), 1);
    assert_eq!(completed_summaries[0]["phaseId"], json!("phase-1"));
    assert_eq!(completed_summaries[0]["status"], json!("completed"));
    assert!(
        completed_summaries[0]["title"].is_string(),
        "completed phase summary should include compact title from BrainstormContract: {completed_summaries:#?}"
    );
    let phase_2_generation_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repository_context_request_ref.to_string(),
        group_id: "repository_context_generation_rules".to_string(),
    })
    .expect("read phase-2 repository context generation rules");
    assert!(
        phase_2_generation_rules.fields["generationRules"]
            .value
            .to_string()
            .contains("after those delivered phases"),
        "incremental repository context must tell the agent to scan after completed phases"
    );
    let brainstorm_contract_ref = phase_2["latestRefs"]["brainstormContract"]
        .as_str()
        .expect("brainstorm contract ref");
    let technical_baseline_ref =
        format!(".loom/deliveries/{delivery_id}/contracts/technical-baseline.json");
    write_candidate_target(
        &fixture,
        repository_context_request_ref,
        &repository_context_candidate_json(
            repository_context_request_ref,
            brainstorm_contract_ref,
            &technical_baseline_ref,
        ),
    );

    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "user_gate",
        "{repository_result:#}"
    );
    assert_eq!(
        repository_result["gate"]["gateId"],
        "phase_brainstorm_required"
    );
    let phase_2_request_ref = repository_result["requestRef"]
        .as_str()
        .expect("phase-2 brainstorm request ref");
    let refreshed_index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let refreshed_index: Value = serde_json::from_str(
        &std::fs::read_to_string(refreshed_index_path).expect("read refreshed index"),
    )
    .expect("parse refreshed index");
    let refreshed_phase_2 = refreshed_index["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|phase| phase["phaseId"] == "phase-2")
        .expect("refreshed phase-2");
    assert_eq!(
        refreshed_phase_2["latestRefs"]["brainstormRequestRef"],
        phase_2_request_ref
    );
    let phase_2_request = read_request_root_value(fixture.root_str(), phase_2_request_ref);
    assert_eq!(phase_2_request["nextPhaseSeed"]["phaseId"], "phase-2");
    assert_eq!(
        phase_2_request["nextPhaseSeed"]["title"],
        "资金账户基础能力"
    );
    assert_eq!(
        phase_2_request["nextPhaseSeed"]["scopePreview"],
        json!(["资金账户开户", "密码管理", "存款与取款", "账户关联"])
    );
    let phase_2_read_groups = phase_2_request["requestReadPlan"]["groups"]
        .as_array()
        .expect("read groups");
    assert!(
        phase_2_read_groups
            .iter()
            .any(|group| group["groupId"] == "knowledge_context_plan"),
        "phase continuation Brainstorm request must keep request-scoped knowledge reads"
    );
    let next_phase_seed_group = phase_2_read_groups
        .iter()
        .find(|group| group["groupId"] == "next_phase_seed")
        .expect("next phase seed group");
    assert_eq!(
        read_group_fields_from_json(next_phase_seed_group)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        [
            "nextPhaseSeed.fromPhaseId",
            "nextPhaseSeed.phaseId",
            "nextPhaseSeed.title",
            "nextPhaseSeed.goal",
            "nextPhaseSeed.scopePreview",
            "nextPhaseSeed.reason",
            "nextPhaseSeed.usageRule"
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
    );
    let phase_continuation_group = phase_2_read_groups
        .iter()
        .find(|group| group["groupId"] == "phase_continuation_context")
        .expect("phase continuation context group");
    let phase_continuation_fields = read_group_fields_from_json(phase_continuation_group);
    for broad_field in [
        "phaseContinuationContext.activePhase",
        "deliveryContext.scope.deferred",
        "latestRepositoryContext.existingCapabilities",
        "latestRepositoryContext.relevantSurfaces",
        "confirmedRequirementDecisionsIndex.decisions",
    ] {
        assert!(
            !phase_continuation_fields.contains(&broad_field.to_string()),
            "broad read field leaked into phase continuation group: {broad_field}"
        );
    }
    for stale_or_duplicate_prefix in [
        "deliveryContext.phasePlan.current.",
        "deliveryContext.phasePlan.nextPhasePreview.",
        "latestConfirmedRequirementDecision.phasePlan.current.",
        "latestConfirmedRequirementDecision.phasePlan.nextPhasePreview.",
    ] {
        assert!(
            !phase_continuation_fields.iter().any(|field| field
                .starts_with(stale_or_duplicate_prefix)),
            "stale or duplicate phase read leaked into phase continuation group: {stale_or_duplicate_prefix}"
        );
    }
    for required_field in [
        "phaseContinuationContext.activePhase.phaseId",
        "phaseContinuationContext.activePhase.title",
        "phaseContinuationContext.activePhase.goal",
        "phaseContinuationContext.repository.repoSummary",
        "phaseContinuationContext.repository.capabilitySummaries",
        "phaseContinuationContext.repository.surfaceSummaries",
    ] {
        assert!(
            phase_continuation_fields
                .iter()
                .any(|field| field == required_field),
            "missing field-level phase continuation read: {required_field}"
        );
    }

    let phase_2_candidate_request_ref =
        confirm_phase2_brainstorm_to_candidate_write(&fixture, phase_2_request_ref);
    let phase_2_candidate_request =
        read_request_root_value(fixture.root_str(), &phase_2_candidate_request_ref);
    assert_eq!(
        phase_2_candidate_request["postSubmit"]["nextAction"]["kind"],
        "planning_contract_create"
    );
    write_candidate_target(
        &fixture,
        &phase_2_candidate_request_ref,
        &phase2_candidate_json(),
    );
    let phase_2_brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &phase_2_candidate_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        phase_2_brainstorm_result["state"], "auto_runnable",
        "{phase_2_brainstorm_result:#}"
    );
    assert_eq!(
        phase_2_brainstorm_result["next"]["artifactKind"],
        "architecture_section_candidate"
    );
    let phase_2_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let phase_2_contract: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(&phase_2_contract_ref))
            .expect("read phase-2 brainstorm contract"),
    )
    .expect("parse phase-2 brainstorm contract");
    assert_eq!(
        phase_2_contract["handoff"]["nextNode"],
        "planning_generation_contract"
    );
}

#[test]
fn review_accept_approved_activates_existing_unstarted_next_phase_from_preview() {
    let fixture = Fixture::new("review-existing-next-phase");
    let review_request_ref = complete_task_execution_to_review_with_candidate(
        &fixture,
        candidate_with_next_phase_preview(),
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    append_unstarted_phase(&fixture, &delivery_id, "phase-2");

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(
        result["next"]["artifactKind"], "repository_context_candidate",
        "{result:#}"
    );
    assert_eq!(
        active_phase_id(fixture.root_str(), &delivery_id),
        "phase-2".to_string()
    );
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    let phase_2 = index["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|phase| phase["phaseId"] == "phase-2")
        .expect("phase-2");
    assert_eq!(phase_2["nextAction"]["kind"], "repository_context_request");
    assert!(phase_2["latestRefs"]["brainstormContract"].is_string());
    assert!(phase_2["latestRefs"]["requirementContext"].is_string());
    assert!(phase_2["latestRefs"]["technicalBaseline"].is_string());
}

#[test]
fn review_environment_blocker_cannot_route_execution_repair() {
    let fixture = Fixture::new("review-env-blocker");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-env",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "review_limitation",
            "failureClass": "environment_blocker",
            "category": "environment_or_dependency",
            "summary": "Local browser dependency is unavailable.",
            "evidence": "The verification environment is unavailable.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "findings[].failureClass"
    }));
}

#[test]
fn review_accept_allows_normalized_changed_file_refs() {
    let fixture = Fixture::new("review-changed-file-ref-normalization");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-file-ref",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "The changed file evidence supports this finding.",
            "evidence": "The changed file was read through a normalized changed_file ref.",
            "readRefs": [{"type": "changed_file", "ref": "./src/main.tsx", "reason": "Read the changed file."}],
            "evidenceRefs": [{"type": "changed_file", "ref": "file:src/main.tsx", "reason": "Changed file evidence."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "yes",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
}

#[test]
fn review_accept_rejects_pending_action_refs_with_wrong_route() {
    let fixture = Fixture::new("review-pending-action-route-mismatch");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "blocked",
        "architecture_artifact_repair",
        vec![json!({
            "findingId": "finding-arch",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "contract",
            "failureClass": "contract_gap",
            "category": "architecture_design_gap",
            "summary": "Architecture contract is missing required detail.",
            "evidence": "The review packet shows a contract gap.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "architecture_artifact_repair"
        })],
    );
    mutate_review_result_candidate(&fixture, &review_request_ref, |candidate| {
        candidate["pendingActions"] = json!([{
            "type": "taskplan_repair",
            "findingRefs": ["finding-arch"],
            "reason": "Wrong route for this finding."
        }]);
    });

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "pendingActions[].findingRefs"
    }));
}

#[test]
fn review_accept_drops_untyped_pending_action_drafts_before_schema_parse() {
    let fixture = Fixture::new("review-drops-untyped-pending-action");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    mutate_review_result_candidate(&fixture, &review_request_ref, |candidate| {
        candidate["pendingActions"] = json!([{
            "findingRefs": [],
            "reason": "Draft action without a route type should not cause a schema-level bounce."
        }]);
    });

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "done", "{result:#}");
}

#[test]
fn review_accept_rejects_warning_only_repair_route() {
    let fixture = Fixture::new("review-warning-only-repair");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "approved_with_notes",
        "execution_repair",
        vec![json!({
            "findingId": "finding-warning",
            "severity": "minor",
            "severityClass": "warning",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "Non-blocking warning.",
            "evidence": "This should not route repair.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "yes",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "nextAction.type"
    }));
}

#[test]
fn review_request_uses_git_diff_refs_without_inlining_diffs() {
    let fixture = Fixture::new("review-git-diff-refs");
    let execution_request_ref = start_planned_task_execution(&fixture);
    run_git(&fixture, &["init"]);
    run_git(&fixture, &["config", "user.email", "loom@example.test"]);
    run_git(&fixture, &["config", "user.name", "Loom Test"]);
    run_git(&fixture, &["add", "."]);
    run_git(&fixture, &["commit", "-m", "test: baseline"]);
    std::fs::write(
        fixture.root.join("src/main.tsx"),
        "export const app = 'changed';\n",
    )
    .expect("modify tracked source");
    std::fs::write(
        fixture.root.join("src/new-widget.ts"),
        "export const newWidget = true;\n",
    )
    .expect("write declared untracked source");
    std::fs::write(
        fixture.root.join("src/not-declared.ts"),
        "export const notDeclared = true;\n",
    )
    .expect("write unrelated untracked source");
    write_task_result_candidate(&fixture, &execution_request_ref);
    mutate_task_result_candidate(&fixture, &execution_request_ref, |result| {
        result["changedFiles"] = json!(["src/main.tsx", "src/new-widget.ts"]);
    });
    let task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    let review_request_ref = if task_result["next"]["artifactKind"] == json!("review_result") {
        task_result["next"]["requestRef"]
            .as_str()
            .expect("review requestRef")
            .to_string()
    } else {
        assert_eq!(
            task_result["next"]["kind"], "execute_task",
            "{task_result:#}"
        );
        let next_execution_ref = task_result["next"]["requestRef"]
            .as_str()
            .expect("next execution requestRef")
            .to_string();
        write_task_result_candidate(&fixture, &next_execution_ref);
        let review_result = call_submit(
            "loom.recordTaskResultFile",
            &next_execution_ref,
            fixture.root_str(),
        );
        assert_eq!(review_result["state"], "auto_runnable", "{review_result:#}");
        assert_eq!(
            review_result["next"]["artifactKind"], "review_result",
            "{review_result:#}"
        );
        review_result["next"]["requestRef"]
            .as_str()
            .expect("review requestRef")
            .to_string()
    };

    let change_context = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        group_id: "change_context".to_string(),
    })
    .expect("read change context");
    assert_eq!(
        change_context.fields["changeContext.mode"].value,
        json!("git_diff_ref")
    );
    let changed_files = change_context.fields["changeContext.changedFiles"]
        .value
        .as_array()
        .expect("changed files");
    let changed_files_json = serde_json::to_string(changed_files).expect("serialize changed files");
    assert!(changed_files_json.contains("src/main.tsx"));
    assert!(changed_files_json.contains("src/new-widget.ts"));
    assert!(!changed_files_json.contains("not-declared"));
    let tracked_file = changed_files
        .iter()
        .find(|file| file["path"] == json!("src/main.tsx"))
        .expect("tracked changed file");
    let tracked_diff_ref = tracked_file["diffRef"].as_str().expect("tracked diffRef");
    assert!(fixture.root.join(tracked_diff_ref).exists());
    let tracked_diff =
        std::fs::read_to_string(fixture.root.join(tracked_diff_ref)).expect("read tracked diff");
    assert!(tracked_diff.contains("export const app = 'changed'"));
    let new_file = changed_files
        .iter()
        .find(|file| file["path"] == json!("src/new-widget.ts"))
        .expect("declared new file");
    assert_eq!(new_file["changeType"], json!("declared_changed"));
    assert!(new_file["insertions"].as_u64().unwrap_or(0) >= 1);
    let new_diff_ref = new_file["diffRef"].as_str().expect("new file diffRef");
    let new_diff_path = fixture.root.join(new_diff_ref);
    let new_diff = std::fs::read_to_string(&new_diff_path).expect("read new file diff");
    assert!(new_diff.contains("new-widget.ts"));
    assert!(new_diff.contains("+export const newWidget = true;"));
    assert!(!new_diff.contains("notDeclared"));
    let full_diff_path = new_diff_path
        .parent()
        .expect("diff parent")
        .join("full.diff");
    let full_diff = std::fs::read_to_string(full_diff_path).expect("read full diff");
    assert!(full_diff.contains("export const app = 'changed'"));
    assert!(full_diff.contains("+export const newWidget = true;"));
    assert!(!full_diff.contains("notDeclared"));
    let request_root = read_request_root_value(fixture.root_str(), &review_request_ref);
    assert!(
        request_root.get("changeContext").is_none(),
        "diff refs must stay in private request storage"
    );
    assert!(
        request_root.get("changeSet").is_none(),
        "changeSet must stay in private request storage"
    );
    assert!(!serde_json::to_string(&request_root)
        .expect("serialize request root")
        .contains("export const app = 'changed'"));
    assert!(!serde_json::to_string(&request_root)
        .expect("serialize request root")
        .contains("newWidget"));
}

#[test]
fn review_flags_missing_workflow_closure_assignment_as_taskplan_repair() {
    let fixture = Fixture::new("review-workflow-closure-missing-assignment");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);

    let mut current_execution_request_ref = execution_request_ref.clone();
    let mut task_result = Value::Null;
    for _ in 0..4 {
        let execution_fields = state::read_request_fields(ReadRequestFieldsInput {
            project_root: fixture.root_str().to_string(),
            request_ref: current_execution_request_ref.clone(),
            fields: vec![
                "outputContract.resultFile".to_string(),
                "outputContract.resultTemplate".to_string(),
            ],
        })
        .expect("read execution result template")
        .fields;
        let result_file = execution_fields["outputContract.resultFile"]
            .value
            .as_str()
            .expect("result file");
        let mut task_result_candidate = execution_fields["outputContract.resultTemplate"]
            .value
            .clone();
        task_result_candidate["changedFiles"] = json!(["src/App.tsx"]);
        complete_frontend_quality_token_evidence_for_test(&mut task_result_candidate);
        write_json_atomic(&fixture.root.join(result_file), &task_result_candidate)
            .expect("write task result from template");
        let result = call_submit(
            "loom.recordTaskResultFile",
            &current_execution_request_ref,
            fixture.root_str(),
        );
        assert_eq!(result["state"], "auto_runnable", "{result:#}");
        if result["next"]["artifactKind"] == json!("review_result") {
            task_result = result;
            break;
        }
        assert_eq!(result["next"]["kind"], "execute_task", "{result:#}");
        current_execution_request_ref = result["next"]["requestRef"]
            .as_str()
            .expect("next execution requestRef")
            .to_string();
    }
    assert_eq!(task_result["next"]["artifactKind"], "review_result");

    let architecture_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact");
    let architecture_path = fixture.root.join(&architecture_ref);
    let mut architecture: Value = serde_json::from_str(
        &std::fs::read_to_string(&architecture_path).expect("read architecture artifact"),
    )
    .expect("parse architecture artifact");
    architecture["interfaces"]
        .as_array_mut()
        .expect("interfaces")
        .push(json!({
            "interfaceId": "api.account.reissue",
            "name": "Submit account reissue",
            "type": "http_api",
            "method": "POST",
            "path": "/api/accounts/reissue",
            "requestSchema": [{"field": "accountId", "type": "string", "required": true}],
            "responseSchema": [{"field": "status", "type": "string", "required": true}],
            "errorSchema": [{"field": "message", "type": "string", "required": true}]
        }));
    architecture["userFlows"]
        .as_array_mut()
        .expect("user flows")
        .push(json!({
            "flowId": "flow.account-reissue",
            "name": "Account reissue workflow",
            "kind": "user_interaction",
            "moduleRefs": ["module.account-service"],
            "acceptanceRefs": ["acc_1"],
            "interfaceRefs": ["api.account.reissue"],
            "entry": {},
            "steps": [{
                "stepId": "step.submit-reissue",
                "interfaceRefs": ["api.account.reissue"],
                "stateMachineRefs": []
            }]
        }));
    architecture["frontendExperience"]["surfaces"]
        .as_array_mut()
        .expect("frontend surfaces")
        .push(json!({
            "surfaceId": "surface_account_reissue",
            "name": "Account reissue",
            "workflowRefs": ["flow.account-reissue"]
        }));
    architecture["frontendExperience"]["operationPaths"]
        .as_array_mut()
        .expect("frontend operation paths")
        .push(json!({
            "pathId": "path_account_reissue",
            "surfaceRef": "surface_account_reissue",
            "workflowRef": "flow.account-reissue",
            "dataViewRefs": [],
            "actionRefs": []
        }));
    write_json_atomic(&architecture_path, &architecture)
        .expect("write architecture artifact with missing workflow closure assignment");

    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read delivery index"))
            .expect("parse delivery index");
    let latest_refs = index["phases"][0]["latestRefs"]
        .as_object_mut()
        .expect("latest refs");
    latest_refs.remove("reviewRequestId");
    latest_refs.remove("reviewRequestRef");
    write_json_atomic(&index_path, &index).expect("clear existing review request refs");
    let review_action = RouteAction {
        kind: RouteActionKind::Review,
        source: "test_rematerialize_review".to_string(),
        reason: "rematerialize review after architecture artifact change".to_string(),
        prompt: None,
        accepted_responses: vec![],
        request_ref: None,
        details: None,
        target_phase_id: None,
    };
    let rematerialized = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &review_action,
    );
    let rematerialized: Value =
        serde_json::to_value(rematerialized).expect("serialize rematerialized review result");
    assert_eq!(
        rematerialized["state"], "auto_runnable",
        "{rematerialized:#}"
    );
    assert_eq!(rematerialized["next"]["artifactKind"], "review_result");
    let review_request_ref = rematerialized["next"]["requestRef"]
        .as_str()
        .expect("review requestRef")
        .to_string();
    let review_matrices = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        group_id: "review_matrices".to_string(),
    })
    .expect("read review matrices");
    let review_signals = review_matrices.fields["outputContract.reviewSignals.items"]
        .value
        .as_array()
        .expect("review signals");
    assert!(
        review_signals.iter().any(|signal| {
            signal["kind"] == json!("frontend_workflow_closure")
                && signal["missingTaskAssignment"] == json!(true)
                && signal["recommendedNextAction"] == json!("taskplan_repair")
                && signal["taskRefs"] == json!([])
        }),
        "review signals must include compact missing task assignment fact: {review_signals:#?}"
    );

    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-wrong-route",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "contract",
            "failureClass": "contract_gap",
            "category": "task_verification_mapping_issue",
            "summary": "Workflow closure was not assigned to a task.",
            "evidence": "Review signals show missingTaskAssignment=true.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "execution_repair"
        })],
    );
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT"
            && issue["fieldPath"] == "nextAction.type"
    }));
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT" && issue["fieldPath"] == "findings"
    }));
}

#[test]
fn review_flags_frontend_quality_self_check_gaps() {
    let fixture = Fixture::new("review-frontend-quality-gap");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    let execution_request_ref = execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string();
    let execution_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution result template")
    .fields;
    let result_file = execution_fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut task_result_candidate = execution_fields["outputContract.resultTemplate"]
        .value
        .clone();
    task_result_candidate["changedFiles"] = json!(["src/App.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut task_result_candidate);
    task_result_candidate["frontendQualitySelfCheck"]["status"] = json!("needs_repair");
    task_result_candidate["frontendQualitySelfCheck"]["knownGaps"] =
        json!(["The UI quality pass found a remaining density/state polish gap."]);
    write_json_atomic(&fixture.root.join(result_file), &task_result_candidate)
        .expect("write task result with frontend quality gap");
    let task_result = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
    assert_eq!(task_result["next"]["artifactKind"], "review_result");
    let review_request_ref = task_result["next"]["requestRef"]
        .as_str()
        .expect("review requestRef")
        .to_string();

    let review_packets = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        group_id: "review_packets".to_string(),
    })
    .expect("read review packets");
    assert_eq!(
        review_packets.fields["reviewPacket.taskResultSummaries"].value[0]
            ["frontendQualitySelfCheckPresent"],
        json!(true)
    );
    assert_eq!(
        review_packets.fields["reviewPacket.taskResultSummaries"].value[0]
            ["frontendQualitySelfCheck"]["knownGapCount"],
        json!(1)
    );
    let review_matrices = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: review_request_ref.clone(),
        group_id: "review_matrices".to_string(),
    })
    .expect("read review matrices");
    let quality_matrix = review_matrices.fields["reviewMatrixSummary.frontendQuality"]
        .value
        .as_array()
        .expect("frontend quality matrix summary");
    assert_eq!(quality_matrix[0]["qualitySatisfied"], json!(false));
    assert_eq!(quality_matrix[0]["knownGapCount"], json!(1));
    assert_eq!(quality_matrix[0]["missingQualityGateCount"], json!(0));
    assert_eq!(
        quality_matrix[0]["mustQualityGateUnsatisfiedCount"],
        json!(0)
    );
    assert_eq!(
        quality_matrix[0]["recommendedNextAction"],
        json!("execution_repair")
    );
    let review_signals = review_matrices.fields["outputContract.reviewSignals.items"]
        .value
        .as_array()
        .expect("review signals");
    assert!(
        review_signals.iter().any(|signal| {
            signal["kind"] == json!("frontend_ui_quality")
                && signal["uiQualitySatisfied"] == json!(false)
                && signal["recommendedNextAction"] == json!("execution_repair")
        }),
        "review signals must include compact frontend quality failure fact: {review_signals:#?}"
    );

    write_review_result_candidate(&fixture, &review_request_ref, "approved", "done", vec![]);
    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "REVIEW_RESULT_STATUS_INCONSISTENT" && issue["fieldPath"] == "decision"
    }));
}

#[test]
fn repeated_invalid_review_result_stays_repairable() {
    let fixture = Fixture::new("review-invalid-repairable");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_candidate_target(
        &fixture,
        &review_request_ref,
        &json!({ "schemaVersion": "1.0" }),
    );

    let first = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    let second = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    let third = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(first["state"], "repairable_error", "{first:#}");
    assert_eq!(second["state"], "repairable_error", "{second:#}");
    assert_eq!(third["state"], "repairable_error", "{third:#}");
    assert_eq!(third["resubmitTool"], "loom.reviewAcceptFile");
    assert_eq!(third["fixScope"], "review_result_candidate_only");
}

#[test]
fn review_execution_repair_materializes_repair_task() {
    let fixture = Fixture::new("review-execution-repair");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-product",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "code",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "The implemented flow misses a required behavior.",
            "evidence": "Task result evidence does not cover the required behavior.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "yes",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "review_result");
    assert_eq!(
        result["next"]["repairContext"]["reviewResultRef"],
        latest_ref_for_phase(
            fixture.root_str(),
            &request_delivery_id(fixture.root_str(), &review_request_ref),
            "reviewResult"
        )
    );
    assert_eq!(
        result["next"]["repairContext"]["findingRefs"],
        json!(["finding-product"])
    );
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let repair_root = read_request_root_value(fixture.root_str(), repair_request_ref);
    assert!(repair_root["repairContext"].get("attemptCount").is_none());
    assert_eq!(
        repair_root["repairContext"]["findingRefs"],
        json!(["finding-product"])
    );
    let inspected_repair = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
    })
    .expect("inspect review repair request");
    let repair_core = inspected_repair
        .read_groups
        .iter()
        .find(|group| group.group_id == "repair_execution_core")
        .expect("repair execution core group");
    let repair_core_fields = repair_core.expanded_fields();
    assert!(!repair_core_fields.contains(&"repairContext.attemptCount".to_string()));
    assert!(repair_core_fields.contains(&"repairContext.findingRefs".to_string()));
}

#[test]
fn review_execution_repair_targets_review_finding_task_ref() {
    let fixture = Fixture::new("review-execution-repair-target-task-ref");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    let target_task_id = "task-runtime-delivery-closure";
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "changes_requested",
        "execution_repair",
        vec![json!({
            "findingId": "finding-runtime-closure",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "verification",
            "failureClass": "product_defect",
            "category": "functional_correctness",
            "summary": "Runtime closure verification does not prove the declared delivery contract.",
            "evidence": "The runtime closure task result omitted the required runtime signal.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRefs": [target_task_id],
            "taskRelevance": "direct",
            "scopeRelation": "within_task_changed_files",
            "introducedByCurrentTask": "yes",
            "recommendedNextAction": "execution_repair"
        })],
    );

    let result = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["taskId"], target_task_id);
    assert_eq!(
        result["next"]["repairContext"]["sourceTaskId"],
        target_task_id
    );
    let repair_request_ref = result["next"]["requestRef"].as_str().expect("requestRef");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec![
            "source.taskId".to_string(),
            "repairContext.sourceTaskId".to_string(),
        ],
    })
    .expect("read repair source task fields")
    .fields;
    assert_eq!(repair_fields["source.taskId"].value, json!(target_task_id));
    assert_eq!(
        repair_fields["repairContext.sourceTaskId"].value,
        json!(target_task_id)
    );

    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read delivery index"))
            .expect("parse delivery index");
    let action: RouteAction = serde_json::from_value(
        index["phases"]
            .as_array()
            .expect("phases")
            .iter()
            .find(|phase| phase["phaseId"] == "phase-1")
            .expect("phase-1")["nextAction"]
            .clone(),
    )
    .expect("parse current route action");
    assert_eq!(action.source, "delivery_execution_repair");
    let repeated = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &action,
    );
    let repeated = serde_json::to_value(repeated).expect("serialize repeated result");
    assert_eq!(repeated["state"], "auto_runnable", "{repeated:#}");
    assert_eq!(repeated["next"]["taskId"], target_task_id);
    assert_eq!(repeated["next"]["requestRef"], repair_request_ref);
}

#[test]
fn manual_review_resolution_routes_to_execution_repair() {
    let fixture = Fixture::new("manual-review-resolution");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    write_review_result_candidate(
        &fixture,
        &review_request_ref,
        "blocked",
        "manual_review",
        vec![json!({
            "findingId": "finding-manual",
            "severity": "major",
            "severityClass": "blocking",
            "evidenceKind": "manual",
            "failureClass": "contract_gap",
            "category": "review_limitation",
            "summary": "Manual decision is required.",
            "evidence": "The review requires user judgment.",
            "readRefs": [{"type": "review_packet", "ref": "reviewPacket", "reason": "Review packet was inspected."}],
            "taskRelevance": "current_task",
            "scopeRelation": "in_scope",
            "introducedByCurrentTask": "no",
            "recommendedNextAction": "manual_review"
        })],
    );
    let gate = call_submit(
        "loom.reviewAcceptFile",
        &review_request_ref,
        fixture.root_str(),
    );
    assert_eq!(gate["state"], "user_gate", "{gate:#}");
    assert!(gate["gate"].get("readGroups").is_none());
    assert!(gate["gate"].get("writeTargets").is_none());
    assert!(gate["gate"].get("submitTool").is_none());
    let manual_request_ref = gate["requestRef"]
        .as_str()
        .expect("manual review requestRef")
        .to_string();
    let manual_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: manual_request_ref.clone(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read manual review result template")
    .fields;
    assert!(manual_fields["outputContract.resultTemplate"].value["userAnswer"].is_object());
    assert!(manual_fields["outputContract.resultTemplate"].value["nextAction"].is_object());
    write_manual_review_resolution_candidate(&fixture, &manual_request_ref);

    let result = call_submit(
        "loom.reviewResolveFile",
        &manual_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "delivery_execution_repair");
    assert_eq!(result["next"]["repairOrigin"], "manual_review_resolution");
    assert!(result["next"]["repairContext"]["manualReviewResolutionRef"]
        .as_str()
        .unwrap()
        .contains("/manual-resolutions/"));
    assert_eq!(
        result["next"]["repairContext"]["userChangeSummary"],
        "修复当前实现问题。"
    );
    let execution_repair_ref = result["next"]["requestRef"]
        .as_str()
        .expect("execution repair requestRef");
    let repair_rules = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_repair_ref.to_string(),
        group_id: "repair_execution_core".to_string(),
    })
    .expect("read execution repair rules");
    let repair_rules_text =
        serde_json::to_string(&repair_rules.fields).expect("serialize repair rules");
    assert!(repair_rules_text.contains("Use repairContext as the failure boundary"));
    assert!(repair_rules_text.contains("rerun that signal"));
}

#[test]
fn taskplan_submit_repairs_runtime_requirement_shape_before_parse() {
    let fixture = Fixture::new("taskplan-runtime-requirement-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["runtimeDeliveryRequirement"] = json!({
        "appliesToThisTask": true,
        "reason": "This task touches runtime delivery.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": ["manual_command_output"],
        "evidenceExpectedInTaskResult": [],
        "forbiddenActions": []
    });
    write_json_atomic(&group_path, &group_value).expect("write invalid group file");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["targetFile"], json!(group_file));
    assert!(
        result["issues"].as_array().unwrap().iter().any(|issue| {
            issue["fieldPath"] == "tasks[0].runtimeDeliveryRequirement.requiredCodeLevelChecks[0]"
                && issue["code"] == "TASKPLAN_GROUP_SCHEMA_INVALID"
        }),
        "{result:#}"
    );
}

#[test]
fn taskplan_submit_requires_covered_requirement_detail_assignment() {
    let fixture = Fixture::new("taskplan-detail-assignment-required");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    for group_file in taskplan_group_files(&fixture, &taskplan_request_ref) {
        let group_path = fixture.root.join(&group_file);
        let mut group_value: Value =
            serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
                .expect("parse group file");
        for task in group_value["tasks"]
            .as_array_mut()
            .expect("group tasks")
            .iter_mut()
        {
            task["requirementDetailRefs"] = json!([]);
            for intent in task["verificationIntents"]
                .as_array_mut()
                .expect("verification intents")
                .iter_mut()
            {
                intent["requirementDetailRefs"] = json!([]);
            }
        }
        write_json_atomic(&group_path, &group_value).expect("write group without detail refs");
    }

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    let issues = result["issues"].as_array().expect("issues");
    assert!(issues.iter().any(|issue| {
        issue["code"] == "DETAIL_TASK_ASSIGNMENT_MISSING"
            && issue["fieldPath"] == "tasks[].requirementDetailRefs"
    }));
    assert!(issues.iter().any(|issue| {
        issue["code"] == "DETAIL_TASK_ASSIGNMENT_MISSING"
            && issue["fieldPath"] == "tasks[].verificationIntents[].requirementDetailRefs"
    }));
}

#[test]
fn taskplan_submit_normalizes_verification_detail_parent_refs() {
    let fixture = Fixture::new("taskplan-normalizes-verification-detail-parent");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    let detail_ref = group_value["tasks"][0]["verificationIntents"][0]["requirementDetailRefs"][0]
        .as_str()
        .expect("detail ref")
        .to_string();
    group_value["tasks"][0]["requirementDetailRefs"] = json!([]);
    write_json_atomic(&group_path, &group_value).expect("write group with parent detail gap");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    assert!(persisted["tasks"][0]["requirementDetailRefs"]
        .as_array()
        .expect("task detail refs")
        .iter()
        .any(|item| item == &json!(detail_ref)));
}

#[test]
fn taskplan_submit_normalizes_missing_verification_detail_refs() {
    let fixture = Fixture::new("taskplan-normalizes-verification-detail");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    let detail_ref = group_value["tasks"][0]["requirementDetailRefs"][0]
        .as_str()
        .expect("detail ref")
        .to_string();
    group_value["tasks"][0]["verificationIntents"][0]["requirementDetailRefs"] = json!([]);
    write_json_atomic(&group_path, &group_value).expect("write group with verification detail gap");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    assert!(
        persisted["tasks"][0]["verificationIntents"][0]["requirementDetailRefs"]
            .as_array()
            .expect("verification detail refs")
            .iter()
            .any(|item| item == &json!(detail_ref))
    );
}

#[test]
fn taskplan_submit_normalizes_missing_verification_detail_refs_with_multiple_matching_intents() {
    let fixture = Fixture::new("taskplan-normalizes-verification-detail-multiple-intents");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    let detail_ref = group_value["tasks"][0]["requirementDetailRefs"][0]
        .as_str()
        .expect("detail ref")
        .to_string();
    let mut second_intent = group_value["tasks"][0]["verificationIntents"][0].clone();
    second_intent["verificationId"] = json!("verify-account-002");
    second_intent["requirementDetailRefs"] = json!([]);
    group_value["tasks"][0]["verificationIntents"][0]["requirementDetailRefs"] = json!([]);
    group_value["tasks"][0]["verificationIntents"]
        .as_array_mut()
        .expect("verification intents")
        .push(second_intent);
    write_json_atomic(&group_path, &group_value)
        .expect("write group with ambiguous verification detail gap");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    assert!(
        persisted["tasks"][0]["verificationIntents"][0]["requirementDetailRefs"]
            .as_array()
            .expect("verification detail refs")
            .iter()
            .any(|item| item == &json!(detail_ref))
    );
}

#[test]
fn taskplan_submit_requires_frontend_task_when_frontend_required() {
    let fixture = Fixture::new("taskplan-frontend-task-required");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["taskKind"] = json!("feature_increment");
    group_value["tasks"][0]["implementationActions"] =
        json!(["create_or_update_interface", "add_or_update_tests"]);
    group_value["tasks"][0]
        .as_object_mut()
        .expect("task object")
        .remove("frontendExperienceRequirement");
    write_json_atomic(&group_path, &group_value).expect("write group without frontend task");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "FRONTEND_TASK_REQUIRED"
            && issue["fieldPath"] == "tasks[].frontendExperienceRequirement"
    }));
}

#[test]
fn taskplan_submit_normalizes_frontend_ui_quality_contract_on_ui_tasks() {
    let fixture = Fixture::new("taskplan-frontend-ui-quality-normalized");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["frontendExperienceRequirement"]
        .as_object_mut()
        .expect("frontend requirement object")
        .remove("uiQualityContract");
    write_json_atomic(&group_path, &group_value)
        .expect("write frontend requirement without ui quality");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    let expected =
        frontend_requirement_template_from_taskplan_request(&fixture, &taskplan_request_ref);
    assert_eq!(
        persisted["tasks"][0]["frontendExperienceRequirement"]["uiQualityContract"],
        expected["uiQualityContract"]
    );
}

#[test]
fn taskplan_submit_normalizes_interface_write_refs_when_not_opened() {
    let fixture = Fixture::new("taskplan-interface-refs-not-opened");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_without_interfaces_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["interfaces"] =
        json!(["api.not-opened"]);
    write_json_atomic(&group_path, &group_value).expect("write group with unavailable interface");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    let delivery_id = request_delivery_id(fixture.root_str(), &taskplan_request_ref);
    let taskplan_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlan");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(taskplan_ref)).unwrap())
            .expect("parse persisted taskplan");
    assert!(
        persisted["tasks"][0]["writeBoundary"]["artifactRefs"]["interfaces"]
            .as_array()
            .map(|items| items.is_empty())
            .unwrap_or(true)
    );
}

#[test]
fn taskplan_submit_requires_workflow_closure_assignment() {
    let fixture = Fixture::new("taskplan-workflow-closure-assignment");
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        &fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_json,
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates_for_workflow_closure(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["implementationActions"] = json!(["add_or_update_tests"]);
    write_json_atomic(&group_path, &group_value).expect("write group without workflow wiring");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "WORKFLOW_CLOSURE_NOT_ASSIGNED"
            && issue["fieldPath"] == "tasks[].frontendExperienceRequirement"
    }));
}

#[test]
fn taskplan_submit_requires_runtime_delivery_closure() {
    let fixture = Fixture::new("taskplan-runtime-closure-required");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec!["outputContract.outlineFile".to_string()],
    })
    .expect("read taskplan outline field")
    .fields;
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let outline_path = fixture.root.join(outline_file);
    let mut outline_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&outline_path).expect("read outline"))
            .expect("parse outline");
    outline_value["groups"]
        .as_array_mut()
        .expect("outline groups")
        .retain(|group| group["groupId"] != json!("group-runtime-delivery-closure"));
    write_json_atomic(&outline_path, &outline_value).expect("write outline without closure");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "RUNTIME_CLOSURE_TASK_REQUIRED"
            && issue["fieldPath"] == "tasks.runtimeDeliveryClosure"
    }));
}

#[test]
fn taskplan_submit_rejects_runtime_closure_check_mismatch() {
    let fixture = Fixture::new("taskplan-runtime-closure-check-mismatch");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: taskplan_request_ref.to_string(),
        fields: vec!["outputContract.groupFilePattern".to_string()],
    })
    .expect("read taskplan group pattern")
    .fields;
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group pattern");
    let closure_group_file = group_pattern.replace("{groupId}", "group-runtime-delivery-closure");
    let closure_group_path = fixture.root.join(&closure_group_file);
    let mut closure_group: Value = serde_json::from_str(
        &std::fs::read_to_string(&closure_group_path).expect("read closure group"),
    )
    .expect("parse closure group");
    closure_group["tasks"][0]["runtimeDeliveryRequirement"]["requiredCodeLevelChecks"][0]
        ["checkId"] = json!("wrong-runtime-check-id");
    write_json_atomic(&closure_group_path, &closure_group).expect("write bad closure group");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "RUNTIME_CLOSURE_CHECK_INVALID"
            && issue["fieldPath"] == "tasks[].runtimeDeliveryRequirement.requiredCodeLevelChecks"
    }));
}

#[test]
fn taskplan_submit_repairs_task_enum_shape_before_parse() {
    let fixture = Fixture::new("taskplan-enum-shape");
    let architecture_request_ref = start_existing_project_architecture_flow(&fixture);
    let taskplan_result = complete_architecture_sections(&fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();

    write_taskplan_grouped_candidates(&fixture, &taskplan_request_ref);
    let group_file = first_taskplan_group_file(&fixture, &taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["verificationIntents"][0]["acceptableEvidence"] =
        json!(["static_check", "shell_output"]);
    write_json_atomic(&group_path, &group_value).expect("write invalid group file");

    let result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "repairable_error", "{result:#}");
    assert_eq!(result["targetFile"], json!(group_file));
    assert!(
        result["issues"].as_array().unwrap().iter().any(|issue| {
            issue["fieldPath"] == "tasks[0].verificationIntents[0].acceptableEvidence[1]"
                && issue["code"] == "TASKPLAN_GROUP_SCHEMA_INVALID"
        }),
        "{result:#}"
    );
}

#[test]
fn taskplan_repair_submit_replaces_taskplan_and_starts_new_run() {
    let fixture = Fixture::new("repair-submit-taskplan");
    let review_request_ref = complete_task_execution_to_review(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    let old_run_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRun");
    let repair_result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &delivery_core::RouteAction {
            kind: delivery_core::RouteActionKind::TaskplanRepair,
            source: "test".to_string(),
            reason: "taskplan repair".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let repair_result = serde_json::to_value(repair_result).expect("repair result value");
    assert_eq!(repair_result["state"], "auto_runnable", "{repair_result:#}");
    assert_eq!(repair_result["next"]["artifactKind"], "taskplan_repair");
    assert_eq!(repair_result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_action_ref = repair_result["next"]["requestRef"]
        .as_str()
        .expect("repair action requestRef")
        .to_string();
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        fields: vec![
            "outputContract.runtimeDeliveryRequirementTemplate".to_string(),
            "outputContract.runtimeDeliveryClosureTaskTemplate".to_string(),
            "outputContract.engineeringQualityRequirementTemplate".to_string(),
            "generationRules.engineeringQualityRules".to_string(),
        ],
    })
    .expect("read taskplan repair runtime templates")
    .fields;
    assert!(
        repair_fields["outputContract.runtimeDeliveryRequirementTemplate"]
            .value
            .is_object()
    );
    assert!(
        repair_fields["outputContract.runtimeDeliveryClosureTaskTemplate"]
            .value
            .is_object()
    );
    assert_eq!(
        repair_fields["outputContract.engineeringQualityRequirementTemplate"].value["kind"],
        json!("persistence_mapping")
    );
    assert!(
        repair_fields["generationRules.engineeringQualityRules"].value["acceptNormalization"]
            .as_str()
            .is_some_and(|rule| rule.contains("do not duplicate full quality requirements"))
    );
    let repair_inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
    })
    .expect("inspect taskplan repair request");
    let taskplan_repair_core_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "taskplan_core_context".to_string(),
    })
    .expect("read taskplan repair core group");
    assert!(
        taskplan_repair_core_group
            .fields
            .iter()
            .filter(|(field, _)| field.starts_with("sourceRefs."))
            .all(|(_, field)| !field.value.is_null()),
        "taskplan repair must not expose null optional source refs: {:#?}",
        taskplan_repair_core_group.fields
    );
    assert!(
        taskplan_repair_core_group
            .fields
            .get("sourceRefs.repositoryContextRef")
            .is_some(),
        "taskplan repair must preserve repository context from the original request"
    );
    assert!(
        taskplan_repair_core_group
            .fields
            .get("repairContext.sourceRef")
            .is_none(),
        "taskplan repair must omit sourceRef when there is no review/manual source"
    );
    let taskplan_repair_core_fields = repair_inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == "taskplan_core_context")
        .expect("taskplan repair core group")
        .expanded_fields();
    assert!(taskplan_repair_core_fields.contains(&"sourceRefs.repositoryContextRef".to_string()));
    assert!(!taskplan_repair_core_fields.contains(&"repairContext.sourceRef".to_string()));
    assert!(repair_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "outputContract.runtimeDeliveryRequirementTemplate"));
    assert!(repair_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .any(|field| field == "outputContract.engineeringQualityRequirementTemplate"));
    assert!(!repair_inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "taskplan_optional_projection"));
    assert!(repair_inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .all(
            |field| field != "contextProjection.frontendExperienceProjection"
                && field != "contextProjection.runtimeDeliveryProjection"
        ));
    write_taskplan_grouped_candidates(&fixture, &repair_action_ref);

    let result = call_submit(
        "loom.repairSubmitFile",
        &repair_action_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "execute_task");
    assert_eq!(result["next"]["executionKind"], "planned_task");
    let new_run_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRun");
    assert_ne!(new_run_ref, old_run_ref);
}

#[test]
fn architecture_repair_submit_rebuilds_aac_and_recreates_taskplan_request() {
    let fixture = Fixture::new("repair-submit-architecture");
    let review_request_ref = complete_task_execution_to_review_with_candidate(
        &fixture,
        valid_candidate_with_frontend_json(),
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &review_request_ref);
    let old_taskplan_request_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRequestRef");
    let repair_result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &delivery_core::RouteAction {
            kind: delivery_core::RouteActionKind::ArchitectureArtifactRepair,
            source: "test".to_string(),
            reason: "architecture repair".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let repair_result = serde_json::to_value(repair_result).expect("repair result value");
    assert_eq!(repair_result["state"], "auto_runnable", "{repair_result:#}");
    assert_eq!(
        repair_result["next"]["artifactKind"],
        "architecture_artifact_repair"
    );
    assert_eq!(repair_result["next"]["submitTool"], "loom.repairSubmitFile");
    let repair_action_ref = repair_result["next"]["requestRef"]
        .as_str()
        .expect("repair action requestRef")
        .to_string();
    let architecture_core_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "architecture_core_context".to_string(),
    })
    .expect("read architecture repair core group");
    assert!(
        architecture_core_group
            .fields
            .iter()
            .filter(|(field, _)| field.starts_with("sourceRefs."))
            .all(|(_, field)| !field.value.is_null()),
        "architecture repair must not expose null optional source refs: {:#?}",
        architecture_core_group.fields
    );
    assert!(
        architecture_core_group
            .fields
            .get("repairContext.sourceRef")
            .is_none(),
        "architecture repair must omit sourceRef when there is no review/manual source"
    );
    assert_architecture_group_ids(
        &fixture,
        &repair_action_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_domain_model_context",
        ],
    );
    let domain_model_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "architecture_domain_model_context".to_string(),
    })
    .expect("read architecture repair domain model group");
    assert!(domain_model_group
        .fields
        .get("contextProjection.requirementDetailTransfer.actors")
        .is_some());
    assert!(domain_model_group
        .fields
        .get("contextProjection.requirementDetailTransfer.capabilityGroups")
        .is_some());
    advance_architecture_to_section(&fixture, &repair_action_ref, "frontend_experience");
    assert_architecture_group_ids(
        &fixture,
        &repair_action_ref,
        &[
            "architecture_core_context",
            "architecture_section_contract",
            "architecture_frontend_context",
        ],
    );
    let frontend_core_fields =
        architecture_group_fields(&fixture, &repair_action_ref, "architecture_core_context");
    assert_architecture_scope_summary_fields(&frontend_core_fields);
    assert!(
        !frontend_core_fields.contains(
            &"contextProjection.requirementDetailTransfer.requirementDetails".to_string()
        ),
        "frontend architecture repair must not inherit broad requirement detail reads"
    );
    assert!(
        !frontend_core_fields.contains(&"repairContext.sourceRef".to_string()),
        "architecture repair read plan must not expose absent repairContext.sourceRef"
    );
    assert!(
        !frontend_core_fields
            .contains(&"contextProjection.phaseScope.acceptanceCandidates".to_string()),
        "frontend architecture repair must rely on focused frontend context"
    );
    let frontend_group = state::read_field_group(ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_action_ref.clone(),
        group_id: "architecture_frontend_context".to_string(),
    })
    .expect("read architecture repair frontend group");
    assert!(frontend_group
        .fields
        .get("frontendExperienceSource.confirmedFrontendExperienceRef")
        .or_else(|| frontend_group
            .fields
            .get("frontendExperienceSource.currentFrontendExperienceRef"))
        .is_some());
    assert!(frontend_group
        .fields
        .get("uiQualitySeed.scenarioCandidates")
        .is_some());
    assert!(frontend_group
        .fields
        .get("uiQualitySeed.requiredReferenceGroups")
        .is_some());
    assert!(frontend_group
        .fields
        .get("uiQualitySeed.qualityGatePreview")
        .is_some());
    assert!(frontend_group
        .fields
        .get("uiQualitySeed.designTokenAssetPlan")
        .is_some());
    advance_architecture_to_section(&fixture, &repair_action_ref, "runtime_delivery");
    assert_architecture_group_ids(
        &fixture,
        &repair_action_ref,
        &["architecture_core_context", "architecture_section_contract"],
    );
    let runtime_core_fields =
        architecture_group_fields(&fixture, &repair_action_ref, "architecture_core_context");
    for forbidden in [
        "contextProjection.phaseScope.included",
        "contextProjection.phaseScope.deferred",
        "contextProjection.phaseScope.excluded",
        "contextProjection.phaseScope.acceptanceCandidates",
        "contextProjection.requirementDetailTransfer.requirementDetails",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.businessFlows",
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.requirementDetailIds",
    ] {
        assert!(
            !runtime_core_fields.contains(&forbidden.to_string()),
            "runtime_delivery architecture repair must not read non-runtime field {forbidden}"
        );
    }
    assert_architecture_scope_summary_fields(&runtime_core_fields);
    let repair_runtime_template =
        architecture_section_contract(&fixture, &repair_action_ref, "runtime_delivery")
            ["resultTemplate"]["content"]
            .clone();
    assert!(repair_runtime_template
        .pointer("/runtimeDelivery/start/port")
        .is_none());
    assert!(repair_runtime_template
        .pointer("/runtimeDelivery/build/codeLevelExpectations")
        .and_then(Value::as_array)
        .is_some());
    assert_eq!(
        repair_runtime_template
            .pointer("/runtimeDelivery/taskPlanningGuidance/verificationBoundary"),
        Some(&json!("code_level_only"))
    );
    advance_architecture_to_section(&fixture, &repair_action_ref, "coverage");
    let coverage_core_fields =
        architecture_group_fields(&fixture, &repair_action_ref, "architecture_core_context");
    assert_architecture_scope_summary_fields(&coverage_core_fields);
    assert!(
        coverage_core_fields.contains(
            &"contextProjection.requirementDetailTransfer.requirementDetails".to_string()
        ),
        "coverage architecture repair still needs the detail index"
    );
    let repair_coverage_template =
        architecture_section_contract(&fixture, &repair_action_ref, "coverage")["resultTemplate"]
            ["content"]
            .clone();
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["acceptanceId"],
        json!("acc_1")
    );
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["statement"]
            .as_str()
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        "repair coverage template must preserve acceptance statements"
    );
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["type"],
        json!("module")
    );
    assert!(repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["refs"].is_array());
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["coverage"][0]["description"].is_string()
    );
    assert_eq!(
        repair_coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["kind"],
        json!("manual")
    );
    assert!(
        repair_coverage_template["acceptanceMatrix"][0]["verificationHints"][0]["description"]
            .is_string()
    );
    let frontend_authority_ref = frontend_group
        .fields
        .get("frontendExperienceSource.confirmedFrontendExperienceRef")
        .or_else(|| {
            frontend_group
                .fields
                .get("frontendExperienceSource.currentFrontendExperienceRef")
        })
        .and_then(|field| field.value.as_str())
        .expect("repair frontend authority ref");
    let frontend_section_contract =
        architecture_section_contract(&fixture, &repair_action_ref, "frontend_experience");
    let frontend_template_refs = frontend_section_contract["resultTemplate"]["content"]
        ["frontendExperience"]["sourceRefs"]
        .as_object()
        .expect("repair frontend sourceRefs");
    assert_eq!(frontend_template_refs.len(), 1);
    assert_eq!(
        frontend_template_refs["brainstormFrontendExperienceRef"],
        json!(frontend_authority_ref)
    );
    let repair_frontend_quality_contract = &frontend_section_contract["resultTemplate"]["content"]
        ["frontendExperience"]["uiQualityContract"];
    assert!(
        repair_frontend_quality_contract
            .get("semanticTokenPolicy")
            .is_none(),
        "repair frontend template must not ask agents to write MCP-owned semanticTokenPolicy"
    );
    assert!(
        repair_frontend_quality_contract
            .get("referenceProfile")
            .is_none(),
        "repair frontend template must not ask agents to write MCP-owned referenceProfile"
    );
    assert!(
        repair_frontend_quality_contract
            .get("qualityGates")
            .is_none(),
        "repair frontend template must not ask agents to write MCP-owned qualityGates"
    );
    assert!(
        repair_frontend_quality_contract
            .get("forbiddenUserVisibleContent")
            .is_none(),
        "repair frontend template must not ask agents to write MCP-owned forbiddenUserVisibleContent"
    );
    let repair_root = read_request_root_value(fixture.root_str(), &repair_action_ref);
    assert!(
        repair_root.get("sectionOutputs").is_none(),
        "architecture repair request root must not expose all section contracts"
    );
    let repair_output_contract = private_output_contract(&fixture, &repair_action_ref);
    assert_eq!(
        repair_output_contract["writeTargets"][0]["targetId"],
        json!("coverage")
    );
    assert!(
        repair_output_contract["schemaShape"]
            .get("properties")
            .is_none(),
        "architecture repair outputContract schemaShape must be the current section shape, not the full Rust candidate schema"
    );
    assert!(
        repair_output_contract["schemaShape"]
            .pointer("/content/frontendExperience/uiQualityContract/referenceProfile")
            .is_none(),
        "architecture repair outputContract schemaShape must not expose MCP-owned uiQuality referenceProfile"
    );
    let result = complete_architecture_sections(&fixture, &repair_action_ref);

    assert_eq!(result["state"], "auto_runnable", "{result:#}");
    assert_eq!(result["next"]["kind"], "write_artifact");
    assert_eq!(result["next"]["artifactKind"], "task_plan_candidate");
    assert_eq!(result["next"]["submitTool"], "loom.taskPlanAcceptFile");
    let new_taskplan_request_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "taskPlanRequestRef");
    assert_ne!(new_taskplan_request_ref, old_taskplan_request_ref);
}

#[test]
fn brainstorm_submit_rejects_stale_request_binding() {
    let fixture = Fixture::new("submit-stale-request");
    let request_ref = start_brainstorm_candidate_write_request(&fixture);
    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let delivery_index = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&delivery_index).expect("read index"))
            .expect("parse index");
    index["phases"][0]["latestRefs"]["brainstormRequestRef"] =
        Value::String("loom://projects/stale/requests/other".to_string());
    write_json_atomic(&delivery_index, &index).expect("write stale index");
    write_candidate_target(&fixture, &request_ref, &valid_candidate_json());

    let result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );

    assert_eq!(result["state"], "failed");
    assert_eq!(result["error"]["code"], "STALE_BRAINSTORM_REQUEST");
    assert_eq!(result["error"]["recoveryTool"], "loom.continue");
}

fn write_brainstorm_request(
    fixture: &Fixture,
    request_id: &str,
    write_target: bool,
) -> state::StoredRequest {
    if write_target {
        write_json_atomic(
            &fixture.root.join(".loom/agent-writable/candidate.json"),
            &json!({ "summary": "ok" }),
        )
        .expect("write target");
    }
    write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: request_id.to_string(),
            request_kind: "brainstorm_candidate".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "outputContract": {
                    "artifactKind": "brainstorm_candidate",
                    "submitTool": "loom.brainstormAcceptFile",
                    "writeMode": "single_json",
                    "writeTargets": [{
                        "targetId": "candidate",
                        "path": ".loom/agent-writable/candidate.json",
                        "required": true,
                        "description": "Brainstorm candidate JSON."
                    }]
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "required": true,
                        "purpose": "Read core fields.",
                        "whenToRead": "Before writing.",
                        "selectors": delivery_core::read_selectors_value_from_paths([
                            "outputContract.writeTargets"
                        ])
                    }]
                }
            }),
        },
    )
    .expect("write request")
}

fn call_submit(tool_name: &str, request_ref: &str, project_root: &str) -> serde_json::Value {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request for submit");
    let written_target_ids = inspected
        .write_targets
        .iter()
        .filter_map(|target| target.get("targetId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root,
        "requestRef": request_ref,
        "writtenTargetIds": written_target_ids
    })
    .as_object()
    .expect("arguments object")
    .clone();
    server
        .invoke_tool(tool_name, Some(arguments))
        .expect("call tool")
        .structured_content
        .expect("structured content")
}

fn continue_delivery(project_root: &str) -> serde_json::Value {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root
    })
    .as_object()
    .expect("arguments object")
    .clone();
    server
        .invoke_tool("loom.continue", Some(arguments))
        .expect("continue")
        .structured_content
        .expect("structured content")
}

fn start_brainstorm_request(fixture: &Fixture) -> String {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": fixture.root_str(),
        "requestText": "实现证券账户开户流程"
    })
    .as_object()
    .expect("arguments object")
    .clone();
    let result = server
        .invoke_tool("loom.plan", Some(arguments))
        .expect("plan call")
        .structured_content
        .expect("structured content");
    result["requestRef"]
        .as_str()
        .expect("requestRef")
        .to_string()
}

fn start_brainstorm_candidate_write_request(fixture: &Fixture) -> String {
    let server = LoomMcpServer::default();
    let mut request_ref = start_brainstorm_request(fixture);

    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "phase_scope",
        "确认第一阶段为证券账户模块闭环。",
        json!({
            "scope": {
                "included": ["证券账户开户", "证券账户挂失补办", "证券账户销户", "账户状态管理"],
                "deferred": ["资金账户", "交易客户端", "中央撮合"],
                "excluded": []
            },
            "recommendation": {
                "label": "证券账户模块闭环",
                "reason": "证券账户是资金账户和交易链路的上游基础对象。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("concept requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "concept_grounding",
        "确认证券账户业务规则、状态和边界。",
        json!({
            "objects": ["证券账户"],
            "operations": ["开户", "挂失补办", "销户"],
            "rules": ["开户需要资格校验", "挂失后冻结证券", "销户前必须清空持仓"],
            "boundaries": ["资金账户递延", "交易客户端递延"]
        }),
    )["requestRef"]
        .as_str()
        .expect("frontend requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "frontend_experience",
        "确认工作人员后台证券账户管理页面路径。",
        json!({
            "required": true,
            "surfaces": ["证券账户管理页面"],
            "targetDiscovery": ["分页查询列表", "按账户号、姓名、证件号查询"],
            "operationPaths": ["开户从新建入口进入", "挂失补办和销户先查询并选择目标账户"],
            "mustNot": ["不能只靠内部主键触发办理动作"]
        }),
    )["requestRef"]
        .as_str()
        .expect("final summary requestRef")
        .to_string();
    let write_action = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "final_summary",
        "用户已确认阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["证券账户模块闭环", "开户/挂失补办/销户规则", "工作人员后台办理路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    let candidate_request_ref = write_action["next"]["requestRef"]
        .as_str()
        .expect("candidate write requestRef")
        .to_string();
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: candidate_request_ref.clone(),
    })
    .expect("inspect candidate write request");
    assert_eq!(inspected.request_kind, "brainstorm_candidate_write");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.brainstormAcceptFile")
    );
    assert_eq!(inspected.write_targets.len(), 1);
    assert!(inspected
        .read_groups
        .iter()
        .any(|group| group.group_id == "candidate_write_contract"));
    candidate_request_ref
}

fn confirm_brainstorm_block(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
    summary: &str,
    confirmed_data: Value,
) -> Value {
    if block != "final_summary" {
        run_knowledge_context(server, fixture, request_ref, block);
    }
    let arguments = json!({
        "projectRoot": fixture.root_str(),
        "requestRef": request_ref,
        "block": block,
        "summary": summary,
        "confirmedData": confirmed_data
    })
    .as_object()
    .expect("arguments object")
    .clone();
    let result = server
        .invoke_tool("loom.brainstormConfirmBlock", Some(arguments))
        .expect("confirm brainstorm block")
        .structured_content
        .expect("structured content");
    assert!(
        result["state"] == "user_gate" || result["state"] == "auto_runnable",
        "{result:#}"
    );
    result
}

fn confirm_phase2_brainstorm_to_candidate_write(
    fixture: &Fixture,
    phase_2_request_ref: &str,
) -> String {
    let server = LoomMcpServer::default();
    let mut request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        phase_2_request_ref,
        "phase_scope",
        "确认第二阶段为资金账户基础能力。",
        json!({
            "scope": {
                "included": ["资金账户开户", "密码管理", "存款与取款", "证券账户与资金账户关联"],
                "deferred": ["交易客户端", "中央撮合", "行情发布"],
                "excluded": []
            },
            "recommendation": {
                "label": "资金账户基础能力",
                "reason": "资金账户承接已完成的证券账户闭环，是交易客户端和撮合前的依赖。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 concept requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "concept_grounding",
        "确认资金账户业务规则、状态和边界。",
        json!({
            "objects": ["资金账户", "证券账户关联关系"],
            "operations": ["开户", "密码管理", "存款", "取款", "账户关联"],
            "rules": ["资金账户负责现金余额和资金冻结", "证券账户负责交易身份和持仓", "资金账户销户前需要清空余额和解除关联"],
            "boundaries": ["交易撮合递延", "行情发布递延"]
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 frontend requestRef")
        .to_string();
    request_ref = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "frontend_experience",
        "确认工作人员后台资金账户管理页面路径。",
        json!({
            "required": true,
            "surfaces": ["资金账户管理页面"],
            "targetDiscovery": ["分页查询列表", "按投资者、资金账户号、关联证券账户查询"],
            "operationPaths": ["开户从新建入口进入", "存取款和关联操作先查询并选择目标账户"],
            "mustNot": ["不能把资金账户能力混入证券账户模块"]
        }),
    )["requestRef"]
        .as_str()
        .expect("phase2 final summary requestRef")
        .to_string();
    let write_action = confirm_brainstorm_block(
        &server,
        fixture,
        &request_ref,
        "final_summary",
        "用户已确认第二阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["资金账户基础能力", "开户/密码/存取款/账户关联规则", "工作人员后台办理路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    write_action["next"]["requestRef"]
        .as_str()
        .expect("phase2 candidate write requestRef")
        .to_string()
}

fn run_knowledge_context(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
) {
    let knowledge_plan = server
        .invoke_tool(
            "loom.readFieldGroup",
            Some(
                json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "groupId": "knowledge_context_plan"
                })
                .as_object()
                .expect("arguments object")
                .clone(),
            ),
        )
        .expect("read knowledge context plan")
        .structured_content
        .expect("structured content");
    let steps = knowledge_plan["fields"]["knowledgeQueryPlan"]["blocks"][block]["executionOrder"]
        .as_array()
        .expect("knowledge executionOrder");
    for step in steps {
        let step_id = step["stepId"].as_str().expect("stepId");
        if step["repeatMode"].as_str() == Some("per_candidate_phase_cut") {
            for query_id in ["capability_closure_A", "capability_closure_B"] {
                server
                    .invoke_tool(
                        "loom.knowledgeBrainstormContext",
                        Some(
                            json!({
                                "projectRoot": fixture.root_str(),
                                "requestRef": request_ref,
                                "block": block,
                                "stepId": step_id,
                                "queryId": query_id,
                                "querySubject": format!("{block} {step_id} {query_id}"),
                                "naturalLanguageQuery": "证券账户 开户 挂失 补办 销户 资金账户 交易 依赖 闭环",
                                "semanticFocus": ["证券账户", "开户", "挂失", "补办", "销户"]
                            })
                            .as_object()
                            .expect("arguments object")
                            .clone(),
                        ),
                    )
                    .expect("knowledge brainstorm context");
            }
            continue;
        }
        server
            .invoke_tool(
                "loom.knowledgeBrainstormContext",
                Some(
                    json!({
                        "projectRoot": fixture.root_str(),
                        "requestRef": request_ref,
                        "block": block,
                        "stepId": step_id,
                        "querySubject": format!("{block} {step_id}"),
                        "naturalLanguageQuery": "证券账户 开户 挂失 补办 销户 资金账户 交易 依赖 闭环",
                        "semanticFocus": ["证券账户", "开户", "挂失", "补办", "销户"]
                    })
                    .as_object()
                    .expect("arguments object")
                    .clone(),
                ),
            )
            .expect("knowledge brainstorm context");
    }
}

fn write_candidate_target(fixture: &Fixture, request_ref: &str, value: &Value) {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect request");
    let target = inspected.write_targets.first().expect("write target");
    let path = target["path"].as_str().expect("target path");
    write_json_atomic(&fixture.root.join(path), value).expect("write candidate");
}

fn start_existing_project_architecture_flow(fixture: &Fixture) -> String {
    start_existing_project_architecture_flow_with_candidate(fixture, valid_candidate_json())
}

fn start_existing_project_architecture_flow_with_candidate(
    fixture: &Fixture,
    candidate: Value,
) -> String {
    write_json_atomic(
        &fixture.root.join("package.json"),
        &json!({ "name": "loom-fixture", "private": true }),
    )
    .expect("write package.json");
    std::fs::create_dir_all(fixture.root.join("src")).expect("create src");
    std::fs::write(
        fixture.root.join("src/main.tsx"),
        "export const app = true;\n",
    )
    .expect("write entrypoint");

    let request_ref = start_brainstorm_candidate_write_request(fixture);
    write_candidate_target(fixture, &request_ref, &candidate);

    let brainstorm_result = call_submit(
        "loom.brainstormAcceptFile",
        &request_ref,
        fixture.root_str(),
    );
    let baseline_request_ref = brainstorm_result["next"]["requestRef"]
        .as_str()
        .expect("baseline requestRef")
        .to_string();
    write_candidate_target(
        fixture,
        &baseline_request_ref,
        &technical_baseline_candidate_json("existing_project", "policy_auto_accept"),
    );
    let baseline_result = call_submit(
        "loom.technicalBaselineAcceptFile",
        &baseline_request_ref,
        fixture.root_str(),
    );
    let repository_context_request_ref = baseline_result["next"]["requestRef"]
        .as_str()
        .expect("repository context requestRef")
        .to_string();

    let delivery_id = request_delivery_id(fixture.root_str(), &request_ref);
    let brainstorm_contract_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "brainstormContract");
    let technical_baseline_ref =
        latest_ref_for_phase(fixture.root_str(), &delivery_id, "technicalBaseline");
    write_candidate_target(
        fixture,
        &repository_context_request_ref,
        &repository_context_candidate_json(
            &repository_context_request_ref,
            &brainstorm_contract_ref,
            &technical_baseline_ref,
        ),
    );

    let repository_result = call_submit(
        "loom.repositoryContextAcceptFile",
        &repository_context_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        repository_result["state"], "auto_runnable",
        "{repository_result:#}"
    );
    assert_eq!(
        repository_result["next"]["artifactKind"],
        "architecture_section_candidate"
    );
    repository_result["next"]["requestRef"]
        .as_str()
        .expect("architecture requestRef")
        .to_string()
}

fn complete_architecture_sections(fixture: &Fixture, architecture_request_ref: &str) -> Value {
    complete_architecture_sections_with(
        fixture,
        architecture_request_ref,
        architecture_section_candidate_json,
    )
}

fn complete_architecture_sections_with(
    fixture: &Fixture,
    architecture_request_ref: &str,
    candidate_fn: fn(&Fixture, &str) -> Value,
) -> Value {
    let current_request_ref = architecture_request_ref.to_string();
    let mut last = json!(null);
    let section_order = [
        "foundation",
        "domain_contract",
        "behavior",
        "frontend_experience",
        "runtime_delivery",
        "coverage",
    ];
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: current_request_ref.clone(),
    })
    .expect("inspect current architecture request");
    let current_section = inspected.write_targets[0]["targetId"]
        .as_str()
        .expect("current architecture target");
    let start_index = section_order
        .iter()
        .position(|section| *section == current_section)
        .expect("known architecture section");
    for expected_section in section_order.iter().skip(start_index) {
        let inspected = state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: current_request_ref.clone(),
        })
        .expect("inspect current architecture request");
        assert_eq!(
            inspected.write_targets[0]["targetId"],
            json!(*expected_section)
        );

        write_candidate_target(
            fixture,
            &current_request_ref,
            &candidate_fn(fixture, &current_request_ref),
        );

        let submit_tool = inspected.submit_tool.as_deref().expect("submit tool");
        last = call_submit(submit_tool, &current_request_ref, fixture.root_str());

        if *expected_section != "coverage" {
            assert_eq!(last["state"], "auto_runnable", "{last:#}");
            assert_eq!(
                last["next"]["requestRef"],
                json!(current_request_ref),
                "{last:#}"
            );
        }
    }
    last
}

fn advance_architecture_to_section(
    fixture: &Fixture,
    architecture_request_ref: &str,
    target: &str,
) {
    for _ in 0..6 {
        let inspected = state::inspect_request(InspectRequestInput {
            project_root: fixture.root_str().to_string(),
            request_ref: architecture_request_ref.to_string(),
        })
        .expect("inspect current architecture request");
        let current = inspected.write_targets[0]["targetId"]
            .as_str()
            .expect("current architecture target");
        if current == target {
            return;
        }
        assert_ne!(
            current, "coverage",
            "target section {target} was not reached"
        );
        write_candidate_target(
            fixture,
            architecture_request_ref,
            &architecture_section_candidate_json(fixture, architecture_request_ref),
        );
        let submit_tool = inspected.submit_tool.as_deref().expect("submit tool");
        let result = call_submit(submit_tool, architecture_request_ref, fixture.root_str());
        assert_eq!(result["state"], "auto_runnable", "{result:#}");
        assert_eq!(
            result["next"]["requestRef"],
            json!(architecture_request_ref)
        );
    }
    panic!("target section {target} was not reached");
}

fn assert_architecture_group_ids(fixture: &Fixture, request_ref: &str, expected: &[&str]) {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect architecture request");
    let actual = inspected
        .read_groups
        .iter()
        .map(|group| group.group_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{actual:#?}");
}

fn architecture_group_fields(fixture: &Fixture, request_ref: &str, group_id: &str) -> Vec<String> {
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect architecture request");
    inspected
        .read_groups
        .iter()
        .find(|group| group.group_id == group_id)
        .unwrap_or_else(|| panic!("missing architecture read group {group_id}"))
        .expanded_fields()
}

fn assert_architecture_scope_summary_fields(fields: &[String]) {
    for forbidden in [
        "contextProjection.phaseScope.included",
        "contextProjection.phaseScope.deferred",
        "contextProjection.phaseScope.excluded",
    ] {
        assert!(
            !fields.contains(&forbidden.to_string()),
            "architecture read group must not expose broad phase scope field {forbidden}"
        );
    }
    for required in [
        "contextProjection.phaseScopeSummary.includedIds",
        "contextProjection.phaseScopeSummary.includedLabels",
        "contextProjection.phaseScopeSummary.includedItems",
        "contextProjection.phaseScopeSummary.deferredIds",
        "contextProjection.phaseScopeSummary.deferredLabels",
        "contextProjection.phaseScopeSummary.deferredItems",
        "contextProjection.phaseScopeSummary.excludedIds",
        "contextProjection.phaseScopeSummary.excludedLabels",
        "contextProjection.phaseScopeSummary.excludedItems",
    ] {
        assert!(
            fields.contains(&required.to_string()),
            "architecture read group must expose compact phase scope field {required}"
        );
    }
}

fn write_taskplan_grouped_candidates(fixture: &Fixture, request_ref: &str) {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    let allowed_read_fields = inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .collect::<BTreeSet<_>>();
    let mut fields_to_read = vec![
        "allowedRefs.scopeRefs".to_string(),
        "allowedRefs.acceptanceRefs".to_string(),
        "allowedRefs.requirementDetailIds".to_string(),
        "allowedRefs.moduleRefs".to_string(),
        "allowedRefs.entityRefs".to_string(),
        "allowedRefs.interfaceRefs".to_string(),
        "allowedRefs.userFlowRefs".to_string(),
        "allowedRefs.stateMachineRefs".to_string(),
        "outputContract.outlineFile".to_string(),
        "outputContract.groupFilePattern".to_string(),
    ];
    if allowed_read_fields.contains("outputContract.runtimeDeliveryClosureTaskTemplate") {
        fields_to_read.push("outputContract.runtimeDeliveryClosureTaskTemplate".to_string());
    }
    for field in [
        "allowedRefs.decisionRefs",
        "allowedRefs.nfrRefs",
        "allowedRefs.riskRefs",
    ] {
        if allowed_read_fields.contains(field) {
            fields_to_read.push(field.to_string());
        }
    }
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: fields_to_read,
    })
    .expect("read taskplan fields")
    .fields;
    let allowed_refs = json!({
        "scopeRefs": field_value(&fields, "allowedRefs.scopeRefs"),
        "acceptanceRefs": field_value(&fields, "allowedRefs.acceptanceRefs"),
        "requirementDetailIds": field_value(&fields, "allowedRefs.requirementDetailIds"),
        "moduleRefs": field_value(&fields, "allowedRefs.moduleRefs"),
        "entityRefs": field_value(&fields, "allowedRefs.entityRefs"),
        "interfaceRefs": field_value(&fields, "allowedRefs.interfaceRefs"),
        "userFlowRefs": field_value(&fields, "allowedRefs.userFlowRefs"),
        "stateMachineRefs": field_value(&fields, "allowedRefs.stateMachineRefs"),
        "decisionRefs": field_value(&fields, "allowedRefs.decisionRefs"),
        "nfrRefs": field_value(&fields, "allowedRefs.nfrRefs"),
        "riskRefs": field_value(&fields, "allowedRefs.riskRefs")
    });
    let decision_refs = first_ref_array(&allowed_refs["decisionRefs"]);
    let nfr_refs = first_ref_array(&allowed_refs["nfrRefs"]);
    let risk_refs = first_ref_array(&allowed_refs["riskRefs"]);
    let scope_id = allowed_refs["scopeRefs"][0].as_str().expect("scope ref");
    let acceptance_id = allowed_refs["acceptanceRefs"][0]
        .as_str()
        .expect("acceptance ref");
    let detail_id = allowed_refs["requirementDetailIds"][0]
        .as_str()
        .expect("detail id");
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let group_id = "group-account";
    let task_id = "task-account-001";
    let runtime_closure_template =
        field_value(&fields, "outputContract.runtimeDeliveryClosureTaskTemplate");
    let has_runtime_closure = runtime_closure_template.is_object();
    let closure_group_id = "group-runtime-delivery-closure";
    let closure_task_id = "task-runtime-delivery-closure";
    let outline_groups = if has_runtime_closure {
        json!([
            {
                "groupId": group_id,
                "title": "Account capability",
                "objective": "Implement the account capability slice.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            },
            {
                "groupId": closure_group_id,
                "title": "Runtime delivery closure",
                "objective": "Verify the final RuntimeDeliveryContract code-level closure.",
                "dependsOn": [group_id],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [closure_task_id]
            }
        ])
    } else {
        json!([{
            "groupId": group_id,
            "title": "Account capability",
            "objective": "Implement the account capability slice.",
            "dependsOn": [],
            "scopeRefs": [scope_id],
            "acceptanceRefs": [acceptance_id],
            "taskIds": [task_id]
        }])
    };
    write_json_atomic(
        &fixture.root.join(outline_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "taskPlanId": "taskplan-phase-1",
            "groups": outline_groups,
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan outline");
    let group_file = group_pattern.replace("{groupId}", group_id);
    let frontend_requirement_template =
        frontend_requirement_template_from_taskplan_request(fixture, request_ref);
    write_json_atomic(
        &fixture.root.join(group_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "group": {
                "groupId": group_id,
                "title": "Account capability",
                "objective": "Implement the account capability slice.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            },
            "tasks": [{
                "taskId": task_id,
                "groupId": group_id,
                "title": "Implement account flow",
                "taskKind": "ui_flow_increment",
                "implementationActions": ["create_or_update_interface", "wire_reference_in_api_or_ui", "add_or_update_tests"],
                "objective": "Implement account lifecycle behavior, UI action wiring, and success feedback.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "requirementDetailRefs": [detail_id],
                "writeBoundary": {
                    "forbiddenPaths": [".loom"],
                    "artifactRefs": {
                        "modules": ["module.account-service"],
                        "entities": [allowed_refs["entityRefs"].get(0).and_then(Value::as_str).unwrap_or("entity.account")],
                        "interfaces": [allowed_refs["interfaceRefs"].get(0).and_then(Value::as_str).unwrap_or("api.account")],
                        "userFlows": [allowed_refs["userFlowRefs"].get(0).and_then(Value::as_str).unwrap_or("flow.account-lifecycle")],
                        "stateMachines": [allowed_refs["stateMachineRefs"].get(0).and_then(Value::as_str).unwrap_or("machine.account-status")],
                        "decisions": decision_refs,
                        "nfrs": nfr_refs,
                        "risks": risk_refs
                    }
                },
                "verificationIntents": [{
                    "verificationId": "verify-account-001",
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "behavior": "Verify account lifecycle behavior and visible success feedback.",
                    "preferredEvidence": ["static_check"],
                    "acceptableEvidence": ["static_check", "manual_command_output"]
                }],
                "frontendExperienceRequirement": frontend_requirement_template,
                "conceptRefs": [],
                "conceptResponsibilities": [],
                "conceptVerificationIntents": []
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan group");
    if has_runtime_closure {
        let closure_group_file = group_pattern.replace("{groupId}", closure_group_id);
        let closure_requirement = runtime_closure_template["runtimeDeliveryRequirement"].clone();
        write_json_atomic(
            &fixture.root.join(closure_group_file),
            &json!({
                "schemaVersion": "1.0",
                "requestId": request_id,
                "deliveryId": delivery_id,
                "phaseId": phase_id,
                "status": "ready",
                "group": {
                    "groupId": closure_group_id,
                    "title": "Runtime delivery closure",
                    "objective": "Verify the final RuntimeDeliveryContract code-level closure.",
                    "dependsOn": [group_id],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "taskIds": [closure_task_id]
                },
                "tasks": [{
                    "taskId": closure_task_id,
                    "groupId": closure_group_id,
                    "title": "Verify runtime delivery closure",
                    "taskKind": "runtime_delivery_closure",
                    "implementationActions": ["implement_runtime_delivery_contract", "add_or_update_tests"],
                    "objective": "Verify build, start, runtime surfaces, probes, frontend/API serving, and environment fields against RuntimeDeliveryContract.",
                    "dependsOn": [],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "writeBoundary": {
                        "forbiddenPaths": [".loom"],
                        "artifactRefs": {
                            "modules": ["module.account-service"],
                            "entities": [],
                            "interfaces": [],
                            "userFlows": [],
                            "stateMachines": [],
                            "decisions": [],
                            "nfrs": [],
                            "risks": []
                        }
                    },
                    "verificationIntents": [{
                        "verificationId": "verify-runtime-delivery-closure",
                        "acceptanceRefs": [acceptance_id],
                        "requirementDetailRefs": [detail_id],
                        "behavior": "Verify runtime delivery contract fields are closed at code level.",
                        "preferredEvidence": ["static_check"],
                        "acceptableEvidence": ["static_check", "manual_command_output", "runtime_api_check"]
                    }],
                    "runtimeDeliveryRequirement": closure_requirement,
                    "conceptRefs": [],
                    "conceptResponsibilities": [],
                    "conceptVerificationIntents": []
                }],
                "createdAt": "2026-06-24T10:00:00+08:00"
            }),
        )
        .expect("write runtime closure group");
    }
}

fn write_taskplan_grouped_candidates_with_persistence_quality(
    fixture: &Fixture,
    request_ref: &str,
) {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "allowedRefs.scopeRefs".to_string(),
            "allowedRefs.acceptanceRefs".to_string(),
            "allowedRefs.requirementDetailIds".to_string(),
            "allowedRefs.moduleRefs".to_string(),
            "allowedRefs.entityRefs".to_string(),
            "allowedRefs.interfaceRefs".to_string(),
            "allowedRefs.userFlowRefs".to_string(),
            "allowedRefs.stateMachineRefs".to_string(),
            "allowedRefs.decisionRefs".to_string(),
            "allowedRefs.nfrRefs".to_string(),
            "allowedRefs.riskRefs".to_string(),
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan persistence fields")
    .fields;
    let scope_id = fields["allowedRefs.scopeRefs"].value[0]
        .as_str()
        .expect("scope ref");
    let acceptance_id = fields["allowedRefs.acceptanceRefs"].value[0]
        .as_str()
        .expect("acceptance ref");
    let detail_id = fields["allowedRefs.requirementDetailIds"].value[0]
        .as_str()
        .expect("detail id");
    let module_id = fields["allowedRefs.moduleRefs"].value[0]
        .as_str()
        .unwrap_or("module.account-service");
    let entity_id = fields["allowedRefs.entityRefs"].value[0]
        .as_str()
        .unwrap_or("entity.account");
    let interface_id = fields["allowedRefs.interfaceRefs"].value[0]
        .as_str()
        .unwrap_or("api.account.open");
    let user_flow_id = fields["allowedRefs.userFlowRefs"].value[0]
        .as_str()
        .unwrap_or("flow.account-lifecycle");
    let state_machine_id = fields["allowedRefs.stateMachineRefs"].value[0]
        .as_str()
        .unwrap_or("machine.account-status");
    let decision_refs = first_ref_array(&fields["allowedRefs.decisionRefs"].value);
    let nfr_refs = first_ref_array(&fields["allowedRefs.nfrRefs"].value);
    let risk_refs = first_ref_array(&fields["allowedRefs.riskRefs"].value);
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let backend_group_id = "group-backend";
    let frontend_group_id = "group-frontend";
    let backend_data_task_id = "task-backend-data-001";
    let backend_api_task_id = "task-backend-api-001";
    let frontend_task_id = "task-frontend-001";
    write_json_atomic(
        &fixture.root.join(outline_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "taskPlanId": "taskplan-phase-1",
            "groups": [
                {
                    "groupId": backend_group_id,
                    "title": "Backend persistence and API",
                    "objective": "Implement persisted backend capability.",
                    "dependsOn": [],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "taskIds": [backend_data_task_id, backend_api_task_id]
                },
                {
                    "groupId": frontend_group_id,
                    "title": "Frontend workflow",
                    "objective": "Implement staff-facing UI workflow.",
                    "dependsOn": [backend_group_id],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "taskIds": [frontend_task_id]
                }
            ],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan outline");

    let backend_group_file = group_pattern.replace("{groupId}", backend_group_id);
    write_json_atomic(
        &fixture.root.join(backend_group_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "group": {
                "groupId": backend_group_id,
                "title": "Backend persistence and API",
                "objective": "Implement persisted backend capability.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [backend_data_task_id, backend_api_task_id]
            },
            "tasks": [
                {
                    "taskId": backend_data_task_id,
                    "groupId": backend_group_id,
                    "title": "Implement persisted domain model",
                    "taskKind": "data_model_increment",
                    "implementationActions": [
                        "create_or_update_entity",
                        "create_or_update_persistence",
                        "create_entity_migration",
                        "create_entity_repository",
                        "add_or_update_tests"
                    ],
                    "objective": "Create the persisted entity, schema or migration, repository mapping, and provider-backed tests.",
                    "dependsOn": [],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "writeBoundary": {
                        "forbiddenPaths": [".loom"],
                        "artifactRefs": {
                            "modules": [module_id],
                            "entities": [entity_id],
                            "interfaces": [],
                            "userFlows": [],
                            "stateMachines": [state_machine_id],
                            "decisions": decision_refs,
                            "nfrs": nfr_refs,
                            "risks": risk_refs
                        }
                    },
                    "verificationIntents": [{
                        "verificationId": "verify-backend-data-001",
                        "acceptanceRefs": [acceptance_id],
                        "requirementDetailRefs": [detail_id],
                        "behavior": "Verify persisted fields, schema mapping, and repository readback with the selected provider.",
                        "preferredEvidence": ["automated_test"],
                        "acceptableEvidence": ["automated_test", "manual_command_output"]
                    }],
                    "conceptRefs": [],
                    "conceptResponsibilities": [],
                    "conceptVerificationIntents": []
                },
                {
                    "taskId": backend_api_task_id,
                    "groupId": backend_group_id,
                    "title": "Implement persisted API behavior",
                    "taskKind": "interface_increment",
                    "implementationActions": [
                        "create_or_update_interface",
                        "create_or_update_business_rule",
                        "wire_reference_in_api_or_ui",
                        "add_or_update_tests"
                    ],
                    "objective": "Expose create/list/detail API behavior on top of the persisted entity.",
                    "dependsOn": [backend_data_task_id],
                    "scopeRefs": [scope_id],
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "writeBoundary": {
                        "forbiddenPaths": [".loom"],
                        "artifactRefs": {
                            "modules": [module_id],
                            "entities": [entity_id],
                            "interfaces": [interface_id],
                            "userFlows": [],
                            "stateMachines": [state_machine_id],
                            "decisions": [],
                            "nfrs": [],
                            "risks": []
                        }
                    },
                    "verificationIntents": [{
                        "verificationId": "verify-backend-api-001",
                        "acceptanceRefs": [acceptance_id],
                        "requirementDetailRefs": [detail_id],
                        "behavior": "Verify API DTOs, validation errors, persisted create/read behavior, and query fields stay aligned.",
                        "preferredEvidence": ["automated_test"],
                        "acceptableEvidence": ["automated_test", "runtime_api_check"]
                    }],
                    "conceptRefs": [],
                    "conceptResponsibilities": [],
                    "conceptVerificationIntents": []
                }
            ],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write backend taskplan group");

    let frontend_requirement_template =
        frontend_requirement_template_from_taskplan_request(fixture, request_ref);
    let frontend_group_file = group_pattern.replace("{groupId}", frontend_group_id);
    write_json_atomic(
        &fixture.root.join(frontend_group_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "group": {
                "groupId": frontend_group_id,
                "title": "Frontend workflow",
                "objective": "Implement staff-facing UI workflow.",
                "dependsOn": [backend_group_id],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [frontend_task_id]
            },
            "tasks": [{
                "taskId": frontend_task_id,
                "groupId": frontend_group_id,
                "title": "Implement frontend workflow",
                "taskKind": "ui_flow_increment",
                "implementationActions": [
                    "create_or_update_ui_flow",
                    "wire_reference_in_api_or_ui",
                    "implement_frontend_experience_contract",
                    "add_or_update_tests"
                ],
                "objective": "Implement the staff-facing workflow UI and bind it to the backend API.",
                "dependsOn": [backend_api_task_id],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "requirementDetailRefs": [detail_id],
                "writeBoundary": {
                    "forbiddenPaths": [".loom"],
                    "artifactRefs": {
                        "modules": [module_id],
                        "entities": [entity_id],
                        "interfaces": [interface_id],
                        "userFlows": [user_flow_id],
                        "stateMachines": [state_machine_id],
                        "decisions": [],
                        "nfrs": [],
                        "risks": []
                    }
                },
                "verificationIntents": [{
                    "verificationId": "verify-frontend-001",
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "behavior": "Verify the UI displays the workflow, submits the action, and shows success or blocking feedback.",
                    "preferredEvidence": ["static_check"],
                    "acceptableEvidence": ["static_check", "runtime_api_check"]
                }],
                "frontendExperienceRequirement": frontend_requirement_template,
                "conceptRefs": [],
                "conceptResponsibilities": [],
                "conceptVerificationIntents": []
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write frontend taskplan group");
}

fn write_taskplan_grouped_candidates_for_workflow_closure(fixture: &Fixture, request_ref: &str) {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect taskplan request");
    let allowed_read_fields = inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .collect::<BTreeSet<_>>();
    let mut fields_to_read = vec![
        "allowedRefs.scopeRefs".to_string(),
        "allowedRefs.acceptanceRefs".to_string(),
        "allowedRefs.requirementDetailIds".to_string(),
        "allowedRefs.moduleRefs".to_string(),
        "allowedRefs.entityRefs".to_string(),
        "allowedRefs.interfaceRefs".to_string(),
        "allowedRefs.userFlowRefs".to_string(),
        "allowedRefs.stateMachineRefs".to_string(),
        "outputContract.outlineFile".to_string(),
        "outputContract.groupFilePattern".to_string(),
    ];
    if allowed_read_fields.contains("outputContract.runtimeDeliveryClosureTaskTemplate") {
        fields_to_read.push("outputContract.runtimeDeliveryClosureTaskTemplate".to_string());
    }
    for field in [
        "allowedRefs.decisionRefs",
        "allowedRefs.nfrRefs",
        "allowedRefs.riskRefs",
    ] {
        if allowed_read_fields.contains(field) {
            fields_to_read.push(field.to_string());
        }
    }
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: fields_to_read,
    })
    .expect("read taskplan fields")
    .fields;
    let scope_id = fields["allowedRefs.scopeRefs"].value[0]
        .as_str()
        .expect("scope ref");
    let acceptance_id = fields["allowedRefs.acceptanceRefs"].value[0]
        .as_str()
        .expect("acceptance ref");
    let detail_id = fields["allowedRefs.requirementDetailIds"].value[0]
        .as_str()
        .expect("detail id");
    let decision_refs = first_ref_array(&field_value(&fields, "allowedRefs.decisionRefs"));
    let nfr_refs = first_ref_array(&field_value(&fields, "allowedRefs.nfrRefs"));
    let risk_refs = first_ref_array(&field_value(&fields, "allowedRefs.riskRefs"));
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let group_id = "group-account-ui";
    let task_id = "task-account-ui-001";
    write_json_atomic(
        &fixture.root.join(outline_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "taskPlanId": "taskplan-phase-1",
            "groups": [{
                "groupId": group_id,
                "title": "Account UI workflow",
                "objective": "Wire the account UI workflow to the declared backend interface.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan outline");
    let group_file = group_pattern.replace("{groupId}", group_id);
    let frontend_requirement_template =
        frontend_requirement_template_from_taskplan_request(fixture, request_ref);
    write_json_atomic(
        &fixture.root.join(group_file),
        &json!({
            "schemaVersion": "1.0",
            "requestId": request_id,
            "deliveryId": delivery_id,
            "phaseId": phase_id,
            "status": "ready",
            "group": {
                "groupId": group_id,
                "title": "Account UI workflow",
                "objective": "Wire the account UI workflow to the declared backend interface.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "taskIds": [task_id]
            },
            "tasks": [{
                "taskId": task_id,
                "groupId": group_id,
                "title": "Wire account opening UI",
                "taskKind": "ui_flow_increment",
                "implementationActions": ["wire_reference_in_api_or_ui", "add_or_update_tests"],
                "objective": "Wire the account opening UI action to the declared open-account API and verify success feedback.",
                "dependsOn": [],
                "scopeRefs": [scope_id],
                "acceptanceRefs": [acceptance_id],
                "requirementDetailRefs": [detail_id],
                "writeBoundary": {
                    "forbiddenPaths": [".loom"],
                    "artifactRefs": {
                        "modules": ["module.account-service"],
                        "entities": ["entity.account"],
                        "interfaces": ["api.account.open"],
                        "userFlows": ["flow.account-lifecycle"],
                        "stateMachines": ["machine.account-status"],
                        "decisions": decision_refs,
                        "nfrs": nfr_refs,
                        "risks": risk_refs
                    }
                },
                "verificationIntents": [{
                    "verificationId": "verify-account-ui-001",
                    "acceptanceRefs": [acceptance_id],
                    "requirementDetailRefs": [detail_id],
                    "behavior": "Verify the UI action invokes the declared API and shows success feedback.",
                    "preferredEvidence": ["runtime_api_check"],
                    "acceptableEvidence": ["automated_test", "runtime_api_check", "manual_command_output"]
                }],
                "frontendExperienceRequirement": frontend_requirement_template,
                "conceptRefs": [],
                "conceptResponsibilities": [],
                "conceptVerificationIntents": []
            }],
            "createdAt": "2026-06-24T10:00:00+08:00"
        }),
    )
    .expect("write taskplan group");
}

fn first_taskplan_group_file(fixture: &Fixture, request_ref: &str) -> String {
    taskplan_group_files(fixture, request_ref)
        .into_iter()
        .next()
        .expect("first taskplan group file")
}

fn frontend_requirement_template_from_taskplan_request(
    fixture: &Fixture,
    request_ref: &str,
) -> Value {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.frontendExperienceRequirementTemplate".to_string()],
    })
    .expect("read frontend requirement template")
    .fields;
    fields["outputContract.frontendExperienceRequirementTemplate"]
        .value
        .clone()
}

fn taskplan_group_files(fixture: &Fixture, request_ref: &str) -> Vec<String> {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "outputContract.outlineFile".to_string(),
            "outputContract.groupFilePattern".to_string(),
        ],
    })
    .expect("read taskplan file fields")
    .fields;
    let outline_file = fields["outputContract.outlineFile"]
        .value
        .as_str()
        .expect("outline file");
    let group_pattern = fields["outputContract.groupFilePattern"]
        .value
        .as_str()
        .expect("group file pattern");
    let outline_path = fixture.root.join(outline_file);
    let outline_value: Value =
        serde_json::from_str(&std::fs::read_to_string(outline_path).expect("read outline file"))
            .expect("parse outline file");
    outline_value["groups"]
        .as_array()
        .expect("outline groups")
        .iter()
        .map(|group| {
            group_pattern.replace("{groupId}", group["groupId"].as_str().expect("group id"))
        })
        .collect()
}

fn write_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, true, false);
}

fn complete_frontend_quality_token_evidence_for_test(result: &mut Value) {
    complete_architecture_quality_evidence_for_test(result);
    if let Some(self_check) = result
        .get_mut("frontendQualitySelfCheck")
        .and_then(Value::as_object_mut)
    {
        self_check.insert("status".to_string(), json!("satisfied"));
        self_check.insert("knownGaps".to_string(), json!([]));
        if let Some(forbidden) = self_check
            .get_mut("forbiddenContentCheck")
            .and_then(Value::as_object_mut)
        {
            forbidden.insert("violations".to_string(), json!([]));
        }
        if let Some(gates) = self_check
            .get_mut("gateResults")
            .and_then(Value::as_array_mut)
        {
            for gate in gates {
                let gate_id = gate
                    .get("gateId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                gate["status"] = json!("satisfied");
                gate["files"] = json!(["src/App.tsx"]);
                gate["sourceChecks"] = json!(["src/App.tsx"]);
                gate["attemptedChecks"] = json!([]);
                gate["fallbackEvidence"] = json!([]);
                gate["blockedReason"] = Value::Null;
                if gate_id.contains("render")
                    || gate_id.contains("viewport")
                    || gate_id.contains("mobile")
                {
                    gate["viewportsChecked"] = json!(["desktop 1280x800", "mobile 390x844"]);
                }
                gate["evidence"] = json!(format!(
                    "Test candidate satisfies UI quality gate {gate_id} through src/App.tsx."
                ));
            }
        }
    }
    let Some(evidence) = result
        .pointer_mut("/frontendQualitySelfCheck/designTokenEvidence")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let strategy = evidence
        .get("strategyUsed")
        .and_then(Value::as_str)
        .unwrap_or("create_css_tokens")
        .to_string();
    if strategy != "not_applicable" {
        let asset_file = match strategy.as_str() {
            "create_tailwind_tokens" => "tailwind.config.js",
            _ => "src/styles/tokens.css",
        };
        evidence.insert("tokenAssetFiles".to_string(), json!([asset_file]));
        evidence.insert("tokenConsumerFiles".to_string(), json!(["src/App.tsx"]));
        evidence.insert(
            "mergeSummary".to_string(),
            json!("Token assets were reused or extended before page-level styling in this test candidate."),
        );
    }
    evidence.insert("parallelTokenSystemCreated".to_string(), json!(false));
}

fn complete_architecture_quality_evidence_for_test(result: &mut Value) {
    let verification_id = result
        .get("verificationResults")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("verificationId"))
        .and_then(Value::as_str)
        .unwrap_or("verify-account-001")
        .to_string();
    let Some(evidence_items) = result
        .get_mut("architectureQualityEvidence")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for evidence in evidence_items {
        evidence["status"] = json!("satisfied");
        evidence["verificationIds"] = json!([verification_id]);
        evidence["changedFiles"] = json!(["src/main.tsx"]);
        evidence["summary"] = json!(
            "The task implementation respects the assigned architecture quality requirement and links it to verification evidence."
        );
    }
}

fn complete_api_contract_evidence_for_test(result: &mut Value) {
    let verification_id = result
        .get("verificationResults")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("verificationId"))
        .and_then(Value::as_str)
        .unwrap_or("verify-account-001")
        .to_string();
    let Some(evidence_items) = result
        .get_mut("apiContractEvidence")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for evidence in evidence_items {
        evidence["status"] = json!("satisfied");
        evidence["verificationIds"] = json!([verification_id]);
        evidence["changedFiles"] = json!(["src/main.tsx"]);
        evidence["successPaths"] = json!(["declared API success path verified"]);
        evidence["errorPaths"] = json!(["declared API validation or business error path verified"]);
        evidence["summary"] = json!(
            "The task implementation preserves the declared API request, response, status, and error contract and links it to verification evidence."
        );
    }
}

fn mutate_task_result_candidate<F>(fixture: &Fixture, request_ref: &str, mutate: F)
where
    F: FnOnce(&mut Value),
{
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read task result file")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let result_path = fixture.root.join(result_file);
    let mut result: Value =
        serde_json::from_str(&std::fs::read_to_string(&result_path).expect("read task result"))
            .expect("parse task result");
    mutate(&mut result);
    write_json_atomic(&result_path, &result).expect("write mutated task result");
}

fn write_task_result_candidate_with_empty_changed_files(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate(fixture, request_ref);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read result file")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let result_path = fixture.root.join(result_file);
    let mut result: Value =
        serde_json::from_str(&std::fs::read_to_string(&result_path).expect("read task result"))
            .expect("parse task result");
    result["changedFiles"] = json!([]);
    write_json_atomic(&result_path, &result).expect("write empty changedFiles task result");
}

fn write_large_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, true, true);
}

fn write_task_result_candidate_without_requirement_detail_evidence(
    fixture: &Fixture,
    request_ref: &str,
) {
    write_task_result_candidate_with_detail_evidence(fixture, request_ref, false, false);
}

fn write_task_result_candidate_with_detail_evidence(
    fixture: &Fixture,
    request_ref: &str,
    include_detail_evidence: bool,
    include_large_text: bool,
) {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "task.requirementDetailRefs".to_string(),
            "task.verificationIntents".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution request fields")
    .fields;
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let detail_id = fields["task.requirementDetailRefs"].value[0]
        .as_str()
        .expect("requirement detail id");
    let verification_id = fields["task.verificationIntents"].value[0]["verificationId"]
        .as_str()
        .expect("verification id");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    let verification_summary = if include_large_text {
        format!("very large verification summary {}", "x".repeat(20_000))
    } else {
        "Static verification passed for the account flow.".to_string()
    };
    let detail_summary = if include_large_text {
        format!("very large detail evidence summary {}", "x".repeat(20_000))
    } else {
        "The account lifecycle detail is covered by the implemented flow and static verification."
            .to_string()
    };
    let result_notes = if include_large_text {
        json!([format!("very large execution note {}", "x".repeat(20_000))])
    } else {
        json!([])
    };
    let requirement_detail_evidence = if include_detail_evidence {
        json!([{
            "detailId": detail_id,
            "status": "satisfied",
            "verificationIds": [verification_id],
            "evidenceRefs": ["src/main.tsx"],
            "summary": detail_summary
        }])
    } else {
        json!([])
    };
    result["taskResultId"] = json!("result-task-account-001");
    result["taskId"] = json!(task_id);
    result["taskPlanId"] = json!(task_plan_id);
    result["status"] = json!("completed");
    result["changedFiles"] = json!(["src/main.tsx"]);
    result["noChangeReason"] = Value::Null;
    result["verificationResults"] = json!([{
        "verificationId": verification_id,
        "status": "passed",
        "evidenceType": "static_check",
        "summary": verification_summary
    }]);
    result["selfRepairSummary"] = json!({
        "attempted": false,
        "attemptCount": 0,
        "stopReason": "not_attempted",
        "progressObserved": false
    });
    result["failure"] = Value::Null;
    result["executionContinuity"] = json!({
        "taskResultSubmittedAfterVerification": true,
        "agentOwnedLongRunningWork": "none",
        "notes": []
    });
    result["notes"] = result_notes;
    result["requirementDetailEvidence"] = requirement_detail_evidence;
    result["blockedReasons"] = json!([]);
    result["createdAt"] = json!("2026-06-24T10:05:00+08:00");
    result["updatedAt"] = json!("2026-06-24T10:05:00+08:00");
    if let Some(self_check) = result
        .get_mut("frontendExperienceSelfCheck")
        .and_then(Value::as_object_mut)
    {
        self_check.insert("evidenceRefs".to_string(), json!(["src/App.tsx"]));
        self_check.insert(
            "summary".to_string(),
            json!("The frontend flow is wired to the declared task behavior and verified."),
        );
    }
    complete_frontend_quality_token_evidence_for_test(&mut result);
    complete_api_contract_evidence_for_test(&mut result);
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write task result");
}

fn complete_task_execution_to_review(fixture: &Fixture) -> String {
    complete_task_execution_to_review_with_candidate(fixture, valid_candidate_json())
}

fn complete_task_execution_to_review_with_candidate(fixture: &Fixture, candidate: Value) -> String {
    let mut execution_request_ref = start_planned_task_execution_with_candidate(fixture, candidate);
    for _ in 0..8 {
        write_task_result_candidate(fixture, &execution_request_ref);
        let task_result = call_submit(
            "loom.recordTaskResultFile",
            &execution_request_ref,
            fixture.root_str(),
        );
        assert_eq!(task_result["state"], "auto_runnable", "{task_result:#}");
        if task_result["next"]["artifactKind"] == json!("review_result") {
            return task_result["next"]["requestRef"]
                .as_str()
                .expect("review requestRef")
                .to_string();
        }
        assert_eq!(
            task_result["next"]["kind"], "execute_task",
            "{task_result:#}"
        );
        execution_request_ref = task_result["next"]["requestRef"]
            .as_str()
            .expect("next execution requestRef")
            .to_string();
    }
    panic!("execution did not reach review_result");
}

#[test]
fn task_result_repair_template_preserves_previous_changed_files_for_replacement() {
    let fixture = Fixture::new("task-result-repair-preserves-changed-files");
    let execution_request_ref = start_planned_task_execution_without_runtime_closure(&fixture);

    write_task_result_candidate(&fixture, &execution_request_ref);
    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");

    write_task_result_candidate_with_empty_changed_files(&fixture, &execution_request_ref);
    let invalid_replacement = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        invalid_replacement["state"], "auto_runnable",
        "{invalid_replacement:#}"
    );
    assert_eq!(
        invalid_replacement["next"]["artifactKind"],
        "task_result_repair"
    );
    let repair_request_ref = invalid_replacement["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    let repair_fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: repair_request_ref.to_string(),
        fields: vec!["outputContract.resultTemplate".to_string()],
    })
    .expect("read repair fields")
    .fields;

    assert_eq!(
        repair_fields["outputContract.resultTemplate"].value["changedFiles"],
        json!(["src/main.tsx"])
    );
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("frontendExperienceSelfCheck")
        .is_some());
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("runtimeDeliveryEvidence")
        .is_none());
    assert!(repair_fields["outputContract.resultTemplate"]
        .value
        .get("conceptEvidence")
        .is_none());
}

#[test]
fn task_result_submit_backfills_machine_owned_shape_fields() {
    let fixture = Fixture::new("task-result-backfills-machine-shape");
    let execution_request_ref = start_planned_task_execution_without_runtime_closure(&fixture);
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: execution_request_ref.clone(),
        fields: vec![
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read task result contract")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("result file");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["changedFiles"] = json!(["src/main.tsx"]);
    complete_frontend_quality_token_evidence_for_test(&mut result);
    for field in [
        "schemaVersion",
        "taskResultId",
        "taskId",
        "taskPlanId",
        "noChangeReason",
        "selfRepairSummary",
        "failure",
        "notes",
        "blockedReasons",
        "createdAt",
        "updatedAt",
    ] {
        result
            .as_object_mut()
            .expect("task result object")
            .remove(field);
    }
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write compact task result");

    let accepted = call_submit(
        "loom.recordTaskResultFile",
        &execution_request_ref,
        fixture.root_str(),
    );
    assert_eq!(accepted["state"], "auto_runnable", "{accepted:#}");
    assert_ne!(accepted["next"]["artifactKind"], "task_result_repair");

    let delivery_id = request_delivery_id(fixture.root_str(), &execution_request_ref);
    let persisted_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "latestTaskResult");
    let persisted: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.root.join(persisted_ref)).unwrap())
            .expect("parse persisted task result");
    assert_eq!(persisted["schemaVersion"], "1.0");
    assert!(persisted["taskResultId"]
        .as_str()
        .is_some_and(|value| value.starts_with("taskresult-")));
    assert_eq!(persisted["noChangeReason"], Value::Null);
    assert_eq!(
        persisted["selfRepairSummary"],
        json!({
            "attempted": false,
            "attemptCount": 0,
            "stopReason": "not_attempted",
            "progressObserved": false
        })
    );
    assert_eq!(persisted["failure"], Value::Null);
    assert!(persisted
        .get("notes")
        .is_none_or(|value| value.as_array().is_some_and(Vec::is_empty)));
    assert!(persisted
        .get("blockedReasons")
        .is_none_or(|value| value.as_array().is_some_and(Vec::is_empty)));
    assert!(persisted["createdAt"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(persisted["updatedAt"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}

fn start_planned_task_execution(fixture: &Fixture) -> String {
    start_planned_task_execution_with_candidate(fixture, valid_candidate_json())
}

fn start_frontend_quality_task_execution_without_architecture_quality(fixture: &Fixture) -> String {
    let architecture_request_ref = start_existing_project_architecture_flow_with_candidate(
        fixture,
        valid_candidate_with_frontend_json(),
    );
    let taskplan_result = complete_architecture_sections_with(
        fixture,
        &architecture_request_ref,
        architecture_section_candidate_with_workflow_closure_no_runtime_json,
    );
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef");
    write_taskplan_grouped_candidates_for_workflow_closure(fixture, taskplan_request_ref);
    let group_file = first_taskplan_group_file(fixture, taskplan_request_ref);
    let group_path = fixture.root.join(&group_file);
    let mut group_value: Value =
        serde_json::from_str(&std::fs::read_to_string(&group_path).expect("read group file"))
            .expect("parse group file");
    group_value["tasks"][0]["architectureQualityRequirementRefs"] = json!([]);
    group_value["tasks"][0]["apiContractRequirementRefs"] = json!([]);
    group_value["tasks"][0]["codeQualityRequirementRefs"] = json!([]);
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["decisions"] = json!([]);
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["nfrs"] = json!([]);
    group_value["tasks"][0]["writeBoundary"]["artifactRefs"]["risks"] = json!([]);
    write_json_atomic(&group_path, &group_value).expect("write frontend-only group file");

    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string()
}

fn start_planned_task_execution_without_runtime_closure(fixture: &Fixture) -> String {
    let architecture_request_ref = start_existing_project_architecture_flow(fixture);
    let taskplan_result = complete_architecture_sections(fixture, &architecture_request_ref);
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let delivery_id = request_delivery_id(fixture.root_str(), &architecture_request_ref);
    let aac_ref = latest_ref_for_phase(fixture.root_str(), &delivery_id, "architectureArtifact");
    let aac_path = fixture.root.join(&aac_ref);
    let mut aac: Value =
        serde_json::from_str(&std::fs::read_to_string(&aac_path).expect("read AAC"))
            .expect("parse AAC");
    aac["runtimeDelivery"] = Value::Null;
    write_json_atomic(&aac_path, &aac).expect("write AAC without runtime delivery");

    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(&delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    let latest_refs = index["phases"]
        .as_array_mut()
        .expect("phases")
        .iter_mut()
        .find(|phase| phase["phaseId"].as_str() == Some("phase-1"))
        .expect("phase-1")["latestRefs"]
        .as_object_mut()
        .expect("latestRefs object");
    latest_refs.remove("taskPlanRequestId");
    latest_refs.remove("taskPlanRequestRef");
    write_json_atomic(&index_path, &index).expect("write index without taskplan request");

    let result = execution::ExecutionDomainDispatcher.dispatch_route_action(
        fixture.root_str(),
        &delivery_id,
        "phase-1",
        &RouteAction {
            kind: RouteActionKind::TaskplanGeneration,
            source: "test".to_string(),
            reason: "regenerate_taskplan_request_without_runtime_closure".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: None,
            details: None,
            target_phase_id: None,
        },
    );
    let taskplan_result = serde_json::to_value(result).expect("serialize taskplan result");
    assert_eq!(
        taskplan_result["state"], "auto_runnable",
        "{taskplan_result:#}"
    );
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates(fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string()
}

fn start_planned_task_execution_with_candidate(fixture: &Fixture, candidate: Value) -> String {
    let architecture_request_ref =
        start_existing_project_architecture_flow_with_candidate(fixture, candidate);
    let taskplan_result = complete_architecture_sections(fixture, &architecture_request_ref);
    let taskplan_request_ref = taskplan_result["next"]["requestRef"]
        .as_str()
        .expect("taskplan requestRef")
        .to_string();
    write_taskplan_grouped_candidates(fixture, &taskplan_request_ref);
    let execution_result = call_submit(
        "loom.taskPlanAcceptFile",
        &taskplan_request_ref,
        fixture.root_str(),
    );
    assert_eq!(
        execution_result["state"], "auto_runnable",
        "{execution_result:#}"
    );
    execution_result["next"]["requestRef"]
        .as_str()
        .expect("execution requestRef")
        .to_string()
}

fn write_failed_task_result_candidate(fixture: &Fixture, request_ref: &str) {
    write_failed_task_result_candidate_with_id(
        fixture,
        request_ref,
        "result-failed-task-account-001",
    );
}

fn write_failed_task_result_candidate_with_id(
    fixture: &Fixture,
    request_ref: &str,
    task_result_id: &str,
) {
    let fields = execution_result_fields(fixture, request_ref);
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "taskResultId": task_result_id,
            "taskId": task_id,
            "taskPlanId": task_plan_id,
            "status": "failed",
            "changedFiles": [],
            "noChangeReason": null,
            "verificationResults": [{
                "verificationId": "verify-account-001",
                "status": "failed",
                "evidenceType": "static_check",
                "summary": "Static verification failed."
            }],
            "selfRepairSummary": {
                "attempted": true,
                "attemptCount": 1,
                "stopReason": "verification_failed",
                "progressObserved": true
            },
            "failure": {
                "code": "VERIFICATION_FAILED",
                "summary": "Verification failed for the current task."
            },
            "executionContinuity": {
                "taskResultSubmittedAfterVerification": true,
                "agentOwnedLongRunningWork": "none",
                "notes": []
            },
            "notes": [],
            "frontendExperienceSelfCheck": null,
            "runtimeDeliveryEvidence": null,
            "requirementDetailEvidence": [],
            "conceptEvidence": [],
            "blockedReasons": [],
            "createdAt": "2026-06-24T10:06:00+08:00",
            "updatedAt": "2026-06-24T10:06:00+08:00"
        }),
    )
    .expect("write failed task result");
}

fn write_blocked_task_result_candidate(
    fixture: &Fixture,
    request_ref: &str,
    code: &str,
    next_node: &str,
) {
    let fields = execution_result_fields(fixture, request_ref);
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("resultFile");
    let task_plan_id = fields["source.taskPlanId"]
        .value
        .as_str()
        .expect("taskPlanId");
    let task_id = fields["source.taskId"].value.as_str().expect("taskId");
    let mut result = fields["outputContract.resultTemplate"].value.clone();
    result["taskResultId"] = json!(format!("result-blocked-{code}"));
    result["taskId"] = json!(task_id);
    result["taskPlanId"] = json!(task_plan_id);
    result["status"] = json!("blocked");
    result["changedFiles"] = json!([]);
    result["noChangeReason"] = json!({
        "code": "BLOCKED",
        "summary": "The task is blocked by an upstream contract issue."
    });
    result["verificationResults"] = json!([{
        "verificationId": "verify-account-001",
        "status": "not_run",
        "evidenceType": "static_check",
        "summary": "Verification was not run because the task is blocked."
    }]);
    result["selfRepairSummary"] = json!({
        "attempted": false,
        "attemptCount": 0,
        "stopReason": "not_attempted",
        "progressObserved": false
    });
    result["failure"] = Value::Null;
    result["executionContinuity"] = json!({
        "taskResultSubmittedAfterVerification": true,
        "agentOwnedLongRunningWork": "none",
        "notes": []
    });
    result["notes"] = json!([]);
    result["frontendExperienceSelfCheck"] = Value::Null;
    result["frontendQualitySelfCheck"] = Value::Null;
    result["runtimeDeliveryEvidence"] = Value::Null;
    result["requirementDetailEvidence"] = json!([]);
    result["conceptEvidence"] = json!([]);
    result["blockedReasons"] = json!([{
        "code": code,
        "nextNode": next_node,
        "message": "The task is blocked by an upstream contract issue.",
        "details": {}
    }]);
    result["createdAt"] = json!("2026-06-24T10:06:00+08:00");
    result["updatedAt"] = json!("2026-06-24T10:06:00+08:00");
    write_json_atomic(&fixture.root.join(result_file), &result).expect("write blocked task result");
}

fn execution_result_fields(
    fixture: &Fixture,
    request_ref: &str,
) -> std::collections::BTreeMap<String, delivery_core::FieldReadResult> {
    state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.taskPlanId".to_string(),
            "source.taskId".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.resultTemplate".to_string(),
        ],
    })
    .expect("read execution request fields")
    .fields
}

fn write_review_result_candidate(
    fixture: &Fixture,
    request_ref: &str,
    decision: &str,
    next_action: &str,
    findings: Vec<Value>,
) {
    let finding_ids = findings
        .iter()
        .filter_map(|finding| finding.get("findingId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "source.phaseId".to_string(),
            "source.taskPlanId".to_string(),
            "source.taskPlanRunId".to_string(),
            "outputContract.resultFile".to_string(),
            "outputContract.allowedRefs.taskIds".to_string(),
            "outputContract.allowedRefs.acceptanceRefs".to_string(),
            "outputContract.allowedRefs.taskResultIds".to_string(),
        ],
    })
    .expect("read review fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("review result file");
    let first_task_id = fields["outputContract.allowedRefs.taskIds"]
        .value
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::to_string);
    let findings = findings
        .into_iter()
        .map(|mut finding| {
            if matches!(
                finding.get("severity").and_then(Value::as_str),
                Some("critical" | "major")
            ) {
                finding["taskRelevance"] = json!("direct");
                finding["scopeRelation"] = json!("within_task_changed_files");
                let route = finding
                    .get("recommendedNextAction")
                    .and_then(Value::as_str)
                    .unwrap_or("done");
                let has_refs = finding
                    .get("taskRefs")
                    .and_then(Value::as_array)
                    .map(|items| !items.is_empty())
                    .unwrap_or(false);
                if !has_refs && !matches!(route, "manual_review" | "needs_user_decision") {
                    if let Some(task_id) = &first_task_id {
                        finding["taskRefs"] = json!([task_id]);
                    }
                }
            }
            finding
        })
        .collect::<Vec<_>>();
    let acceptance_refs = fields["outputContract.allowedRefs.acceptanceRefs"]
        .value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let task_result_ids = fields["outputContract.allowedRefs.taskResultIds"]
        .value
        .as_array()
        .cloned()
        .unwrap_or_default();
    let supporting_task_results = task_result_ids
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let must_acceptance = acceptance_refs
        .iter()
        .filter_map(Value::as_str)
        .map(|acceptance_ref| {
            json!({
                "acceptanceRef": acceptance_ref,
                "status": "satisfied",
                "supportingTaskResults": supporting_task_results,
                "evidenceStatus": "sufficient",
                "notes": []
            })
        })
        .collect::<Vec<_>>();
    let total_must = must_acceptance.len();
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "reviewId": "review-phase-1",
            "source": {
                "requestId": request_id_from_ref(request_ref),
                "phaseId": field_value(&fields, "source.phaseId"),
                "taskPlanId": field_value(&fields, "source.taskPlanId"),
                "taskPlanRunId": field_value(&fields, "source.taskPlanRunId")
            },
            "decision": decision,
            "findings": findings,
            "coverageAssessment": {
                "mustAcceptance": must_acceptance,
                "summary": {
                    "totalMust": total_must,
                    "satisfied": total_must,
                    "insufficientEvidence": 0,
                    "notSatisfied": 0,
                    "notReviewed": 0
                }
            },
            "limitations": [],
            "pendingActions": [],
            "nextAction": {
                "type": next_action,
                "reason": "Review selected the next route.",
                "targetPhaseId": if next_action == "continue_to_next_phase" { json!("phase-2") } else { Value::Null },
                "findingRefs": if next_action == "done" || next_action == "continue_to_next_phase" { json!([]) } else { json!(finding_ids.clone()) }
            },
            "createdAt": "2026-06-24T10:10:00+08:00",
            "updatedAt": "2026-06-24T10:10:00+08:00"
        }),
    )
    .expect("write review result");
}

fn review_result_candidate_path(fixture: &Fixture, request_ref: &str) -> PathBuf {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read review result file")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("review result file");
    fixture.root.join(result_file)
}

fn mutate_review_result_candidate<F>(fixture: &Fixture, request_ref: &str, mutate: F)
where
    F: FnOnce(&mut Value),
{
    let path = review_result_candidate_path(fixture, request_ref);
    let mut candidate: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read review result"))
            .expect("parse review result");
    mutate(&mut candidate);
    write_json_atomic(&path, &candidate).expect("write mutated review result");
}

fn run_git(fixture: &Fixture, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(&fixture.root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_manual_review_resolution_candidate(fixture: &Fixture, request_ref: &str) {
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["outputContract.resultFile".to_string()],
    })
    .expect("read manual review fields")
    .fields;
    let result_file = fields["outputContract.resultFile"]
        .value
        .as_str()
        .expect("manual review resolution file");
    write_json_atomic(
        &fixture.root.join(result_file),
        &json!({
            "schemaVersion": "1.0",
            "manualReviewResolutionId": "manual-review-resolution-001",
            "manualReviewRequestId": request_id_from_ref(request_ref),
            "deliveryId": request_delivery_id(fixture.root_str(), request_ref),
            "phaseId": "phase-1",
            "userAnswer": {
                "text": "需要修改，请修复当前实现问题。",
                "selectedShortReply": "request_changes"
            },
            "decision": "request_changes",
            "changeRequest": {
                "summary": "修复当前实现问题。",
                "route": "execution_repair",
                "reason": "需要修改代码或验证证据。",
                "details": {}
            },
            "nextAction": {
                "type": "execution_repair",
                "targetNode": "execution",
                "reason": "User requested execution repair."
            },
            "createdAt": "2026-06-24T10:12:00+08:00"
        }),
    )
    .expect("write manual review resolution");
}

fn request_id_from_ref(request_ref: &str) -> String {
    request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref")
        .to_string()
}

fn field_value(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    field: &str,
) -> Value {
    fields
        .get(field)
        .map(|result| result.value.clone())
        .unwrap_or(Value::Null)
}

fn first_ref_array(value: &Value) -> Value {
    value
        .get(0)
        .and_then(Value::as_str)
        .map(|reference| json!([reference]))
        .unwrap_or_else(|| json!([]))
}

fn architecture_section_candidate_json(fixture: &Fixture, request_ref: &str) -> Value {
    let request_root = read_request_root_value(fixture.root_str(), request_ref);
    let request_id = request_root["requestId"].as_str().expect("requestId");
    let delivery_id = request_root["deliveryId"].as_str().expect("deliveryId");
    let phase_id = request_root["phaseId"].as_str().expect("phaseId");
    let section = request_root["sectionState"]["currentSection"]
        .as_str()
        .expect("currentSection");
    let inspected = state::inspect_request(InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
    })
    .expect("inspect architecture request");
    let readable_fields = inspected
        .read_groups
        .iter()
        .flat_map(delivery_core::ReadGroupRef::expanded_fields)
        .collect::<Vec<_>>();
    let requested_fields = [
        "allowedRefs.scopeRefs",
        "allowedRefs.acceptanceRefs",
        "allowedRefs.deferredScopeRefs",
        "allowedRefs.excludedScopeRefs",
        "allowedRefs.requirementDetailIds",
        "sourceRefs.technicalBaselineRef",
        "contextProjection.planningContractId",
        "contextProjection.technicalBaseline.technicalBaselineId",
        "contextProjection.requirementDetailTransfer.acceptanceDetails",
        "contextProjection.requirementDetailTransfer.requirementDetails",
        "frontendExperienceSource.confirmedFrontendExperienceRef",
        "frontendExperienceSource.currentFrontendExperienceRef",
    ]
    .iter()
    .filter(|field| readable_fields.contains(&field.to_string()))
    .map(|field| field.to_string())
    .collect::<Vec<_>>();
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.to_string(),
        fields: requested_fields,
    })
    .expect("read architecture request fields")
    .fields;
    let planning_contract_id = fields["contextProjection.planningContractId"]
        .value
        .as_str()
        .expect("planningContractId");
    let technical_baseline_id = fields["contextProjection.technicalBaseline.technicalBaselineId"]
        .value
        .as_str()
        .expect("technicalBaselineId");
    let acceptance_details = fields
        .get("contextProjection.requirementDetailTransfer.acceptanceDetails")
        .map(|field| &field.value)
        .unwrap_or(&Value::Null)
        .as_array()
        .cloned()
        .unwrap_or_default();
    let acceptance_priority = acceptance_details
        .first()
        .and_then(|item| item.get("priority"))
        .and_then(Value::as_str)
        .unwrap_or("must");
    let acceptance_statement = acceptance_details
        .first()
        .and_then(|item| item.get("statement"))
        .and_then(Value::as_str)
        .unwrap_or("Current phase acceptance is covered by the architecture.");
    let acceptance_refs = field_value(&fields, "allowedRefs.acceptanceRefs");
    let acceptance_id = acceptance_refs
        .get(0)
        .and_then(Value::as_str)
        .unwrap_or("acc_1")
        .to_string();
    let scope_refs = field_value(&fields, "allowedRefs.scopeRefs");
    let scope_id = scope_refs
        .get(0)
        .and_then(Value::as_str)
        .unwrap_or("scope_1")
        .to_string();
    let requirement_detail_ids = field_value(&fields, "allowedRefs.requirementDetailIds");
    let detail_id = requirement_detail_ids
        .get(0)
        .and_then(Value::as_str)
        .unwrap_or("detail.scope.scope_1.1")
        .to_string();
    let frontend_authority_ref = request_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            request_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
                .and_then(Value::as_str)
        });
    let technical_baseline_ref = fields
        .get("sourceRefs.technicalBaselineRef")
        .map(|field| &field.value)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let frontend_ui_quality_contract =
        architecture_section_contract(fixture, request_ref, "frontend_experience")
            ["resultTemplate"]["content"]["frontendExperience"]["uiQualityContract"]
            .clone();

    let content = match section {
        "foundation" => json!({
            "source": {
                "planningGenerationContractId": planning_contract_id,
                "technicalBaselineId": technical_baseline_id
            },
            "engineeringBoundary": {
                "summary": "Current phase stays inside the confirmed account-delivery boundary.",
                "applications": [],
                "modules": []
            },
            "modules": [{
                "moduleId": "module.account-service",
                "name": "account-service",
                "summary": "Handles the current phase account workflow."
            }]
        }),
        "domain_contract" => json!({
            "dataModel": {
                "entities": [{
                    "entityId": "entity.account",
                    "name": "Account"
                }],
                "relationships": [],
                "constraints": []
            },
            "interfaces": [{
                "interfaceId": "api.account",
                "name": "Account API"
            }]
        }),
        "behavior" => json!({
            "userFlows": [{
                "flowId": "flow.account-lifecycle",
                "name": "Account lifecycle"
            }],
            "stateMachines": [{
                "machineId": "machine.account-status",
                "name": "Account status"
            }]
        }),
        "frontend_experience" => json!({
            "frontendExperience": {
                "required": true,
                "surfaces": [],
                "dataViews": [],
                "actions": [],
                "operationPaths": [],
                "uiQualityContract": frontend_ui_quality_contract,
                "sourceRefs": {
                    "brainstormFrontendExperienceRef": frontend_authority_ref
                }
            }
        }),
        "runtime_delivery" => json!({
            "runtimeDelivery": {
                "status": "modified",
                "runtimeKind": "spring_boot_serves_react_static",
                "deploymentShape": "frontend-and-backend",
                "basis": {
                    "technicalBaselineRef": technical_baseline_ref
                },
                "build": {
                    "command": "cd web && npm run build && cd ../service && ./gradlew build",
                    "workingDirectory": ".",
                    "outputs": ["web/dist", "service/build/libs"],
                    "codeLevelExpectations": [
                        "Frontend assets and backend artifact are produced by declared build commands."
                    ]
                },
                "start": {
                    "command": "cd service && ./gradlew bootRun",
                    "workingDirectory": ".",
                    "port": 8080,
                    "codeLevelExpectations": [
                        "Backend starts the accepted API/runtime surface."
                    ]
                },
                "runtimeSurfaces": [{
                    "surfaceId": "runtime-preview-root",
                    "kind": "http",
                    "urlPath": "/",
                    "purpose": "Preview the staff-facing account workflow."
                }],
                "httpProbes": {
                    "previewPath": "/",
                    "apiPaths": ["/api/securities-accounts/runtime-info"],
                    "expectedStatus": "2xx_or_3xx"
                },
                "frontend": {
                    "required": true,
                    "kind": "vite_react",
                    "buildCommand": "cd web && npm run build",
                    "sourceRoot": "web",
                    "outputDir": "web/dist",
                    "servedBy": "spring_boot_static"
                },
                "api": {
                    "required": true,
                    "kind": "spring_boot",
                    "buildCommand": "cd service && ./gradlew build",
                    "entry": "service/src/main",
                    "basePath": "/api",
                    "probePaths": ["/api/securities-accounts/runtime-info"]
                },
                "environment": {
                    "required": [],
                    "optional": ["PORT"]
                },
                "deliveryMechanics": {
                    "staticAssets": {
                        "required": true,
                        "source": "web",
                        "output": "web/dist",
                        "servedBy": "spring_boot_static"
                    },
                    "api": {
                        "required": true,
                        "entry": "service/src/main",
                        "basePath": "/api",
                        "probePaths": ["/api/securities-accounts/runtime-info"]
                    },
                    "codegen": {
                        "required": "no",
                        "commands": [],
                        "codeLevelExpectations": []
                    }
                },
                "taskPlanningGuidance": {
                    "requireRuntimeDeliveryRequirementWhenTaskTouches": [
                        "build_or_packaging",
                        "runtime_entry",
                        "serving_or_routing",
                        "configuration_or_environment",
                        "generated_artifacts",
                        "runtime_surface"
                    ],
                    "doNotRequireForTaskKinds": [
                        "domain_only_validation",
                        "pure_unit_test_additions"
                    ],
                    "verificationBoundary": "code_level_only",
                    "doNotRequireCleanInstallOrContainerBuild": true
                }
            }
        }),
        "coverage" => json!({
            "acceptanceMatrix": [{
                "acceptanceId": acceptance_id.clone(),
                "priority": acceptance_priority,
                "statement": acceptance_statement,
                "coverageStatus": "covered",
                "coverage": [{
                    "type": "module",
                    "refs": ["module.account-service"],
                    "description": "Covered by the account-service module."
                }],
                "verificationHints": []
            }],
            "detailCoverage": [{
                "detailId": detail_id.clone(),
                "coverageStatus": "covered",
                "artifactRefs": {
                    "modules": ["module.account-service"],
                    "entities": [],
                    "fields": [],
                    "constraints": [],
                    "interfaces": [],
                    "userFlows": [],
                    "stateMachines": [],
                    "frontendDataViews": [],
                    "frontendActions": [],
                    "frontendOperationPaths": [],
                    "acceptanceMatrix": [acceptance_id.clone()]
                }
            }],
            "architectureQuality": {
                "decisions": [{
                    "decisionId": "adr-current-001",
                    "category": "architecture_style",
                    "title": "Keep the account slice inside the declared module boundary",
                    "status": "accepted",
                    "context": "The current phase needs a coherent architecture handoff for task planning without expanding beyond the accepted account capability.",
                    "decision": "Implement the phase through the account-service module and expose only the declared account interface surface.",
                    "alternativesConsidered": [{
                        "name": "Split the current account slice across unrelated modules",
                        "tradeoff": "Could look more layered in tests but would duplicate ownership for one phase.",
                        "rejectedBecause": "The accepted architecture module boundary is sufficient for this phase."
                    }],
                    "consequences": {
                        "positive": ["Task planning can assign one coherent module owner."],
                        "negative": ["Future phases must add new modules deliberately instead of by accident."],
                        "neutral": ["The decision can be revisited when a later phase adds a separate runtime boundary."]
                    },
                    "sourceRefs": {
                        "scopeRefs": [scope_id.clone()],
                        "acceptanceRefs": [acceptance_id.clone()],
                        "requirementDetailRefs": [detail_id.clone()]
                    },
                    "verificationHints": ["TaskPlan must assign this decision to the implementation task that owns module.account-service."]
                }],
                "nfrs": [{
                    "nfrId": "nfr-current-001",
                    "category": "maintainability",
                    "target": "Account workflow code remains traceable from accepted requirement detail to module, interface, and verification task.",
                    "rationale": "The execution chain needs architecture evidence without repeating full architecture prose in every task.",
                    "architectureRefs": {
                        "decisions": ["adr-current-001"],
                        "risks": ["risk-current-001"]
                    },
                    "verificationStrategy": "TaskResult architectureQualityEvidence cites the generated requirement and the task verification id."
                }],
                "risks": [{
                    "riskId": "risk-current-001",
                    "category": "maintainability",
                    "severity": "medium",
                    "likelihood": "medium",
                    "impact": "A task could expand beyond the current account boundary and make review unable to distinguish owned architecture work.",
                    "mitigation": "Assign the decision, NFR, and risk refs to the task write boundary and require task-level evidence.",
                    "ownerArtifactRefs": {
                        "modules": ["module.account-service"],
                        "interfaces": ["api.account"],
                        "decisions": ["adr-current-001"],
                        "nfrs": ["nfr-current-001"]
                    },
                    "verificationHints": ["Review should fail approval if the architecture quality refs are not assigned to a task and evidenced."]
                }]
            },
            "handoff": {
                "readyForTaskPlan": true,
                "blockingReasons": [],
                "nextNode": "task_plan"
            }
        }),
        other => panic!("unexpected architecture section: {other}"),
    };

    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "section": section,
        "status": "ready",
        "content": content,
        "createdAt": "2026-06-24T10:00:00+08:00"
    })
}

fn architecture_section_candidate_with_workflow_closure_json(
    fixture: &Fixture,
    request_ref: &str,
) -> Value {
    let mut candidate = architecture_section_candidate_json(fixture, request_ref);
    match candidate["section"].as_str().unwrap_or_default() {
        "domain_contract" => {
            candidate["content"]["dataModel"]["entities"] = json!([{
                "entityId": "entity.account",
                "name": "Account",
                "type": "aggregate",
                "moduleRefs": ["module.account-service"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "fields": [{
                    "fieldId": "field.account.id",
                    "name": "accountId",
                    "type": "string",
                    "required": true
                }],
                "constraints": []
            }]);
            candidate["content"]["interfaces"] = json!([{
                "interfaceId": "api.account.open",
                "name": "Open account API",
                "type": "http_api",
                "role": "command",
                "method": "POST",
                "path": "/api/accounts",
                "moduleRefs": ["module.account-service"],
                "entityRefs": ["entity.account"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "requestSchema": [{
                    "fieldId": "field.request.investorName",
                    "name": "investorName",
                    "type": "string",
                    "required": true
                }],
                "responseSchema": [{
                    "fieldId": "field.response.accountId",
                    "name": "accountId",
                    "type": "string",
                    "required": true
                }],
                "errorSchema": [{
                    "fieldId": "field.error.message",
                    "name": "message",
                    "type": "string",
                    "required": true
                }]
            }]);
        }
        "behavior" => {
            candidate["content"]["userFlows"] = json!([{
                "flowId": "flow.account-lifecycle",
                "name": "Account lifecycle",
                "kind": "user_interaction",
                "moduleRefs": ["module.account-service"],
                "interfaceRefs": ["api.account.open"],
                "entityRefs": ["entity.account"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "entry": {
                    "type": "frontend_action",
                    "ref": "action_open_account"
                },
                "steps": [{
                    "stepId": "step.submit-open-account",
                    "actor": "工作人员",
                    "action": "提交开户表单",
                    "interfaceRefs": ["api.account.open"],
                    "stateMachineRefs": ["machine.account-status"]
                }],
                "outcomes": [{
                    "type": "success",
                    "description": "返回开户成功反馈并刷新账户列表。"
                }]
            }]);
            candidate["content"]["stateMachines"] = json!([{
                "machineId": "machine.account-status",
                "name": "Account status",
                "entityRef": "entity.account",
                "entityRefs": ["entity.account"],
                "moduleRefs": ["module.account-service"],
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "states": [{"stateId": "active", "name": "正常", "terminal": false}],
                "transitions": [],
                "rules": []
            }]);
        }
        "frontend_experience" => {
            let refs = candidate["content"]["frontendExperience"]["sourceRefs"].clone();
            let ui_quality_contract =
                candidate["content"]["frontendExperience"]["uiQualityContract"].clone();
            candidate["content"]["frontendExperience"] = json!({
                "required": true,
                "kind": "staff_console",
                "experienceLevel": "usable_internal_product",
                "surfaces": [{
                    "surfaceId": "surface_account_admin",
                    "name": "证券账户管理",
                    "workflowRefs": ["flow.account-lifecycle"],
                    "audienceRefs": ["audience_staff"]
                }],
                "dataViews": [{
                    "viewId": "view_account_list",
                    "name": "证券账户列表"
                }],
                "actions": [{
                    "actionId": "action_open_account",
                    "label": "新建证券账户"
                }],
                "operationPaths": [{
                    "pathId": "path_account_lifecycle",
                    "name": "证券账户生命周期办理",
                    "surfaceRef": "surface_account_admin",
                    "workflowRef": "flow.account-lifecycle",
                    "dataViewRefs": ["view_account_list"],
                    "actionRefs": ["action_open_account"]
                }],
                "uiQualityContract": ui_quality_contract,
                "sourceRefs": refs
            });
        }
        _ => {}
    }
    candidate
}

fn architecture_section_candidate_without_interfaces_json(
    fixture: &Fixture,
    request_ref: &str,
) -> Value {
    let mut candidate = architecture_section_candidate_json(fixture, request_ref);
    match candidate["section"].as_str().unwrap_or_default() {
        "domain_contract" => {
            candidate["content"]["interfaces"] = json!([]);
        }
        "coverage" => {
            candidate["content"]["architectureQuality"]["risks"][0]["ownerArtifactRefs"]
                ["interfaces"] = json!([]);
        }
        _ => {}
    }
    candidate
}

fn architecture_section_candidate_with_workflow_closure_no_runtime_json(
    fixture: &Fixture,
    request_ref: &str,
) -> Value {
    let mut candidate =
        architecture_section_candidate_with_workflow_closure_json(fixture, request_ref);
    if candidate["section"].as_str() == Some("runtime_delivery") {
        candidate["content"]["runtimeDelivery"] = json!({
            "status": "not_applicable",
            "reason": "This test focuses on frontend workflow closure review signals."
        });
    }
    candidate
}

fn read_request_root_value(project_root: &str, request_ref: &str) -> Value {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let index = state::request_index::get_request_index_entry(project_root, request_id)
        .expect("request index entry");
    let request_path = std::path::Path::new(project_root).join(index.request_file);
    serde_json::from_str(&std::fs::read_to_string(request_path).expect("read request file"))
        .expect("parse request file")
}

fn read_group_fields_from_json(group: &Value) -> Vec<String> {
    let selectors =
        serde_json::from_value::<Vec<delivery_core::ReadSelector>>(group["selectors"].clone())
            .expect("read group selectors");
    delivery_core::expand_read_selectors(&selectors)
}

fn remove_task_result_optional_validation_fields(fixture: &Fixture, request_ref: &str) {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let index = state::request_index::get_request_index_entry(fixture.root_str(), request_id)
        .expect("request index entry");
    let request_path = fixture.root.join(index.request_file);
    let mut root: Value =
        serde_json::from_str(&std::fs::read_to_string(&request_path).expect("read request file"))
            .expect("parse request file");
    let groups = root["requestReadPlan"]["groups"]
        .as_array_mut()
        .expect("read plan groups");
    for group in groups {
        let mut fields = read_group_fields_from_json(group);
        fields.retain(|field| {
            !matches!(
                field.as_str(),
                "task.conceptRefs" | "outputContract.blockedReasonOptions"
            )
        });
        group["selectors"] = delivery_core::read_selectors_value_from_paths(fields);
    }
    write_json_atomic(&request_path, &root).expect("write request with trimmed selectors");
}

fn assert_no_root_submit_metadata(root: &Value) {
    for key in [
        "artifactKind",
        "submitTool",
        "writeTargets",
        "writeMode",
        "outputContract",
    ] {
        assert!(
            root.get(key).is_none(),
            "compact request root must not duplicate {key}: {root:#}"
        );
    }
}

fn assert_no_read_plan_size_warnings(fixture: &Fixture) {
    let audit_path = fixture.root.join(".loom/metrics/request-size-audit.jsonl");
    let audit = std::fs::read_to_string(&audit_path).expect("read request size audit");
    for line in audit.lines() {
        let entry: Value = serde_json::from_str(line).expect("parse request size audit entry");
        let warnings = entry["readPlanWarnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            warnings.is_empty(),
            "generated request should not produce read-plan size warnings: {entry:#}"
        );
    }
}

fn architecture_section_contract(fixture: &Fixture, request_ref: &str, section: &str) -> Value {
    private_architecture_section_outputs(fixture, request_ref)
        .into_iter()
        .find(|output| output["section"] == json!(section))
        .unwrap_or_else(|| panic!("{section} section contract"))
}

fn private_architecture_section_outputs(fixture: &Fixture, request_ref: &str) -> Vec<Value> {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let relative =
        state::request_manifest::request_storage_ref(&fixture.root, request_id, "sectionOutputs")
            .expect("read private sectionOutputs ref")
            .expect("private sectionOutputs ref");
    let path = fixture.root.join(relative);
    serde_json::from_str(&std::fs::read_to_string(path).expect("read private sectionOutputs"))
        .expect("parse private sectionOutputs")
}

fn private_output_contract(fixture: &Fixture, request_ref: &str) -> Value {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    let relative =
        state::request_manifest::request_storage_ref(&fixture.root, request_id, "outputContract")
            .expect("read private outputContract ref")
            .expect("private outputContract ref");
    let path = fixture.root.join(relative);
    serde_json::from_str(&std::fs::read_to_string(path).expect("read private outputContract"))
        .expect("parse private outputContract")
}

fn latest_ref_for_phase(project_root: &str, delivery_id: &str, key: &str) -> String {
    let index_path = std::path::Path::new(project_root)
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    index["phases"][0]["latestRefs"][key]
        .as_str()
        .expect("latest ref")
        .to_string()
}

fn active_phase_id(project_root: &str, delivery_id: &str) -> String {
    let index_path = std::path::Path::new(project_root)
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(index_path).expect("read index"))
            .expect("parse index");
    index["activePhaseId"]
        .as_str()
        .expect("active phase id")
        .to_string()
}

fn append_done_phase(fixture: &Fixture, delivery_id: &str, phase_id: &str) {
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    index["phases"].as_array_mut().expect("phases").push(json!({
        "phaseId": phase_id,
        "latestRefs": {},
        "nextAction": {
            "kind": "done",
            "source": "test",
            "reason": "phase_done"
        }
    }));
    write_json_atomic(&index_path, &index).expect("write delivery index");
}

fn append_unstarted_phase(fixture: &Fixture, delivery_id: &str, phase_id: &str) {
    append_phase_with_refs(fixture, delivery_id, phase_id, json!({}));
}

fn append_phase_with_refs(
    fixture: &Fixture,
    delivery_id: &str,
    phase_id: &str,
    latest_refs: Value,
) {
    let index_path = fixture
        .root
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("index.json");
    let mut index: Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).expect("read index"))
            .expect("parse index");
    index["phases"].as_array_mut().expect("phases").push(json!({
        "phaseId": phase_id,
        "latestRefs": latest_refs
    }));
    write_json_atomic(&index_path, &index).expect("write delivery index");
}

fn request_delivery_id(project_root: &str, request_ref: &str) -> String {
    let request_id = request_ref
        .split("/requests/")
        .nth(1)
        .expect("request id in ref");
    state::request_index::get_request_index_entry(project_root, request_id)
        .expect("request index entry")
        .delivery_id
        .expect("delivery id")
}

fn technical_baseline_candidate_json(project_kind: &str, approval_type: &str) -> Value {
    json!({
        "status": "confirmed",
        "source": if project_kind == "existing_project" {
            "detected_from_repo"
        } else {
            "agent_recommended_for_new_project"
        },
        "projectKind": project_kind,
        "scope": "project",
        "stack": {
            "frontend": "vite-react",
            "backend": "spring-boot",
            "database": "postgres"
        },
        "constraints": [],
        "evidence": [{
            "path": if project_kind == "existing_project" { json!("package.json") } else { Value::Null },
            "reason": "Repository signals support the selected stack."
        }],
        "approval": {
            "type": approval_type,
            "reason": "Current repository stack is stable."
        },
        "confidence": "high",
        "requiresUserConfirmation": false,
        "reasoningSummary": [
            "The current phase should preserve a stable application stack."
        ],
        "alternatives": []
    })
}

fn new_project_technical_baseline_candidate_json() -> Value {
    json!({
        "status": "confirmed",
        "source": "agent_recommended_for_new_project",
        "projectKind": "new_project",
        "scope": "project",
        "stack": {
            "tracks": {
                "web": {
                    "status": "selected",
                    "selection": "React + Vite + TypeScript",
                    "source": "user_confirmed",
                    "rationale": "Matches the confirmed staff-facing web workflow."
                },
                "app": {
                    "status": "not_applicable",
                    "selection": "No native/mobile app in this phase",
                    "source": "requirement_scope",
                    "rationale": "The confirmed phase only needs a staff web surface."
                },
                "backend": {
                    "status": "selected",
                    "selection": "Java + Spring Boot",
                    "source": "user_confirmed",
                    "rationale": "Supports the required account workflow APIs."
                },
                "persistence": {
                    "status": "selected",
                    "selection": "SQLite",
                    "source": "user_confirmed",
                    "rationale": "Enough for local phase verification."
                },
                "dataAccess": {
                    "status": "selected",
                    "selection": "Spring Data JPA",
                    "source": "user_confirmed",
                    "rationale": "Aligns with Spring Boot persistence."
                },
                "externalServices": {
                    "status": "not_needed",
                    "selection": "No external services in this phase",
                    "source": "requirement_scope",
                    "rationale": "The phase can run locally without third-party integration."
                }
            }
        },
        "constraints": [],
        "evidence": [{
            "path": Value::Null,
            "reason": "New-project stack was selected from the confirmed phase scope."
        }],
        "approval": {
            "type": "user_confirmed",
            "confirmedAt": "2026-06-24T10:30:00+08:00",
            "reason": "User confirmed the final technology baseline."
        },
        "confidence": "high",
        "requiresUserConfirmation": false,
        "reasoningSummary": [
            "The selected stack supports the confirmed new-project phase without adding extra surfaces."
        ],
        "alternatives": []
    })
}

fn write_previous_technical_baseline(fixture: &Fixture, delivery_id: &str) {
    let baseline_path = fixture
        .root
        .join(".loom/deliveries")
        .join(delivery_id)
        .join("contracts/technical-baseline.json");
    std::fs::create_dir_all(baseline_path.parent().expect("baseline parent"))
        .expect("create baseline parent");
    write_json_atomic(
        &baseline_path,
        &json!({
            "schemaVersion": "1.0",
            "technicalBaselineId": "tb_previous",
            "deliveryId": delivery_id,
            "phaseId": "phase-1",
            "status": "confirmed",
            "source": "user_confirmed",
            "projectKind": "existing_project",
            "scope": "project",
            "stack": {
                "frontend": "plain-html",
                "backend": "none"
            },
            "constraints": [],
            "evidence": [{
                "path": "README.md",
                "reason": "Previous baseline was confirmed earlier."
            }],
            "approval": {
                "type": "user_confirmed",
                "reason": "Existing baseline fixture."
            },
            "confidence": "high",
            "requiresUserConfirmation": false,
            "reasoningSummary": ["Previous baseline fixture."],
            "alternatives": [],
            "createdAt": "2026-06-24T09:00:00+08:00",
            "updatedAt": "2026-06-24T09:00:00+08:00"
        }),
    )
    .expect("write previous technical baseline");
}

fn repository_context_candidate_json(
    request_ref: &str,
    brainstorm_contract_ref: &str,
    technical_baseline_ref: &str,
) -> Value {
    json!({
        "status": "ready",
        "source": {
            "requestRef": request_ref,
            "brainstormContractRef": brainstorm_contract_ref,
            "technicalBaselineRef": technical_baseline_ref
        },
        "requestLens": {
            "projectKind": "existing_project",
            "baselineProjectKind": "existing_project",
            "repositoryMode": "existing_project",
            "phaseDevelopmentMode": "initial_delivery",
            "scanPurpose": "phase_start_repository_snapshot",
            "primaryConsumer": "phase_brainstorm",
            "laterConsumers": ["PGC", "AAC", "TaskPlan"]
        },
        "repoOverview": {
            "summary": "Repository contains an existing frontend application entrypoint.",
            "repositoryShape": "single_package",
            "primaryApplications": [{
                "applicationId": "app_web",
                "name": "web",
                "kind": "frontend",
                "rootPath": "src"
            }]
        },
        "technologySignals": {
            "primaryLanguages": ["typescript"],
            "frameworks": ["react"],
            "packageManagers": ["npm"],
            "buildCommands": ["npm run build"],
            "testCommands": ["npm test"],
            "notes": []
        },
        "structureSignals": {
            "rootPaths": [{
                "path": "src",
                "role": "application_root"
            }],
            "entryPoints": [{
                "path": "src/main.tsx",
                "kind": "entrypoint",
                "description": "frontend entrypoint"
            }],
            "configurationFiles": ["package.json"]
        },
        "existingCapabilities": [{
            "capabilityId": "capability_ui_root",
            "name": "frontend entrypoint",
            "status": "partial",
            "summary": "The repository already has a frontend entrypoint.",
            "surfaceRefs": ["surface_ui_root"],
            "confidence": "high",
            "deliveryRelevance": "Can be extended for the current phase."
        }],
        "relevantSurfaces": [{
            "surfaceId": "surface_ui_root",
            "kind": "ui",
            "path": "src/main.tsx",
            "summary": "Existing frontend entrypoint.",
            "relevance": "extension_point",
            "suggestedUse": "inspect_or_extend"
        }],
        "recommendedReadRefs": [{
            "path": "src/main.tsx",
            "reason": "implemented_capability",
            "priority": "high",
            "summary": "Inspect the existing entrypoint before modifying UI flow.",
            "surfaceRefs": ["surface_ui_root"]
        }],
        "contextQuality": {
            "coverage": "focused",
            "confidence": "high",
            "warnings": []
        },
        "warnings": []
    })
}

fn valid_candidate_json() -> Value {
    json!({
        "requestSummary": {
            "title": "证券账户开户流程",
            "oneLine": "实现证券账户开户闭环",
            "businessGoal": "完成第一阶段证券账户业务闭环",
            "complexity": "medium"
        },
        "scope": {
            "included": [{
                "id": "scope_1",
                "label": "证券账户开户",
                "items": ["个人开户", "法人开户"],
                "reason": "当前阶段先完成证券账户能力闭环",
                "source": "user_confirmed"
            }],
            "excluded": [],
            "deferred": [],
            "assumptions": []
        },
        "roadmap": {
            "required": false,
            "currentPhaseId": "phase-1",
            "phases": [{
                "phaseId": "phase-1",
                "title": "证券账户开户流程",
                "name": "证券账户开户流程",
                "status": "scope_confirmed",
                "goal": "实现证券账户开户闭环",
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "dependsOn": []
            }]
        },
        "phasePlan": {
            "current": {
                "phaseId": "phase-1",
                "title": "证券账户开户流程",
                "goal": "实现证券账户开户闭环",
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "status": "scope_confirmed"
            },
            "nextPhasePreview": {
                "kind": "none",
                "reason": "下一阶段在当前提交之外处理。"
            }
        },
        "acceptance": [{
            "id": "acc_1",
            "statement": "工作人员可以完成证券账户开户并得到成功反馈。",
            "capabilityRefs": [],
            "sourceRefs": [],
            "priority": "must"
        }],
        "userConfirmation": {
            "confirmed": true,
            "confirmedAt": "2026-06-24T10:00:00+08:00",
            "confirmationSummary": "用户已确认阶段范围、业务规则和提交前核对。",
            "confirmationBasis": {
                "initialRequestOnly": false,
                "summaryPresentedToUser": true,
                "confirmedAfterSummary": true,
                "presentedItems": [
                    "phase_scope",
                    "concept_grounding",
                    "final_summary"
                ]
            }
        },
        "conceptGrounding": {
            "phaseConceptGrounding": {
                "mode": "none_required",
                "reason": "当前范围不需要额外术语表才能继续。",
                "concepts": []
            },
            "glossaryUpdates": []
        },
        "conceptConfirmation": {
            "shownToUser": true,
            "confirmedConceptRefs": [],
            "confirmationSummary": "用户已确认当前阶段的业务理解。"
        },
        "clarificationProgress": {
            "mode": "progressive_blocks",
            "confirmedBlocks": [
                {
                    "block": "phase_scope",
                    "summary": "已确认当前阶段范围。",
                    "confirmedByUser": true
                },
                {
                    "block": "concept_grounding",
                    "summary": "已确认业务规则和边界。",
                    "confirmedByUser": true
                }
            ],
            "skippedBlocks": [
                {
                    "block": "frontend_experience",
                    "reason": "本用例只验证提交链路，不要求页面路径细节。"
                }
            ],
            "finalSummaryConfirmed": true
        }
    })
}

fn candidate_with_next_phase_preview() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["scope"]["deferred"] = json!([{
        "id": "deferred_1",
        "label": "资金账户基础能力",
        "items": ["资金账户开户", "密码管理", "存款与取款", "证券账户与资金账户关联"],
        "reason": "资金账户依赖已完成的证券账户主数据，适合作为下一阶段。",
        "source": "user_confirmed"
    }]);
    candidate["roadmap"]["required"] = json!(true);
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "candidate",
        "suggestedPhaseId": "phase-next",
        "title": "资金账户基础能力",
        "goal": "在证券账户基础上实现资金账户开户、密码、存取款与账户关联。",
        "scopePreview": ["资金账户开户", "密码管理", "存款与取款", "账户关联"],
        "reason": "资金账户是交易客户端和撮合前的下一层依赖。"
    });
    candidate
}

fn phase2_candidate_json() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["requestSummary"]["title"] = json!("资金账户基础能力");
    candidate["requestSummary"]["oneLine"] = json!("实现资金账户开户、密码、存取款与账户关联");
    candidate["requestSummary"]["businessGoal"] = json!("承接证券账户闭环，完成资金账户基础能力");
    candidate["scope"]["included"][0]["label"] = json!("资金账户基础能力");
    candidate["scope"]["included"][0]["items"] = json!([
        "资金账户开户",
        "密码管理",
        "存款与取款",
        "证券账户与资金账户关联"
    ]);
    candidate["scope"]["included"][0]["reason"] =
        json!("phase2 承接已完成的证券账户底座，补齐交易前置的资金账户能力。");
    candidate["roadmap"]["currentPhaseId"] = json!("phase-2");
    candidate["roadmap"]["phases"][0]["phaseId"] = json!("phase-2");
    candidate["roadmap"]["phases"][0]["title"] = json!("资金账户基础能力");
    candidate["roadmap"]["phases"][0]["name"] = json!("资金账户基础能力");
    candidate["roadmap"]["phases"][0]["goal"] = json!("实现资金账户开户、密码、存取款与账户关联。");
    candidate["phasePlan"]["current"]["phaseId"] = json!("phase-2");
    candidate["phasePlan"]["current"]["title"] = json!("资金账户基础能力");
    candidate["phasePlan"]["current"]["goal"] = json!("实现资金账户开户、密码、存取款与账户关联。");
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "none",
        "reason": "phase2 之后的范围由后续阶段确认。"
    });
    candidate["acceptance"][0]["statement"] =
        json!("工作人员可以完成资金账户开户、密码管理、存取款和证券账户关联。");
    candidate
}

fn valid_candidate_with_frontend_json() -> Value {
    let mut candidate = valid_candidate_json();
    candidate["frontendExperience"] = json!({
        "required": true,
        "kind": "staff_console",
        "experienceLevel": "usable_internal_product",
        "audiences": [{
            "audienceId": "audience_staff",
            "name": "工作人员",
            "primaryJobs": ["办理证券账户业务"]
        }],
        "surfaces": [{
            "surfaceId": "surface_account_admin",
            "name": "证券账户管理",
            "audienceRefs": ["audience_staff"],
            "primaryJobs": ["开户", "挂失补办", "销户"]
        }],
        "dataViews": [{
            "viewId": "view_account_list",
            "name": "证券账户列表",
            "purpose": "查询并选择证券账户办理后续动作",
            "targetObject": "证券账户",
            "selectionMode": "query_and_select",
            "paginationRequired": true,
            "defaultLoadsFirstPage": true,
            "searchCriteria": [{
                "criterionId": "criterion_account_no",
                "label": "证券账户号",
                "fieldRef": "securityAccountNo",
                "reason": "工作人员按账户号定位办理对象",
                "sourceRefs": []
            }],
            "sourceRefs": []
        }],
        "actions": [{
            "actionId": "action_open_account",
            "label": "新建证券账户",
            "targetObject": "证券账户",
            "entryPoint": "navigation_entry",
            "inputFields": ["个人开户信息", "法人开户信息"],
            "resultObservation": ["response_message", "list_refresh"],
            "refreshPolicy": "办理完成后返回列表并刷新状态",
            "successFeedback": ["开户成功并显示证券账户号"],
            "blockingOrErrorFeedback": ["开户资格不满足时显示中文阻断原因"],
            "sourceRefs": []
        }],
        "operationPaths": [{
            "pathId": "path_account_lifecycle",
            "name": "证券账户生命周期办理",
            "userGoal": "工作人员完成证券账户开户、挂失补办和销户办理",
            "surfaceRef": "surface_account_admin",
            "targetObject": "证券账户",
            "selectionMode": "query_and_select",
            "selectionSummary": "开户从新建入口进入；挂失补办和销户先查询列表并选择目标账户",
            "dataViewRefs": ["view_account_list"],
            "actionRefs": ["action_open_account"],
            "requiredStates": ["success", "business_blocking", "error"],
            "sourceRefs": []
        }],
        "mustNot": ["不能只靠内部主键触发挂失补办或销户"],
        "confirmationSummary": "已确认工作人员后台证券账户管理页面路径。"
    });
    candidate["clarificationProgress"]["confirmedBlocks"] = json!([
        {
            "block": "phase_scope",
            "summary": "已确认当前阶段范围。",
            "confirmedByUser": true
        },
        {
            "block": "concept_grounding",
            "summary": "已确认业务规则和边界。",
            "confirmedByUser": true
        },
        {
            "block": "frontend_experience",
            "summary": "已确认页面办理路径。",
            "confirmedByUser": true
        }
    ]);
    candidate["clarificationProgress"]["skippedBlocks"] = json!([]);
    candidate["userConfirmation"]["confirmationBasis"]["presentedItems"] = json!([
        "phase_scope",
        "concept_grounding",
        "frontend_experience",
        "final_summary"
    ]);
    candidate
}

fn candidate_with_planning_details_json() -> Value {
    let mut candidate = valid_candidate_with_frontend_json();
    candidate["scope"]["deferred"] = json!([{
        "id": "deferred_1",
        "label": "资金账户",
        "items": ["资金账户开户留到后续阶段"],
        "reason": "当前阶段只做证券账户闭环",
        "source": "user_confirmed"
    }]);
    candidate["scope"]["excluded"] = json!([{
        "id": "excluded_1",
        "label": "中央撮合",
        "items": ["撮合成交不在当前阶段实现"],
        "reason": "避免把交易核心提前塞入账户阶段",
        "source": "user_confirmed"
    }]);
    candidate["phasePlan"]["nextPhasePreview"] = json!({
        "kind": "candidate",
        "suggestedPhaseId": "phase-next",
        "title": "资金账户基础能力",
        "goal": "在证券账户基础上实现资金账户开户、密码、存取款与账户关联。",
        "scopePreview": ["资金账户开户", "密码管理", "存款与取款", "账户关联"],
        "reason": "资金账户是交易客户端和撮合前的下一层依赖。"
    });
    candidate["acceptance"][0]["capabilityRefs"] = json!(["cap_account_opening"]);
    candidate["scope"]["assumptions"] = json!([{
        "id": "assumption_1",
        "text": "开户前工作人员已完成必要身份材料核验。",
        "requiresConfirmation": false
    }]);
    candidate["domainModel"] = json!({
        "actors": [{
            "id": "actor_staff",
            "name": "工作人员",
            "description": "办理证券账户开户、挂失补办和销户。"
        }],
        "capabilityGroups": [{
            "id": "cap_account_opening",
            "name": "证券账户开户",
            "description": "个人与法人证券账户开户能力。"
        }],
        "businessFlows": [{
            "id": "flow_account_opening",
            "name": "证券账户开户流程",
            "actors": ["actor_staff"],
            "capabilityRefs": ["cap_account_opening"],
            "summary": "工作人员录入开户材料，系统校验资格并创建证券账户后给出中文成功反馈。"
        }]
    });
    candidate["conceptGrounding"]["phaseConceptGrounding"] = json!({
        "mode": "concepts_present",
        "reason": "证券账户与资金账户容易混淆，需要在规划合同中保留语义边界。",
        "concepts": [{
            "conceptId": "concept_security_account",
            "term": "证券账户",
            "normalizedName": "证券账户",
            "explanation": "证券账户用于登记证券持有与交易资格，不等同于资金账户。",
            "mustNotMisinterpretAs": ["资金账户"],
            "phaseRelevance": "current",
            "priority": "must_understand",
            "attentionRank": 1,
            "riskFactors": ["scope_confusion_risk"],
            "scopeRefs": ["scope_1"],
            "acceptanceRefs": ["acc_1"],
            "humanReadableReason": "当前阶段只实现证券账户闭环。"
        }]
    });
    candidate
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-submit-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            _guard: guard,
        }
    }

    fn root_str(&self) -> &str {
        self.root.to_str().expect("fixture path utf8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
