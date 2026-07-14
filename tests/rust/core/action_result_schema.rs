use delivery_core::{
    ActiveOperationRef, ArtifactKind, DeployRepairAssetsNext, ExecuteEditBoundary, ExecuteTaskNext,
    ExecuteVerificationPolicy, ExecutionKind, GenerateKnowledgeSemanticsNext, KnowledgeReadMode,
    LoomMcpActionResult, LoomMcpActiveOperationResult, LoomMcpAutoRunnableResult,
    LoomMcpBlockedResult, LoomMcpDoneResult, LoomMcpFailure, LoomMcpFailureResult,
    LoomMcpNextAction, LoomMcpRepairableErrorResult, LoomMcpUserGateResult, PostSubmitAction,
    ReadGroupRef, RepairIssue, RunLoomToolNext, WriteArtifactNext, WriteMode, WriteTarget,
};
use serde_json::Value;

const FORBIDDEN_KEYS: &[&str] = &[
    "commandInvocation",
    "argv",
    "argvTemplate",
    "submitCommand",
    "retryCommand",
    "command",
    "launcher",
    "env",
    "LOOM_AGENT_PROFILE",
    "actionRequired",
    "CliEnvelope",
    "readCommand",
    "fallbackRule",
];

#[test]
fn action_result_states_have_expected_next_boundaries() {
    for result in sample_results() {
        let value = serde_json::to_value(&result).expect("result serializes");
        assert_no_forbidden_keys(&value);
        assert!(
            !value.to_string().contains(".refs"),
            "result must not expose .refs paths: {value}"
        );

        match value["state"].as_str().expect("state") {
            "auto_runnable" => {
                assert_eq!(value["stopAllowed"], false);
                assert!(
                    value.get("continuationPolicy").is_none(),
                    "auto_runnable must keep continuation semantics in existing state/stopAllowed/instruction fields: {value}"
                );
                assert!(
                    value["agentInstruction"].as_str().is_some_and(|text| {
                        text.contains("Continue immediately")
                            && (text.contains("submit") || text.contains("retryTool"))
                            && text.contains("Do not stop at a progress recap")
                    }),
                    "auto_runnable must include agentInstruction: {value}"
                );
                assert!(
                    value.get("next").is_some(),
                    "auto_runnable must include next"
                );
            }
            "user_gate" | "done" | "blocked" | "repairable_error" | "failed" => {
                assert!(
                    value.get("next").is_none(),
                    "{} must not include next",
                    value["state"]
                );
            }
            "active_operation" => {
                assert!(value.get("next").is_none());
                assert!(value.get("allowedObservationTools").is_some());
            }
            state => panic!("unexpected state {state}"),
        }
    }
}

#[test]
fn repairable_error_contains_resubmit_contract() {
    let result = LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
        project_root: "/tmp/project".to_string(),
        target_file: ".loom/agent-writable/result.json".to_string(),
        issues: vec![RepairIssue {
            code: "missing_required_field".to_string(),
            message: "field is required".to_string(),
            target_id: Some("candidate".to_string()),
            field_path: Some("summary".to_string()),
        }],
        resubmit_tool: "loom.repairSubmitFile".to_string(),
        fix_scope: Some("Only edit the target file.".to_string()),
        target_ids: vec!["candidate".to_string()],
        read_groups: vec![],
    });
    let value = serde_json::to_value(result).expect("result json");
    assert_eq!(value["targetFile"], ".loom/agent-writable/result.json");
    assert_eq!(value["resubmitTool"], "loom.repairSubmitFile");
    assert_eq!(value["issues"][0]["code"], "missing_required_field");
}

#[test]
fn next_action_shapes_are_stable() {
    let actions = vec![
        sample_write_artifact_next(),
        sample_execute_task_next(),
        sample_run_loom_tool_next(),
        sample_generate_knowledge_semantics_next(),
        sample_deploy_repair_assets_next(),
    ];

    let values: Vec<Value> = actions
        .into_iter()
        .map(|action| serde_json::to_value(action).expect("action json"))
        .collect();
    assert_eq!(
        values
            .iter()
            .map(|value| value["kind"].as_str().expect("kind"))
            .collect::<Vec<_>>(),
        vec![
            "write_artifact",
            "execute_task",
            "run_loom_tool",
            "generate_knowledge_semantics",
            "deploy_repair_assets",
        ]
    );
    for value in values {
        assert_no_forbidden_keys(&value);
    }
}

#[test]
fn execute_task_auto_runnable_instruction_forbids_progress_only_stop() {
    let result = LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
        "/tmp/project",
        sample_execute_task_next(),
    ));
    let value = serde_json::to_value(result).expect("result json");

    assert_eq!(value["state"], "auto_runnable");
    assert_eq!(value["stopAllowed"], false);
    assert!(value.get("continuationPolicy").is_none());
    let instruction = value["agentInstruction"]
        .as_str()
        .expect("agentInstruction");
    assert!(instruction.contains("execute only this task"));
    assert!(instruction.contains("write resultFile"));
    assert!(instruction.contains("submit with submitTool"));
    assert!(instruction.contains("Do not stop at a progress recap"));
    assert!(instruction.contains("Do not mark the workflow complete"));
    assert!(instruction.contains("send a final answer"));
    assert!(instruction.contains("ask the user whether to continue"));
    assert!(instruction.contains("TaskResult submit succeeds"));
    assert_eq!(value["next"]["kind"], "execute_task");
}

