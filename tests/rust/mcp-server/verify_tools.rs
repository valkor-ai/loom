use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn verify_onboarding_records_deferred_choice_without_creating_appkey() {
    let fixture = Fixture::new("deferred");
    let server = LoomMcpServer::default();
    let root = fixture.root.to_string_lossy().into_owned();

    let initialized = server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": root }))),
        )
        .expect("initProject call");
    assert_eq!(structured(initialized)["state"], "done");

    let gate = server
        .invoke_tool(
            "loom.verify",
            Some(args(json!({ "projectRoot": fixture.root }))),
        )
        .expect("verify onboarding call");
    let gate = structured(gate);
    assert_eq!(gate["state"], "user_gate", "{gate:#}");
    assert_eq!(gate["gate"]["kind"], "vsefm_onboarding");
    assert_eq!(gate["acceptedResponses"], json!(["1", "2"]));
    assert!(fixture
        .root
        .join(".loom/verification/v-sefm.json")
        .is_file());

    let resolved = server
        .invoke_tool(
            "loom.verify",
            Some(args(json!({
                "projectRoot": fixture.root,
                "decision": "2"
            }))),
        )
        .expect("verify deferred call");
    let resolved = structured(resolved);
    assert_eq!(resolved["state"], "done", "{resolved:#}");
    assert_eq!(resolved["details"]["decision"], "deferred");
    assert!(!fixture.root.join(".loom-home/v-sefm/appkey").exists());

    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/verification/v-sefm.json"))
            .expect("V-SEFM record"),
    )
    .expect("valid V-SEFM record");
    assert_eq!(record["decision"], "deferred");
    assert_eq!(record["status"], "completed");
    assert!(record.get("appKey").is_none());
    assert!(record.get("resumeAction").is_none());
}

#[test]
fn verify_required_with_appkey_runs_local_agent_verification_and_gates_result() {
    let fixture = Fixture::new("local-verification");
    let server = LoomMcpServer::default();
    let root = fixture.root.to_string_lossy().into_owned();
    let initialized = server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": root }))),
        )
        .expect("initProject call");
    assert_eq!(structured(initialized)["state"], "done");

    let appkey = fixture.root.join(".loom-home/v-sefm/appkey");
    std::fs::create_dir_all(appkey.parent().expect("appkey parent")).expect("appkey directory");
    std::fs::write(&appkey, "local-test-key").expect("appkey");

    let onboarding = server
        .invoke_tool("loom.verify", Some(args(json!({ "projectRoot": root }))))
        .expect("verify onboarding call");
    assert_eq!(structured(onboarding)["state"], "user_gate");

    let started = server
        .invoke_tool(
            "loom.verify",
            Some(args(
                json!({ "projectRoot": fixture.root, "decision": "1" }),
            )),
        )
        .expect("required verify call");
    let started = structured(started);
    assert_eq!(started["state"], "auto_runnable", "{started:#}");
    assert_eq!(started["next"]["kind"], "run_vsefm_verification");
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/verification/v-sefm.json"))
            .expect("V-SEFM local verification record"),
    )
    .expect("valid V-SEFM local verification record");
    assert_eq!(record["status"], "local_verification_pending");
    assert_eq!(record["appKeyPresent"], true);
    assert_eq!(record["urlOpened"], false);
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    let request = request_json(&fixture, request_ref);
    assert_eq!(
        request["requestType"], "vsefm_local_verification",
        "{request:#}"
    );
    let steps = request["agentInstruction"]["steps"]
        .as_array()
        .expect("agent steps");
    assert!(steps.iter().any(|step| step
        .as_str()
        .is_some_and(|step| step.contains("prompt catalog"))));
    assert_eq!(
        request["subject"]["acceptedArtifacts"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(request["subject"].get("checkPlan").is_none());
    assert!(request["prompt"].get("checkPlan").is_none());
    assert!(request["prompt"].get("requiredCheckIds").is_none());
    assert!(request["source"].get("checkPlan").is_none());
    assert!(request["prompt"]["ruleCatalogHash"].is_string());
    assert!(request["agentInstruction"]["steps"]
        .as_array()
        .expect("agent steps")
        .iter()
        .any(|step| step
            .as_str()
            .is_some_and(|step| step
                .contains("initial verification pass must attempt every applicable rule"))));
    assert!(request["agentInstruction"]["steps"]
        .as_array()
        .expect("agent steps")
        .iter()
        .any(|step| step
            .as_str()
            .is_some_and(|step| step.contains("client-provided identity"))));
    assert!(request["agentInstruction"]["steps"]
        .as_array()
        .expect("agent steps")
        .iter()
        .any(|step| step
            .as_str()
            .is_some_and(|step| step.contains("MCP does not decide semantic applicability"))));
    assert!(request["runtimeProvenance"]["runtimeFingerprint"].is_string());
    assert!(request["runtimeProvenance"]["verificationRulesSha256"].is_string());
    assert!(request["agentInstruction"]["hardBlockingRules"]
        .as_array()
        .expect("hard blocking rules")
        .iter()
        .any(|rule| rule
            .as_str()
            .is_some_and(|rule| rule.contains("Loom derives the outer status"))));
    let inspected = server
        .invoke_tool(
            "loom.inspectRequest",
            Some(args(json!({
                "projectRoot": fixture.root,
                "requestRef": request_ref
            }))),
        )
        .expect("verification request inspection");
    let inspected = structured(inspected);
    let group_ids = inspected["readGroups"]
        .as_array()
        .expect("verification read groups")
        .iter()
        .filter_map(|group| group["groupId"].as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(group_ids.len(), 4);
    for group_id in &group_ids {
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "groupId": group_id
                }))),
            )
            .expect("verification request group");
    }
    let contract = read_request_group(
        &server,
        &fixture,
        request_ref,
        "verification_result_contract",
    );
    assert_eq!(
        contract["fields"]["outputContract"]["agentOwnedFields"],
        json!([
            "checks",
            "not_applicable_checks",
            "environment_blocked_checks",
            "blocking_failures",
            "warnings",
            "recommended_actions"
        ])
    );
    assert_eq!(
        contract["fields"]["outputContract"]["mcpOwnedFields"],
        json!([
            "status",
            "artifact_id",
            "verification_id",
            "scope",
            "source",
            "rule_catalog_hash",
            "statistics",
            "attempts"
        ])
    );
    assert_eq!(
        contract["fields"]["outputContract"]["resultSchema"]["type"],
        "object"
    );
    assert_eq!(
        contract["fields"]["outputContract"]["resultSchema"]["additionalProperties"],
        false
    );
    assert!(
        contract["fields"]["outputContract"]["resultSchema"]["properties"]
            .get("status")
            .is_none()
    );
    assert_eq!(
        contract["fields"]["outputContract"]["mcpOwnedPaths"],
        json!([
            "checks[*].check_id",
            "not_applicable_checks[*].check_id",
            "environment_blocked_checks[*].check_id",
            "blocking_failures[*].check_id"
        ])
    );
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let check_ids = [
        "BUSINESS-INTENT",
        "AUTH-HORIZONTAL",
        "AUTH-VERTICAL",
        "TENANT-ISOLATION",
        "STATE-MACHINE",
        "IDEMPOTENCY",
        "CONCURRENCY",
        "TRANSACTION",
        "DATA-INTEGRITY",
        "API-COMPATIBILITY",
        "ERROR-RECOVERY",
        "SECURITY-BOUNDARY",
        "RETRY-TIMEOUT-RATE-LIMIT",
        "OBSERVABILITY-EVIDENCE",
        "REGRESSION-COMPATIBILITY",
        "PERFORMANCE-CAPACITY",
        "BROWSER-QUALITY",
    ];
    let catalog_ids = contract["fields"]["outputContract"]["ruleCatalog"]["rules"]
        .as_array()
        .expect("rule catalog")
        .iter()
        .map(|rule| rule["id"].as_str().expect("rule id"))
        .collect::<Vec<_>>();
    assert_eq!(catalog_ids, check_ids);
    let template_ids = contract["fields"]["outputContract"]["resultTemplate"]["checks"]
        .as_array()
        .expect("result template checks")
        .iter()
        .map(|check| check["check_id"].as_str().expect("template check id"))
        .collect::<Vec<_>>();
    assert!(template_ids.is_empty());
    for definition in [
        "VsefmCheckResult",
        "VsefmNotApplicableCheck",
        "VsefmEnvironmentBlockedCheck",
        "VsefmBlockingFailure",
    ] {
        assert_eq!(
            contract["fields"]["outputContract"]["resultSchema"]["$defs"][definition]["properties"]
                ["check_id"]["enum"],
            json!(check_ids)
        );
    }
    let checks = check_ids
        .iter()
        .map(|check_id| {
            json!({
                "check_id": check_id,
                "category": "QUALITY",
                "rule": check_id,
                "status": "pass",
                "input": "local verification fixture",
                "expected": "no defect",
                "observed": "no defect",
                "evidence": format!("evidence-{check_id}"),
                "timestamp": "2026-08-05T00:00:00Z"
            })
        })
        .collect::<Vec<_>>();
    std::fs::create_dir_all(result_file.parent().expect("result parent"))
        .expect("result directory");
    std::fs::write(
        &result_file,
        serde_json::to_vec_pretty(&json!({
            "checks": checks,
            "not_applicable_checks": [],
            "environment_blocked_checks": [],
            "blocking_failures": [],
            "warnings": [],
            "recommended_actions": []
        }))
        .expect("candidate json"),
    )
    .expect("result candidate");

    let gated = server
        .invoke_tool(
            "loom.vsefmVerificationAcceptFile",
            Some(args(json!({
                "projectRoot": fixture.root,
                "requestRef": request_ref,
                "writtenTargetIds": ["result"]
            }))),
        )
        .expect("verification result submit");
    let gated = structured(gated);
    assert_eq!(gated["state"], "user_gate", "{gated:#}");
    assert_eq!(gated["gate"]["kind"], "vsefm_result");
    assert!(gated["requestRef"].is_null());

    let resumed_gate = server
        .invoke_tool(
            "loom.continue",
            Some(args(json!({ "projectRoot": fixture.root }))),
        )
        .expect("continue from verification result gate");
    let resumed_gate = structured(resumed_gate);
    assert_eq!(resumed_gate["state"], "user_gate", "{resumed_gate:#}");
    assert_eq!(resumed_gate["gate"]["kind"], "vsefm_result");

    let accepted = server
        .invoke_tool(
            "loom.vsefmVerificationResolve",
            Some(args(json!({
                "projectRoot": fixture.root,
                "verificationId": started["next"]["verificationId"],
                "decision": "accept"
            }))),
        )
        .expect("verification accept");
    assert_eq!(structured(accepted)["state"], "done");
    let verification_id = started["next"]["verificationId"]
        .as_str()
        .expect("verification id");
    let canonical_path = fixture
        .root
        .join(format!(".loom/verification/results/{verification_id}.json"));
    let canonical: Value = serde_json::from_str(
        &std::fs::read_to_string(&canonical_path).expect("canonical V-SEFM result"),
    )
    .expect("canonical result json");
    assert_eq!(canonical["verification_id"], verification_id);
    assert_eq!(
        canonical["coverage"]["checked_rule_ids"]
            .as_array()
            .map(Vec::len),
        Some(17)
    );
    assert_eq!(canonical["coverage"]["missing_rule_ids"], json!([]));
    assert_eq!(canonical["not_applicable_checks"], json!([]));
    assert_eq!(canonical["attempts"], 1);
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/verification/v-sefm.json"))
            .expect("completed V-SEFM record"),
    )
    .expect("completed record json");
    assert_eq!(record["status"], "completed");
    assert_eq!(record["verificationId"], verification_id);
    assert_eq!(
        record["resultRef"],
        format!(".loom/verification/results/{verification_id}.json")
    );
}