fn sample_results() -> Vec<LoomMcpActionResult> {
    vec![
        LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
            "/tmp/project",
            sample_write_artifact_next(),
        )),
        LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
            "/tmp/project",
            sample_run_loom_tool_next(),
        )),
        LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
            project_root: "/tmp/project".to_string(),
            prompt: "Confirm scope.".to_string(),
            accepted_responses: vec!["confirm".to_string()],
            request_ref: None,
            delivery_id: None,
            phase_id: None,
            gate: None,
        }),
        LoomMcpActionResult::ActiveOperation(LoomMcpActiveOperationResult {
            project_root: "/tmp/project".to_string(),
            operation: ActiveOperationRef {
                operation_id: "op_1".to_string(),
                operation_type: "deploy_run".to_string(),
                delivery_id: Some("delivery_1".to_string()),
                phase_id: Some("phase_1".to_string()),
                started_at: "2026-06-23T00:00:00Z".to_string(),
                expires_at: "2026-06-23T00:10:00Z".to_string(),
            },
            allowed_observation_tools: vec!["loom.status".to_string()],
            observation_policy: None,
            forbidden_actions: vec![],
            progress_summary: None,
        }),
        LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: "/tmp/project".to_string(),
            summary: "Done.".to_string(),
            details: None,
            warnings: vec![],
        }),
        LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: "/tmp/project".to_string(),
            blockers: vec!["Need user input.".to_string()],
            recommended_tool: Some("loom.plan".to_string()),
            details: None,
        }),
        LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
            project_root: "/tmp/project".to_string(),
            target_file: ".loom/agent-writable/result.json".to_string(),
            target_ids: vec![],
            issues: vec![RepairIssue {
                code: "invalid_shape".to_string(),
                message: "Invalid shape.".to_string(),
                target_id: None,
                field_path: None,
            }],
            resubmit_tool: "loom.repairSubmitFile".to_string(),
            fix_scope: None,
            read_groups: vec![],
        }),
        LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: "/tmp/project".to_string(),
            error: LoomMcpFailure {
                code: "not_implemented_for_batch".to_string(),
                message: "Not implemented.".to_string(),
                target_batch: Some(4),
                domain: Some("planning".to_string()),
                route_action: None,
                recovery_tool: None,
            },
        }),
    ]
}

fn sample_write_artifact_next() -> LoomMcpNextAction {
    LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
        artifact_kind: ArtifactKind::BrainstormCandidate,
        request_ref: "loom://projects/project_1/requests/request_1".to_string(),
        write_mode: WriteMode::CreateOrReplace,
        write_targets: vec![WriteTarget {
            target_id: "candidate".to_string(),
            path: ".loom/agent-writable/candidate.json".to_string(),
            required: true,
            description: "Write candidate JSON.".to_string(),
        }],
        read_groups: vec![ReadGroupRef::new(
            "main",
            1,
            vec!["requirement.text".to_string()],
            "loom://projects/project_1/requests/request_1/field-groups/main",
        )],
        submit_tool: "loom.brainstormAcceptFile".to_string(),
    })
}

fn sample_execute_task_next() -> LoomMcpNextAction {
    LoomMcpNextAction::ExecuteTask(ExecuteTaskNext {
        execution_kind: ExecutionKind::PlannedTask,
        repair_origin: None,
        request_ref: "loom://projects/project_1/requests/request_2".to_string(),
        result_file: ".loom/agent-writable/task-result.json".to_string(),
        task_id: "task_1".to_string(),
        group_id: Some("group_1".to_string()),
        read_groups: vec![],
        submit_tool: "loom.recordTaskResultFile".to_string(),
        edit_boundary: ExecuteEditBoundary {
            allowed_paths: vec!["src".to_string()],
            protected_paths: vec![".loom".to_string()],
        },
        verification_policy: ExecuteVerificationPolicy {
            required_commands: vec!["npm test".to_string()],
            evidence_required: true,
        },
        repair_context: None,
        post_submit: PostSubmitAction::ContinueDelivery,
    })
}

fn sample_run_loom_tool_next() -> LoomMcpNextAction {
    LoomMcpNextAction::RunLoomTool(RunLoomToolNext {
        tool_name: "loom.knowledgeBrainstormContext".to_string(),
        request_ref: "loom://projects/project_1/requests/request_knowledge".to_string(),
        read_groups: vec![ReadGroupRef::new(
            "knowledge_context_plan",
            1,
            vec!["knowledgeQueryPlan.blocks.phase_scope.executionOrder".to_string()],
            "loom://projects/project_1/requests/request_knowledge/field-groups/knowledge_context_plan",
        )],
        retry_tool: "loom.brainstormConfirmBlock".to_string(),
    })
}