#[test]
fn canonical_status_is_derived_without_agent_outer_status() {
    let (fixture, server, started) = prepare_local_verification("status-normalization");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let candidate = blocked_candidate();
    write_json(&result_file, candidate);

    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate", "{gated:#}");
    assert_eq!(gated["gate"]["kind"], "vsefm_result");
    assert!(gated.get("issues").is_none() || gated["issues"].is_null());

    let verification_id = started["next"]["verificationId"]
        .as_str()
        .expect("verification id");
    let canonical: Value = serde_json::from_str(
        &std::fs::read_to_string(
            fixture
                .root
                .join(format!(".loom/verification/results/{verification_id}.json")),
        )
        .expect("canonical result"),
    )
    .expect("canonical result json");
    assert_eq!(canonical["status"], "blocked");
    assert_eq!(canonical["attempts"], 1);
}

#[test]
fn confirmed_failure_is_not_presented_as_environment_retry() {
    let (fixture, server, started) = prepare_local_verification("non-blocking-failure");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let mut candidate = passing_candidate();
    candidate["checks"]
        .as_array_mut()
        .expect("checks")
        .iter_mut()
        .find(|check| check["check_id"] == "BROWSER-QUALITY")
        .expect("browser quality check")
        .as_object_mut()
        .expect("browser quality object")
        .extend([
            ("status".to_string(), json!("fail")),
            ("observed".to_string(), json!("认证创建请求返回非 2xx")),
        ]);
    candidate["checks"]
        .as_array_mut()
        .expect("checks")
        .retain(|check| check["check_id"] != "CONCURRENCY");
    candidate["checks"]
        .as_array_mut()
        .expect("checks")
        .retain(|check| check["check_id"] != "TENANT-ISOLATION");
    candidate["environment_blocked_checks"] = json!([{
        "check_id": "CONCURRENCY",
        "attempted": "Ran the bounded concurrent-write verification against the local service.",
        "reason": "The local runner could not start the required concurrency tool.",
        "required_capability": "A working concurrency test runner."
    }]);
    candidate["not_applicable_checks"] = json!([{
        "check_id": "TENANT-ISOLATION",
        "reason": "The accepted delivery does not declare tenant or workspace boundaries.",
        "evidence": "Accepted architecture contains no tenant isolation model."
    }]);
    write_json(&result_file, candidate);

    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate", "{gated:#}");
    assert_eq!(gated["gate"]["options"][0]["decision"], "repair");
    assert_eq!(gated["gate"]["options"][1]["decision"], "manual_review");
    assert_eq!(gated["gate"]["counts"]["failed"], 1);
    assert_eq!(gated["gate"]["counts"]["environmentBlocked"], 1);
    assert_eq!(gated["gate"]["counts"]["notApplicable"], 1);
    assert_eq!(
        gated["gate"]["passedChecks"].as_array().map(Vec::len),
        Some(14)
    );
    assert_eq!(
        gated["gate"]["passedChecks"][0]["checkId"],
        "API-COMPATIBILITY"
    );
    assert_eq!(
        gated["gate"]["notApplicableChecks"][0]["checkId"],
        "TENANT-ISOLATION"
    );
    assert_eq!(
        gated["gate"]["notApplicableChecks"][0]["reason"],
        "The accepted delivery does not declare tenant or workspace boundaries."
    );
    assert_eq!(
        gated["gate"]["confirmedFindings"][0]["checkId"],
        "BROWSER-QUALITY"
    );
    assert!(gated["gate"]["confirmedFindings"][0]["title"]
        .as_str()
        .expect("rule title")
        .contains("浏览器质量"));
    assert_eq!(
        gated["gate"]["environmentBlockedChecks"][0]["checkId"],
        "CONCURRENCY"
    );
    assert_eq!(
        gated["gate"]["environmentBlockedChecks"][0]["requiredCapability"],
        "A working concurrency test runner."
    );
    assert!(gated["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("gate 结构化结果"));
    assert!(gated["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("environmentBlockedChecks"));
    assert!(gated["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("passedChecks"));
    assert!(gated["agentInstruction"]
        .as_str()
        .expect("agent instruction")
        .contains("notApplicableChecks"));
}

#[test]
fn incomplete_rule_coverage_is_rejected_with_a_missing_coverage_issue() {
    let (fixture, server, started) = prepare_local_verification("incomplete-coverage");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let mut candidate = passing_candidate();
    candidate["checks"]
        .as_array_mut()
        .expect("checks")
        .retain(|check| check["check_id"] != "CONCURRENCY");
    write_json(&result_file, candidate);

    let rejected = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("incomplete verification result submit"),
    );
    assert_eq!(rejected["state"], "repairable_error", "{rejected:#}");
    assert_eq!(rejected["stopAllowed"], false, "{rejected:#}");
    assert!(rejected["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "VSEFM_CHECK_COVERAGE_MISSING"
            && issue["message"]
                .as_str()
                .is_some_and(|message| message.contains("CONCURRENCY"))));
}

#[test]
fn environment_blocked_result_uses_environment_retry_without_repair_request() {
    let (fixture, server, started) = prepare_local_verification("supplemental-verification");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let mut candidate = passing_candidate();
    candidate["checks"]
        .as_array_mut()
        .expect("checks")
        .retain(|check| check["check_id"] != "CONCURRENCY");
    candidate["environment_blocked_checks"] = json!([{
        "check_id": "CONCURRENCY",
        "attempted": "Ran the bounded concurrent-write verification against the local service.",
        "reason": "The local runner could not start the required concurrency tool.",
        "required_capability": "A working concurrency test runner."
    }]);
    write_json(&result_file, candidate);

    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("environment-blocked verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate", "{gated:#}");
    assert_eq!(
        gated["gate"]["options"][0]["decision"],
        "supplemental_verification"
    );
    assert_eq!(
        gated["gate"]["environmentBlockedChecks"][0]["checkId"],
        "CONCURRENCY"
    );
    assert_eq!(
        gated["gate"]["environmentBlockedChecks"][0]["reason"],
        "The local runner could not start the required concurrency tool."
    );

    let supplemental = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "supplemental_verification"
                }))),
            )
            .expect("supplemental verification decision"),
    );
    assert_eq!(supplemental["state"], "auto_runnable", "{supplemental:#}");
    assert_eq!(
        supplemental["next"]["kind"], "run_vsefm_verification",
        "{supplemental:#}"
    );
    assert_ne!(
        supplemental["next"]["verificationId"],
        started["next"]["verificationId"]
    );
    let supplemental_request_ref = supplemental["next"]["requestRef"]
        .as_str()
        .expect("supplemental request ref");
    let supplemental_request = request_json(&fixture, supplemental_request_ref);
    assert_eq!(
        supplemental_request["requestType"],
        "vsefm_supplemental_verification"
    );
    assert_eq!(
        supplemental_request["source"]["supplementalRuleIds"],
        json!(["CONCURRENCY"])
    );
    assert!(supplemental_request["agentInstruction"]["steps"]
        .as_array()
        .expect("supplemental steps")
        .iter()
        .any(|step| step.as_str().is_some_and(
            |step| step.contains("only these environment-blocked check ids: CONCURRENCY")
        )));
    assert_eq!(supplemental_request["subject"].get("checkPlan"), None);
    let supplemental_contract = read_request_group(
        &server,
        &fixture,
        supplemental_request_ref,
        "verification_result_contract",
    );
    assert_eq!(
        supplemental_contract["fields"]["outputContract"]["resultTemplate"]["checks"],
        json!([])
    );
    for definition in [
        "VsefmCheckResult",
        "VsefmNotApplicableCheck",
        "VsefmEnvironmentBlockedCheck",
        "VsefmBlockingFailure",
    ] {
        assert_eq!(
            supplemental_contract["fields"]["outputContract"]["resultSchema"]["$defs"][definition]
                ["properties"]["check_id"]["enum"],
            json!(["CONCURRENCY"])
        );
    }
    assert_ne!(
        supplemental_request["requestType"], "vsefm_repair",
        "{supplemental_request:#}"
    );
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(format!(
            ".loom/verification/sessions/{}/state.json",
            started["next"]["verificationId"]
                .as_str()
                .expect("verification id")
        )))
        .expect("original verification state"),
    )
    .expect("original verification state json");
    assert_eq!(state["status"], "supplemental_started");

    read_request_groups(&server, &fixture, supplemental_request_ref);
    let supplemental_result_file = fixture.root.join(
        supplemental["next"]["resultFile"]
            .as_str()
            .expect("supplemental result file"),
    );
    write_json(
        &supplemental_result_file,
        json!({
            "checks": [{
                "check_id": "CONCURRENCY",
                "category": "concurrency",
                "rule": "Concurrent writes preserve unique identifiers and business invariants.",
                "status": "pass",
                "input": "Concurrent ticket creation and concurrent state mutations.",
                "expected": "No duplicate sequence or lost update occurs under concurrent requests.",
                "observed": "Concurrent requests completed without duplicate sequence or lost update.",
                "evidence": "bounded concurrency test",
                "timestamp": "2026-08-05T00:00:00Z"
            }],
            "blocking_failures": [],
            "warnings": [],
            "environment_blocked_checks": [],
            "recommended_actions": []
        }),
    );
    let merged_gate = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": supplemental_request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("supplemental result submit"),
    );
    assert_eq!(merged_gate["state"], "user_gate", "{merged_gate:#}");
    let supplemental_verification_id = supplemental["next"]["verificationId"]
        .as_str()
        .expect("supplemental verification id");
    let merged_result: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(format!(
            ".loom/verification/results/{supplemental_verification_id}.json"
        )))
        .expect("merged supplemental result"),
    )
    .expect("merged supplemental result json");
    assert_eq!(merged_result["status"], "pass");
    assert_eq!(merged_result["environment_blocked_count"], 0);
    assert_eq!(merged_result["checks"].as_array().map(Vec::len), Some(17));
}

#[test]
fn blocked_vsefm_result_can_enter_repair_and_reverification() {
    let (fixture, server, started) = prepare_local_verification("repair");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    write_json(&result_file, blocked_candidate());

    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("blocked verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate", "{gated:#}");
    assert_eq!(gated["acceptedResponses"], json!(["1", "2"]));

    let repair = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "repair"
                }))),
            )
            .expect("repair decision"),
    );
    assert_eq!(repair["state"], "auto_runnable", "{repair:#}");
    assert_eq!(repair["next"]["kind"], "run_vsefm_repair");
    let repair_request_ref = repair["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    read_request_groups(&server, &fixture, repair_request_ref);
    let repair_request = read_request_group(
        &server,
        &fixture,
        repair_request_ref,
        "repair_result_contract",
    );
    assert_eq!(
        repair_request["fields"]["outputContract"]["resultSchema"]["type"], "object",
        "{repair_request:#}"
    );
    assert_eq!(
        repair_request["fields"]["outputContract"]["agentOwnedFields"],
        json!(["status", "summary", "details"]),
        "{repair_request:#}"
    );
    let repair_file = fixture.root.join(
        repair["next"]["resultFile"]
            .as_str()
            .expect("repair result file"),
    );
    std::fs::create_dir_all(fixture.root.join("src")).expect("repair source directory");
    std::fs::write(fixture.root.join("src/repair-needed.txt"), "repair change")
        .expect("repair source file");
    write_json(
        &repair_file,
        json!({
            "status": "ready",
            "summary": "The blocking authorization check was repaired and verified.",
            "details": {
                "nested": [null, {"note": "user-facing"}]
            },
            "legacy_field": {"is_opaque": true}
        }),
    );
    let reverified = structured(
        server
            .invoke_tool(
                "loom.vsefmRepairAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": repair_request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("repair result submit"),
    );
    assert_eq!(reverified["state"], "auto_runnable", "{reverified:#}");
    assert_eq!(reverified["next"]["kind"], "run_vsefm_verification");
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(format!(
                ".loom/verification/sessions/{verification_id}/state.json",
                verification_id = started["next"]["verificationId"]
                    .as_str()
                    .expect("verification id")
            )))
        .expect("repair session state"),
    )
    .expect("repair session state json");
    assert!(state["repairChangedFiles"]
        .as_array()
        .expect("changed files")
        .iter()
        .any(|path| path == "src/repair-needed.txt"));
    let verification_id = started["next"]["verificationId"]
        .as_str()
        .expect("verification id");
    let repair_audit = fixture.root.join(format!(
        ".loom/verification/sessions/{verification_id}/repair-submit-attempts.jsonl"
    ));
    assert_eq!(
        std::fs::read_to_string(repair_audit)
            .expect("repair submit audit")
            .lines()
            .count(),
        1
    );
}