fn sample_generate_knowledge_semantics_next() -> LoomMcpNextAction {
    LoomMcpNextAction::GenerateKnowledgeSemantics(GenerateKnowledgeSemanticsNext {
        source_name: "domain".to_string(),
        source_id: "ksrc_1".to_string(),
        build_id: "kbld_1".to_string(),
        pack_id: "kpack_1".to_string(),
        pack_index: 1,
        pack_count: 1,
        request_ref: "loom://projects/project_1/requests/request_3".to_string(),
        result_file: ".loom/agent-writable/semantic-result.json".to_string(),
        output_contract: serde_json::json!({
            "resultTemplate": { "buildId": "kbld_1", "packId": "kpack_1", "chunkResults": [] }
        }),
        generation_rules: serde_json::json!({
            "summaryLanguage": "preserve_source_language"
        }),
        read_mode: KnowledgeReadMode::ChunkInspect,
        chunk_read_plan: vec![delivery_core::KnowledgeChunkReadRef {
            source_name: "domain".to_string(),
            source_id: "ksrc_1".to_string(),
            build_id: "kbld_1".to_string(),
            chunk_id: "kchunk_000001".to_string(),
            document_title: "doc".to_string(),
            heading_path: vec![],
            token_estimate: 100,
            summary_language: "zh-CN".to_string(),
            read_tool: "loom.knowledgeInspectChunk".to_string(),
            resource_uri: "loom://knowledge/ksrc_1/builds/kbld_1/chunks/kchunk_000001".to_string(),
        }],
        submit_tool: "loom.knowledgeSemanticSubmitFile".to_string(),
    })
}

fn sample_deploy_repair_assets_next() -> LoomMcpNextAction {
    LoomMcpNextAction::DeployRepairAssets(DeployRepairAssetsNext {
        repair_id: "drepair_1".to_string(),
        failure_kind: "runtime".to_string(),
        failure_owner: "deploy".to_string(),
        repair_route: "asset_repair".to_string(),
        primary_reason: "nginx proxy route is missing".to_string(),
        diagnostics: vec![delivery_core::DeploymentFailureDiagnostic {
            code: "api_route_not_verified".to_string(),
            severity: "error".to_string(),
            message: "API route did not proxy to the backend service.".to_string(),
            evidence: vec!["GET /api/health returned HTML fallback".to_string()],
            suggested_action: "Repair generated nginx proxy route before retrying.".to_string(),
        }],
        suggested_actions: vec!["Repair generated deployment assets.".to_string()],
        editable_files: vec!["deploy/nginx.conf".to_string()],
        protected_files: vec!["src".to_string()],
        source_model_ref: Some(".loom/deployment/specs/generated/source-model.json".to_string()),
        topology_ref: Some(".loom/deployment/specs/generated/topology.json".to_string()),
        model_repair_ref: Some(".loom/deployment/specs/generated/model-repair.json".to_string()),
        generated_file_refs: vec![".loom/deployment/specs/generated/compose.yaml".to_string()],
        diagnostics_ref: None,
        error_window: None,
        read_policy: delivery_core::DeploymentRepairReadPolicy {
            first_read: "Use next diagnostics first.".to_string(),
            diagnostics_ref: "Read diagnosticsRef only if needed.".to_string(),
            full_log_ref: "Read full logs only if compact evidence is insufficient.".to_string(),
        },
        deploy_reference_profile: delivery_core::DeployReferenceProfile {
            load_mode: "mcp_reference_load_plan".to_string(),
            reference_load_plan: vec![
                delivery_core::ReferenceLoadPlanItem {
                    ref_id: "deploy.repair".to_string(),
                    path: "repair.md".to_string(),
                    reason: "Deploy repair decision tree.".to_string(),
                },
                delivery_core::ReferenceLoadPlanItem {
                    ref_id: "deploy.compose".to_string(),
                    path: "compose.md".to_string(),
                    reason: "Compose repair guidance.".to_string(),
                },
                delivery_core::ReferenceLoadPlanItem {
                    ref_id: "deploy.dockerfile".to_string(),
                    path: "dockerfile.md".to_string(),
                    reason: "Dockerfile repair guidance.".to_string(),
                },
            ],
        },
        retry_tool: "loom.deployUp".to_string(),
    })
}

fn assert_no_forbidden_keys(value: &Value) {
    match value {
        Value::Object(object) => {
            for key in object.keys() {
                assert!(
                    !FORBIDDEN_KEYS.contains(&key.as_str()),
                    "forbidden key {key} appears in {value}"
                );
            }
            for child in object.values() {
                assert_no_forbidden_keys(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_forbidden_keys(item);
            }
        }
        _ => {}
    }
}