#[test]
fn blocked_vsefm_result_can_be_resolved_by_manual_review() {
    let (fixture, server, started) = prepare_local_verification("manual-review");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    write_json(&result_file, blocked_candidate());
    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("blocked verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate", "{gated:#}");

    let manual_gate = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "manual_review"
                }))),
            )
            .expect("manual review decision"),
    );
    assert_eq!(manual_gate["state"], "user_gate", "{manual_gate:#}");
    assert_eq!(manual_gate["acceptedResponses"], json!(["1", "2"]));

    let resumed_manual_gate = structured(
        server
            .invoke_tool(
                "loom.continue",
                Some(args(json!({ "projectRoot": fixture.root }))),
            )
            .expect("continue from manual V-SEFM review gate"),
    );
    assert_eq!(resumed_manual_gate["state"], "user_gate");
    assert_eq!(resumed_manual_gate["gate"]["kind"], "vsefm_manual_review");

    let resolved = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "approve_override"
                }))),
            )
            .expect("manual review approval"),
    );
    assert_eq!(resolved["state"], "done", "{resolved:#}");
}

#[test]
fn repair_rejects_agent_changes_to_loom_control_files() {
    let (fixture, server, started) = prepare_local_verification("protected-repair");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    write_json(&result_file, blocked_candidate());
    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("blocked verification result submit"),
    );
    assert_eq!(gated["state"], "user_gate");
    let repair = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "repair"
                }))),
            )
            .expect("repair decision"),
    );
    let repair_request_ref = repair["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    read_request_groups(&server, &fixture, repair_request_ref);
    let repair_file = fixture.root.join(
        repair["next"]["resultFile"]
            .as_str()
            .expect("repair result file"),
    );
    write_json(
        &repair_file,
        json!({"status": "ready", "summary": "repair complete"}),
    );
    write_json(
        &fixture.root.join(".loom/forbidden-agent-change.json"),
        json!({"changedBy": "agent"}),
    );
    let rejected = structured(
        server
            .invoke_tool(
                "loom.vsefmRepairAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": repair_request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("protected repair submit"),
    );
    assert_eq!(rejected["state"], "failed", "{rejected:#}");
    assert_eq!(rejected["error"]["code"], "VSEFM_REPAIR_PROTECTED_CHANGE");
}

#[test]
fn repeated_contract_error_is_not_routed_to_manual_review() {
    let (fixture, server, started) = prepare_local_verification("repeated-invalid");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    let mut invalid = passing_candidate();
    invalid
        .as_object_mut()
        .expect("invalid result object")
        .remove("checks");
    write_json(&result_file, invalid.clone());

    let first = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("first invalid result submit"),
    );
    assert_eq!(first["state"], "repairable_error", "{first:#}");
    assert_eq!(first["stopAllowed"], false, "{first:#}");
    assert!(first["issues"][0]["fieldPath"].is_string(), "{first:#}");

    let second = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("repeated invalid result submit"),
    );
    assert_eq!(second["state"], "done", "{second:#}");
    assert!(second["warnings"]
        .as_array()
        .expect("contract fault warning")
        .iter()
        .any(|warning| warning
            .as_str()
            .is_some_and(|warning| warning.contains("contract error repeated"))));

    let verification_id = started["next"]["verificationId"]
        .as_str()
        .expect("verification id");
    let audit = fixture.root.join(format!(
        ".loom/verification/sessions/{verification_id}/submit-attempts.jsonl"
    ));
    assert_eq!(
        std::fs::read_to_string(audit)
            .expect("submit audit")
            .lines()
            .count(),
        2
    );
}

#[test]
fn repeated_repair_contract_error_resumes_without_manual_review() {
    let (fixture, server, started) = prepare_local_verification("repeated-repair-invalid");
    let request_ref = started["next"]["requestRef"].as_str().expect("request ref");
    read_request_groups(&server, &fixture, request_ref);
    let result_file = fixture
        .root
        .join(started["next"]["resultFile"].as_str().expect("result file"));
    write_json(&result_file, blocked_candidate());
    let gated = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("blocked verification result submit"),
    );
    let repair = structured(
        server
            .invoke_tool(
                "loom.vsefmVerificationResolve",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "verificationId": started["next"]["verificationId"],
                    "decision": "repair"
                }))),
            )
            .expect("repair decision"),
    );
    assert_eq!(gated["state"], "user_gate");
    let repair_request_ref = repair["next"]["requestRef"]
        .as_str()
        .expect("repair request ref");
    read_request_groups(&server, &fixture, repair_request_ref);
    let repair_file = fixture.root.join(
        repair["next"]["resultFile"]
            .as_str()
            .expect("repair result file"),
    );
    let invalid = json!({"status": "ready"});
    write_json(&repair_file, invalid);
    let first = structured(
        server
            .invoke_tool(
                "loom.vsefmRepairAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": repair_request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("first invalid repair result"),
    );
    assert_eq!(first["state"], "repairable_error", "{first:#}");
    let second = structured(
        server
            .invoke_tool(
                "loom.vsefmRepairAcceptFile",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": repair_request_ref,
                    "writtenTargetIds": ["result"]
                }))),
            )
            .expect("repeated invalid repair result"),
    );
    assert_eq!(second["state"], "done", "{second:#}");
    assert!(second["warnings"]
        .as_array()
        .expect("contract fault warning")
        .iter()
        .any(|warning| warning
            .as_str()
            .is_some_and(|warning| warning.contains("contract error repeated"))));
}

fn passing_candidate() -> Value {
    let checks = [
        "BUSINESS-INTENT",
        "AUTH-HORIZONTAL",
        "AUTH-VERTICAL",
        "TENANT-ISOLATION",
        "STATE-MACHINE",
        "IDEMPOTENCY",
        "CONCURRENCY",
        "TRANSACTION",
        "DATA-INTEGRITY",
        "API-COMPATIBILITY",
        "ERROR-RECOVERY",
        "SECURITY-BOUNDARY",
        "RETRY-TIMEOUT-RATE-LIMIT",
        "OBSERVABILITY-EVIDENCE",
        "REGRESSION-COMPATIBILITY",
        "BROWSER-QUALITY",
        "PERFORMANCE-CAPACITY",
    ]
    .iter()
    .map(|check_id| {
        json!({
            "check_id": check_id,
            "category": "QUALITY",
            "rule": check_id,
            "status": "pass",
            "input": "local verification fixture",
            "expected": "no defect",
            "observed": "no defect",
            "evidence": format!("evidence-{check_id}"),
            "timestamp": "2026-08-05T00:00:00Z"
        })
    })
    .collect::<Vec<_>>();
    json!({
        "checks": checks,
        "blocking_failures": [],
        "warnings": [],
        "environment_blocked_checks": [],
        "recommended_actions": []
    })
}

fn prepare_local_verification(name: &str) -> (Fixture, LoomMcpServer, Value) {
    let fixture = Fixture::new(name);
    let server = LoomMcpServer::default();
    let root = fixture.root.to_string_lossy().into_owned();
    let initialized = server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": root }))),
        )
        .expect("initProject call");
    assert_eq!(structured(initialized)["state"], "done");
    let appkey = fixture.root.join(".loom-home/v-sefm/appkey");
    std::fs::create_dir_all(appkey.parent().expect("appkey parent")).expect("appkey directory");
    std::fs::write(&appkey, "local-test-key").expect("appkey");
    server
        .invoke_tool("loom.verify", Some(args(json!({ "projectRoot": root }))))
        .expect("verify onboarding call");
    let started = structured(
        server
            .invoke_tool(
                "loom.verify",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "decision": "1"
                }))),
            )
            .expect("required verify call"),
    );
    assert_eq!(started["state"], "auto_runnable", "{started:#}");
    (fixture, server, started)
}

fn read_request_groups(server: &LoomMcpServer, fixture: &Fixture, request_ref: &str) {
    let inspected = structured(
        server
            .invoke_tool(
                "loom.inspectRequest",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref
                }))),
            )
            .expect("request inspection"),
    );
    for group in inspected["readGroups"].as_array().expect("read groups") {
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "groupId": group["groupId"]
                }))),
            )
            .expect("request group");
    }
}

fn read_request_group(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    group_id: &str,
) -> Value {
    structured(
        server
            .invoke_tool(
                "loom.readFieldGroup",
                Some(args(json!({
                    "projectRoot": fixture.root,
                    "requestRef": request_ref,
                    "groupId": group_id
                }))),
            )
            .expect("request group"),
    )
}

fn request_json(fixture: &Fixture, request_ref: &str) -> Value {
    let index: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/requests/index.json"))
            .expect("request index"),
    )
    .expect("request index json");
    let relative = index["requests"]
        .as_array()
        .expect("request index requests")
        .iter()
        .find(|entry| entry["requestRef"] == request_ref)
        .and_then(|entry| entry["requestFile"].as_str())
        .expect("request file ref");
    let path = fixture.root.join(relative);
    serde_json::from_str(&std::fs::read_to_string(path).expect("request file"))
        .expect("request json")
}

fn blocked_candidate() -> Value {
    let checks = [
        "BUSINESS-INTENT",
        "AUTH-HORIZONTAL",
        "AUTH-VERTICAL",
        "TENANT-ISOLATION",
        "STATE-MACHINE",
        "IDEMPOTENCY",
        "CONCURRENCY",
        "TRANSACTION",
        "DATA-INTEGRITY",
        "API-COMPATIBILITY",
        "ERROR-RECOVERY",
        "SECURITY-BOUNDARY",
        "RETRY-TIMEOUT-RATE-LIMIT",
        "OBSERVABILITY-EVIDENCE",
        "REGRESSION-COMPATIBILITY",
        "BROWSER-QUALITY",
        "PERFORMANCE-CAPACITY",
    ]
    .iter()
    .map(|check_id| {
        json!({
            "check_id": check_id,
            "category": "QUALITY",
            "rule": check_id,
            "status": if *check_id == "AUTH-HORIZONTAL" { "fail" } else { "pass" },
            "input": "User A requests User B resource",
            "expected": "403 or 404",
            "observed": if *check_id == "AUTH-HORIZONTAL" { "200" } else { "no defect" },
            "evidence": format!("evidence-{check_id}"),
            "timestamp": "2026-08-05T00:00:00Z"
        })
    })
    .collect::<Vec<_>>();
    json!({
        "checks": checks,
        "blocking_failures": [{
            "finding_id": "finding-auth-horizontal-001",
            "check_id": "AUTH-HORIZONTAL",
            "severity": "critical",
            "summary": "User B data was returned to User A.",
            "remediation": "Bind resource lookup and authorization to the authenticated owner."
        }],
        "warnings": [],
        "environment_blocked_checks": [],
        "recommended_actions": ["Add object-level authorization."]
    })
}

fn write_json(path: &std::path::Path, value: Value) {
    std::fs::create_dir_all(path.parent().expect("result parent")).expect("result directory");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&value).expect("candidate json"),
    )
    .expect("candidate file");
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value.as_object().cloned().expect("json object args")
}

fn structured(result: rmcp::model::CallToolResult) -> Value {
    serde_json::to_value(result).expect("call result")["structuredContent"].clone()
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().expect("env lock");
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-verify-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            _guard: guard,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
