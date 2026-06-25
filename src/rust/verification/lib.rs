use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use mcp_server::{resource_registry::BATCH_2_RESOURCE_TEMPLATES, tool_registry::BATCH_2_TOOLS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const FORBIDDEN_PROTOCOL_TERMS: &[&str] = &[
    "commandInvocation",
    "submitCommand",
    "retryCommand",
    "run_cli",
    "next-task",
    "readCommand",
    "agentAction.read",
    "loom-cli",
    "LOOM_AGENT_PROFILE",
    "LOOM_COMPACT_OUTPUT",
];

const FORBIDDEN_PACKAGE_ENTRIES: &[&str] = &[
    "src/",
    "tests/",
    "node_modules/",
    "dist/",
    "scripts/refresh-local-codex-plugin.js",
    "scripts/refresh-local-claude-plugin.js",
    "scripts/refresh-local-opencode-plugin.js",
    "scripts/uninstall-local-adapter.js",
    "package-lock.json",
    "bin/loom-cli",
];

const REQUIRED_PACKAGE_ENTRIES: &[&str] = &[
    "bin/loom-mcp-server",
    "bin/loom-setup",
    "python/runtime",
    "python/algorithms",
    "plugins/codex",
    "plugins/claude-code",
    "plugins/opencode",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalVerificationInput {
    pub reference_projection: Value,
    #[serde(default)]
    pub transcripts: Vec<TranscriptInput>,
    #[serde(default)]
    pub setup_checks: Vec<NamedCheck>,
    #[serde(default)]
    pub package_entries: Vec<String>,
    #[serde(default)]
    pub benchmark: BenchmarkInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptInput {
    pub agent: String,
    pub source: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedCheck {
    pub name: String,
    pub passed: bool,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkInput {
    pub reference_payload_bytes: u64,
    pub reference_completion_rate: f64,
    pub mcp_completion_rate: f64,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Default for BenchmarkInput {
    fn default() -> Self {
        Self {
            reference_payload_bytes: 0,
            reference_completion_rate: 1.0,
            mcp_completion_rate: 1.0,
            notes: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalVerificationSummary {
    pub passed: bool,
    pub issue_count: usize,
    pub report_files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionDiff {
    status: String,
    issue_count: usize,
    issues: Vec<VerificationIssue>,
    current: Value,
    reference: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolSnapshotReport {
    status: String,
    tool_count: usize,
    resource_template_count: usize,
    request_count: usize,
    requests_by_kind: BTreeMap<String, usize>,
    deliveries: Vec<DeliveryIsolationSummary>,
    forbidden_terms: Vec<String>,
    issues: Vec<VerificationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryIsolationSummary {
    delivery_id: String,
    phase_ids: Vec<String>,
    request_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentTranscriptAudit {
    status: String,
    audits: Vec<TranscriptAudit>,
    issues: Vec<VerificationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptAudit {
    agent: String,
    source: String,
    mcp_tool_call_count: usize,
    forbidden_command_count: usize,
    jq_count: usize,
    full_artifact_read_count: usize,
    auto_runnable_stop_count: usize,
    issues: Vec<VerificationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenReadPlanSummary {
    status: String,
    request_count: usize,
    total_payload_bytes: u64,
    max_compact_bytes: u64,
    total_field_read_count: usize,
    read_field_group_count: usize,
    read_request_fields_count: usize,
    full_artifact_read_count: usize,
    jq_count: usize,
    reference_payload_bytes: u64,
    mcp_completion_rate: f64,
    reference_completion_rate: f64,
    issues: Vec<VerificationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupPackagingReport {
    status: String,
    setup_checks: Vec<NamedCheck>,
    required_entries: Vec<String>,
    forbidden_entries: Vec<String>,
    issues: Vec<VerificationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerificationIssue {
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    details: BTreeMap<String, Value>,
}

pub fn canonical_project_projection(project_root: &str) -> Result<Value> {
    let paths = state::paths::project_paths(project_root)?;
    let request_index = state::request_index::load_request_index(project_root)?;
    let mut deliveries = vec![];
    if paths.deliveries_dir.exists() {
        for entry in fs::read_dir(&paths.deliveries_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let delivery_id = entry.file_name().to_string_lossy().into_owned();
            let index_file = entry.path().join("index.json");
            if !index_file.exists() {
                continue;
            }
            let delivery: Value = state::store::read_json_value(&index_file)?;
            let phase_ids = delivery
                .get("phases")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|phase| phase.get("phaseId").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>();
            let request_ids = request_index
                .requests
                .iter()
                .filter(|request| request.delivery_id.as_deref() == Some(delivery_id.as_str()))
                .map(|request| {
                    json!({
                        "requestId": request.request_id,
                        "requestKind": request.request_kind,
                        "phaseId": request.phase_id,
                        "sourceProtocol": request.source_protocol,
                    })
                })
                .collect::<Vec<_>>();
            deliveries.push(json!({
                "deliveryId": delivery_id,
                "activePhaseId": delivery.get("activePhaseId"),
                "status": delivery.get("status"),
                "phaseIds": phase_ids,
                "requestIds": request_ids,
            }));
        }
    }
    deliveries.sort_by(|left, right| {
        left.get("deliveryId")
            .and_then(Value::as_str)
            .cmp(&right.get("deliveryId").and_then(Value::as_str))
    });
    Ok(json!({
        "schemaVersion": "mcp-only-canonical-projection-v1",
        "deliveryCount": deliveries.len(),
        "requestCount": request_index.requests.len(),
        "requestKinds": request_kind_counts(&request_index),
        "deliveries": deliveries,
    }))
}

pub fn generate_final_verification(
    project_root: &str,
    input: FinalVerificationInput,
) -> Result<FinalVerificationSummary> {
    let paths = state::paths::project_paths(project_root)?;
    let final_dir = paths.loom_dir.join("verification/final");
    let token_dir = paths.loom_dir.join("verification/token-read-plan");
    state::store::ensure_dir(&final_dir)?;
    state::store::ensure_dir(&token_dir)?;

    let current_projection = canonical_project_projection(project_root)?;
    let projection_diff = compare_projection(&current_projection, &input.reference_projection);
    let protocol_report = build_protocol_report(project_root)?;
    let transcript_audit = audit_transcripts(&input.transcripts);
    let token_summary = build_token_summary(project_root, &input.benchmark)?;
    let token_by_stage = build_token_by_stage(project_root)?;
    let setup_report = build_setup_packaging_report(&input.setup_checks, &input.package_entries);
    let benchmark_markdown = benchmark_markdown(&token_summary, &input.benchmark);

    let mut report_files = BTreeMap::new();
    write_report_json(
        &final_dir.join("canonical-projection-diff.json"),
        &projection_diff,
        &mut report_files,
        "canonicalProjectionDiff",
        &paths.root,
    )?;
    write_report_json(
        &final_dir.join("protocol-snapshot-report.json"),
        &protocol_report,
        &mut report_files,
        "protocolSnapshotReport",
        &paths.root,
    )?;
    write_report_json(
        &final_dir.join("agent-transcript-audit.json"),
        &transcript_audit,
        &mut report_files,
        "agentTranscriptAudit",
        &paths.root,
    )?;
    write_report_json(
        &final_dir.join("setup-packaging-report.json"),
        &setup_report,
        &mut report_files,
        "setupPackagingReport",
        &paths.root,
    )?;
    write_report_json(
        &final_dir.join("token-read-plan-report.json"),
        &token_summary,
        &mut report_files,
        "tokenReadPlanReport",
        &paths.root,
    )?;
    write_report_json(
        &token_dir.join("summary.json"),
        &token_summary,
        &mut report_files,
        "tokenReadPlanSummary",
        &paths.root,
    )?;
    write_report_json(
        &token_dir.join("by-stage.json"),
        &token_by_stage,
        &mut report_files,
        "tokenReadPlanByStage",
        &paths.root,
    )?;
    write_report_text(
        &token_dir.join("benchmark.md"),
        &benchmark_markdown,
        &mut report_files,
        "tokenReadPlanBenchmark",
        &paths.root,
    )?;
    write_report_text(
        &final_dir.join("benchmark-summary.md"),
        &benchmark_markdown,
        &mut report_files,
        "benchmarkSummary",
        &paths.root,
    )?;

    let mut issues = vec![];
    issues.extend(projection_diff.issues.clone());
    issues.extend(protocol_report.issues.clone());
    issues.extend(transcript_audit.issues.clone());
    issues.extend(token_summary.issues.clone());
    issues.extend(setup_report.issues.clone());
    let regression_markdown = regression_report_markdown(
        issues.is_empty(),
        &projection_diff,
        &protocol_report,
        &transcript_audit,
        &token_summary,
        &setup_report,
    );
    write_report_text(
        &final_dir.join("mcp-only-regression-report.md"),
        &regression_markdown,
        &mut report_files,
        "mcpOnlyRegressionReport",
        &paths.root,
    )?;

    Ok(FinalVerificationSummary {
        passed: issues.is_empty(),
        issue_count: issues.len(),
        report_files,
    })
}

fn request_kind_counts(index: &state::request_index::RequestIndex) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for request in &index.requests {
        *counts.entry(request.request_kind.clone()).or_insert(0) += 1;
    }
    counts
}

fn compare_projection(current: &Value, reference: &Value) -> ProjectionDiff {
    let mut issues = vec![];
    if current != reference {
        issues.push(issue(
            "CANONICAL_PROJECTION_DIFF",
            "Current MCP projection differs from the supplied reference projection.",
            [
                ("current", current.clone()),
                ("reference", reference.clone()),
            ],
        ));
    }
    ProjectionDiff {
        status: if issues.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        issue_count: issues.len(),
        issues,
        current: current.clone(),
        reference: reference.clone(),
    }
}

fn build_protocol_report(project_root: &str) -> Result<ProtocolSnapshotReport> {
    let paths = state::paths::project_paths(project_root)?;
    let index = state::request_index::load_request_index(project_root)?;
    let mut issues = vec![];
    let mut deliveries: BTreeMap<String, DeliveryIsolationSummary> = BTreeMap::new();
    let mut seen_request_ids = BTreeSet::new();

    for request in &index.requests {
        if !seen_request_ids.insert(request.request_id.clone()) {
            issues.push(issue(
                "REQUEST_ID_NOT_UNIQUE",
                "Request index contains duplicate requestId.",
                [("requestId", json!(request.request_id))],
            ));
        }
        if request.source_protocol != state::request_index::RequestSourceProtocol::RustMcpNative {
            issues.push(issue(
                "NON_NATIVE_REQUEST_IN_PRODUCT_INDEX",
                "Final MCP-only regression must not rely on legacy TypeScript requests.",
                [("requestId", json!(request.request_id))],
            ));
        }
        let request_file = state::paths::from_project_relative(&paths.root, &request.request_file)?;
        let root = state::store::read_json_value(&request_file)
            .with_context(|| format!("read request {}", request.request_id))?;
        collect_protocol_issues(&request.request_id, &root, &mut issues);

        let manifest_file =
            state::paths::request_storage_manifest_file(&paths.root, &request.request_id);
        if !manifest_file.exists() {
            issues.push(issue(
                "REQUEST_STORAGE_MANIFEST_MISSING",
                "Native request is missing its private storage manifest.",
                [("requestId", json!(request.request_id))],
            ));
        } else {
            let manifest = state::store::read_json_value(&manifest_file)?;
            if manifest.get("protocolAuthority").and_then(Value::as_str)
                != Some("rust_private_request_storage_manifest")
            {
                issues.push(issue(
                    "REQUEST_STORAGE_MANIFEST_AUTHORITY_INVALID",
                    "Private request storage manifest has an invalid authority.",
                    [("requestId", json!(request.request_id))],
                ));
            }
            if serde_json::to_string(&root)?.contains(".refs") {
                issues.push(issue(
                    "AGENT_FACING_REFS_PATH",
                    "Native request root exposes a .refs path.",
                    [("requestId", json!(request.request_id))],
                ));
            }
        }

        if let Some(delivery_id) = &request.delivery_id {
            let summary =
                deliveries
                    .entry(delivery_id.clone())
                    .or_insert_with(|| DeliveryIsolationSummary {
                        delivery_id: delivery_id.clone(),
                        phase_ids: vec![],
                        request_ids: vec![],
                    });
            if let Some(phase_id) = &request.phase_id {
                if !summary.phase_ids.contains(phase_id) {
                    summary.phase_ids.push(phase_id.clone());
                }
            }
            summary.request_ids.push(request.request_id.clone());
        }
    }

    for entry in fs::read_dir(&paths.deliveries_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let delivery_id = entry.file_name().to_string_lossy().into_owned();
        let index_file = entry.path().join("index.json");
        if !index_file.exists() {
            issues.push(issue(
                "DELIVERY_INDEX_MISSING",
                "Delivery directory has no index.json.",
                [("deliveryId", json!(delivery_id))],
            ));
        }
    }

    if index.requests.is_empty() {
        issues.push(issue(
            "REQUEST_INDEX_EMPTY",
            "Final MCP-only regression did not produce any native request.",
            [],
        ));
    }
    let required_tools = [
        "loom.plan",
        "loom.continue",
        "loom.readFieldGroup",
        "loom.readRequestFields",
        "loom.brainstormAcceptFile",
        "loom.knowledgeBuild",
        "loom.knowledgeSearch",
        "loom.deployValidate",
        "loom.repairSubmitFile",
    ];
    let registered_tools = BATCH_2_TOOLS
        .iter()
        .map(|tool| tool.name)
        .collect::<BTreeSet<_>>();
    for required in required_tools {
        if !registered_tools.contains(required) {
            issues.push(issue(
                "REQUIRED_MCP_TOOL_MISSING",
                "MCP protocol surface is missing a required tool.",
                [("tool", json!(required))],
            ));
        }
    }

    let requests_by_kind = request_kind_counts(&index);
    Ok(ProtocolSnapshotReport {
        status: if issues.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        tool_count: BATCH_2_TOOLS.len(),
        resource_template_count: BATCH_2_RESOURCE_TEMPLATES.len(),
        request_count: index.requests.len(),
        requests_by_kind,
        deliveries: deliveries.into_values().collect(),
        forbidden_terms: FORBIDDEN_PROTOCOL_TERMS
            .iter()
            .map(|term| term.to_string())
            .collect(),
        issues,
    })
}

fn collect_protocol_issues(request_id: &str, root: &Value, issues: &mut Vec<VerificationIssue>) {
    for pointer in [
        "/requestManifest/refs",
        "/agentAction/read",
        "/agentAction/submit/command/argv",
        "/submitCommand",
        "/retryCommand",
        "/commandInvocation",
    ] {
        if root.pointer(pointer).is_some() {
            issues.push(issue(
                "LEGACY_PROTOCOL_FIELD",
                "Native MCP request exposes a legacy CLI protocol field.",
                [
                    ("requestId", json!(request_id)),
                    ("jsonPointer", json!(pointer)),
                ],
            ));
        }
    }
    if has_duplicate_context_ref_path(root) {
        issues.push(issue(
            "DUPLICATE_CONTEXT_REF_PATH",
            "contextRefs contains multiple keys pointing at the same path.",
            [("requestId", json!(request_id))],
        ));
    }
    let Some(groups) = root
        .pointer("/requestReadPlan/groups")
        .and_then(Value::as_array)
    else {
        issues.push(issue(
            "REQUEST_READ_PLAN_MISSING",
            "Native MCP request does not declare requestReadPlan.groups.",
            [("requestId", json!(request_id))],
        ));
        return;
    };
    for group in groups {
        for key in ["readCommand", "fallbackRule"] {
            if group.get(key).is_some() {
                issues.push(issue(
                    "READ_PLAN_SIDECAR_FIELD",
                    "requestReadPlan group contains sidecar read instructions.",
                    [("requestId", json!(request_id)), ("field", json!(key))],
                ));
            }
        }
        let fields = group
            .get("fields")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        for field in fields {
            if matches!(
                field,
                "rules"
                    | "outputContract.schemaShape"
                    | "clarificationConversationProtocol"
                    | "executionRules.rules"
                    | "contextProjection.requirementDetailTransfer"
            ) || field.contains(".refs")
                || field.contains("jq")
                || field.contains("readCommand")
                || field.contains("fallbackRule")
            {
                issues.push(issue(
                    "BROAD_OR_FORBIDDEN_READ_FIELD",
                    "requestReadPlan contains a broad or forbidden field selector.",
                    [("requestId", json!(request_id)), ("field", json!(field))],
                ));
            }
        }
    }
}

fn audit_transcripts(transcripts: &[TranscriptInput]) -> AgentTranscriptAudit {
    let mut all_issues = vec![];
    let present_agents = transcripts
        .iter()
        .map(|transcript| transcript.agent.as_str())
        .collect::<BTreeSet<_>>();
    for required in ["codex", "claude-code", "opencode"] {
        if !present_agents.contains(required) {
            all_issues.push(issue(
                "AGENT_TRANSCRIPT_MISSING",
                "Final transcript audit requires Codex, Claude Code, and OpenCode smoke transcripts.",
                [("agent", json!(required))],
            ));
        }
    }
    let audits = transcripts
        .iter()
        .map(|transcript| {
            let mut issues = vec![];
            let forbidden_command_count = FORBIDDEN_PROTOCOL_TERMS
                .iter()
                .filter(|term| transcript.content.contains(**term))
                .count();
            let jq_count = transcript.content.matches("jq ").count()
                + transcript.content.matches("\"jq\"").count();
            let full_artifact_read_count = [".loom/requests/", ".loom/deliveries/"]
                .iter()
                .filter(|needle| transcript.content.contains(**needle))
                .count();
            let mcp_tool_call_count = transcript.content.matches("loom.").count();
            let auto_runnable_stop_count = transcript
                .content
                .matches("state=auto_runnable but stopped")
                .count();
            if forbidden_command_count > 0 {
                issues.push(issue(
                    "FORBIDDEN_CLI_OR_PROTOCOL_TERM",
                    "Transcript contains a forbidden CLI/protocol term.",
                    [("agent", json!(transcript.agent))],
                ));
            }
            if jq_count > 0 {
                issues.push(issue(
                    "JQ_READ_DETECTED",
                    "Transcript contains jq-based .loom reading.",
                    [("agent", json!(transcript.agent))],
                ));
            }
            if full_artifact_read_count > 0 {
                issues.push(issue(
                    "FULL_ARTIFACT_READ_DETECTED",
                    "Transcript appears to read full .loom request/delivery artifacts.",
                    [("agent", json!(transcript.agent))],
                ));
            }
            if auto_runnable_stop_count > 0 {
                issues.push(issue(
                    "AUTO_RUNNABLE_STOP_DETECTED",
                    "Transcript records an auto-runnable stop.",
                    [("agent", json!(transcript.agent))],
                ));
            }
            all_issues.extend(issues.clone());
            TranscriptAudit {
                agent: transcript.agent.clone(),
                source: transcript.source.clone(),
                mcp_tool_call_count,
                forbidden_command_count,
                jq_count,
                full_artifact_read_count,
                auto_runnable_stop_count,
                issues,
            }
        })
        .collect::<Vec<_>>();
    AgentTranscriptAudit {
        status: if all_issues.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        audits,
        issues: all_issues,
    }
}

fn build_token_summary(
    project_root: &str,
    benchmark: &BenchmarkInput,
) -> Result<TokenReadPlanSummary> {
    let paths = state::paths::project_paths(project_root)?;
    let request_audits = read_jsonl(&paths.request_size_audit_file)?;
    let field_audits = read_jsonl(&paths.field_read_audit_file)?;
    let total_payload_bytes = request_audits
        .iter()
        .map(|value| {
            value
                .get("compactBytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .sum::<u64>();
    let max_compact_bytes = request_audits
        .iter()
        .filter_map(|value| value.get("compactBytes").and_then(Value::as_u64))
        .max()
        .unwrap_or(0);
    let read_field_group_count = field_audits
        .iter()
        .filter(|value| value.get("source").and_then(Value::as_str) == Some("readFieldGroup"))
        .count();
    let read_request_fields_count = field_audits
        .iter()
        .filter(|value| value.get("source").and_then(Value::as_str) == Some("readRequestFields"))
        .count();
    let serialized_audits = serde_json::to_string(&field_audits)?;
    let full_artifact_read_count = serialized_audits.matches(".loom/requests/").count()
        + serialized_audits.matches(".loom/deliveries/").count();
    let jq_count = serialized_audits.matches("jq").count();
    let mut issues = vec![];
    if request_audits.is_empty() {
        issues.push(issue(
            "REQUEST_SIZE_AUDIT_EMPTY",
            "No request size audit records were generated.",
            [],
        ));
    }
    if field_audits.is_empty() {
        issues.push(issue(
            "FIELD_READ_AUDIT_EMPTY",
            "No request field read audit records were generated.",
            [],
        ));
    }
    if full_artifact_read_count > 0 {
        issues.push(issue(
            "FULL_ARTIFACT_READ_AUDIT_NON_ZERO",
            "Field read audit contains full artifact reads.",
            [("count", json!(full_artifact_read_count))],
        ));
    }
    if jq_count > 0 {
        issues.push(issue(
            "JQ_AUDIT_NON_ZERO",
            "Field read audit contains jq reads.",
            [("count", json!(jq_count))],
        ));
    }
    if benchmark.reference_payload_bytes > 0
        && total_payload_bytes > benchmark.reference_payload_bytes
    {
        issues.push(issue(
            "PAYLOAD_BYTES_REGRESSED",
            "MCP agent-facing payload bytes exceed TypeScript reference canonical payload bytes.",
            [
                ("mcpPayloadBytes", json!(total_payload_bytes)),
                (
                    "referencePayloadBytes",
                    json!(benchmark.reference_payload_bytes),
                ),
            ],
        ));
    }
    if benchmark.mcp_completion_rate + f64::EPSILON < benchmark.reference_completion_rate {
        issues.push(issue(
            "BENCHMARK_COMPLETION_REGRESSED",
            "MCP completion rate is lower than TypeScript reference completion rate.",
            [
                ("mcpCompletionRate", json!(benchmark.mcp_completion_rate)),
                (
                    "referenceCompletionRate",
                    json!(benchmark.reference_completion_rate),
                ),
            ],
        ));
    }
    Ok(TokenReadPlanSummary {
        status: if issues.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        request_count: request_audits.len(),
        total_payload_bytes,
        max_compact_bytes,
        total_field_read_count: field_audits.len(),
        read_field_group_count,
        read_request_fields_count,
        full_artifact_read_count,
        jq_count,
        reference_payload_bytes: benchmark.reference_payload_bytes,
        mcp_completion_rate: benchmark.mcp_completion_rate,
        reference_completion_rate: benchmark.reference_completion_rate,
        issues,
    })
}

fn build_token_by_stage(project_root: &str) -> Result<Value> {
    let paths = state::paths::project_paths(project_root)?;
    let index = state::request_index::load_request_index(project_root)?;
    let request_audits = read_jsonl(&paths.request_size_audit_file)?;
    let mut request_kind_by_id = BTreeMap::new();
    for request in index.requests {
        request_kind_by_id.insert(request.request_id, request.request_kind);
    }
    let mut by_stage: BTreeMap<String, Value> = BTreeMap::new();
    for audit in request_audits {
        let request_ref = audit
            .get("requestRef")
            .and_then(Value::as_str)
            .unwrap_or("");
        let request_id = request_ref.rsplit('/').next().unwrap_or(request_ref);
        let kind = request_kind_by_id
            .get(request_id)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let entry = by_stage.entry(kind).or_insert_with(|| {
            json!({
                "requestCount": 0u64,
                "compactBytes": 0u64,
                "fullBytes": 0u64,
                "refCount": 0u64
            })
        });
        entry["requestCount"] = json!(entry["requestCount"].as_u64().unwrap_or(0) + 1);
        entry["compactBytes"] = json!(
            entry["compactBytes"].as_u64().unwrap_or(0)
                + audit
                    .get("compactBytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
        );
        entry["fullBytes"] = json!(
            entry["fullBytes"].as_u64().unwrap_or(0)
                + audit.get("fullBytes").and_then(Value::as_u64).unwrap_or(0)
        );
        entry["refCount"] = json!(
            entry["refCount"].as_u64().unwrap_or(0)
                + audit.get("refCount").and_then(Value::as_u64).unwrap_or(0)
        );
    }
    Ok(json!({
        "schemaVersion": "token-read-plan-by-stage-v1",
        "stages": by_stage,
    }))
}

fn build_setup_packaging_report(
    setup_checks: &[NamedCheck],
    package_entries: &[String],
) -> SetupPackagingReport {
    let mut issues = vec![];
    let required_setup_checks = [
        "macos-arm64",
        "linux-x64",
        "windows-x64",
        "macos-x64",
        "linux-arm64",
    ];
    for required in required_setup_checks {
        if !setup_checks
            .iter()
            .any(|check| check.name.contains(required))
        {
            issues.push(issue(
                "SETUP_MATRIX_CHECK_MISSING",
                "Setup/platform matrix is missing a required platform check.",
                [("platform", json!(required))],
            ));
        }
    }
    for check in setup_checks {
        if !check.passed {
            issues.push(issue(
                "SETUP_CHECK_FAILED",
                "Setup/platform matrix check failed.",
                [
                    ("name", json!(check.name)),
                    ("details", check.details.clone()),
                ],
            ));
        }
    }
    for required in REQUIRED_PACKAGE_ENTRIES {
        if !package_entries
            .iter()
            .any(|entry| entry.starts_with(required))
        {
            issues.push(issue(
                "RELEASE_PACKAGE_REQUIRED_ENTRY_MISSING",
                "Release package is missing a required MCP-only runtime entry.",
                [("entry", json!(required))],
            ));
        }
    }
    for forbidden in FORBIDDEN_PACKAGE_ENTRIES {
        if package_entries
            .iter()
            .any(|entry| entry.starts_with(forbidden))
        {
            issues.push(issue(
                "RELEASE_PACKAGE_FORBIDDEN_ENTRY",
                "Release package contains a forbidden legacy/product-source entry.",
                [("entry", json!(forbidden))],
            ));
        }
    }
    SetupPackagingReport {
        status: if issues.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .to_string(),
        setup_checks: setup_checks.to_vec(),
        required_entries: REQUIRED_PACKAGE_ENTRIES
            .iter()
            .map(|entry| entry.to_string())
            .collect(),
        forbidden_entries: FORBIDDEN_PACKAGE_ENTRIES
            .iter()
            .map(|entry| entry.to_string())
            .collect(),
        issues,
    }
}

fn read_jsonl(file: &Path) -> Result<Vec<Value>> {
    if !file.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(file)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn has_duplicate_context_ref_path(value: &Value) -> bool {
    let Some(context_refs) = value.get("contextRefs").and_then(Value::as_object) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    context_refs
        .values()
        .filter_map(Value::as_str)
        .any(|path| !seen.insert(path.to_string()))
}

fn write_report_json<T: Serialize>(
    file: &Path,
    value: &T,
    report_files: &mut BTreeMap<String, String>,
    key: &str,
    project_root: &Path,
) -> Result<()> {
    state::store::write_json_atomic(file, value)?;
    report_files.insert(
        key.to_string(),
        state::paths::to_project_relative(project_root, file)?,
    );
    Ok(())
}

fn write_report_text(
    file: &Path,
    value: &str,
    report_files: &mut BTreeMap<String, String>,
    key: &str,
    project_root: &Path,
) -> Result<()> {
    state::store::write_text_atomic(file, value)?;
    report_files.insert(
        key.to_string(),
        state::paths::to_project_relative(project_root, file)?,
    );
    Ok(())
}

fn benchmark_markdown(summary: &TokenReadPlanSummary, benchmark: &BenchmarkInput) -> String {
    format!(
        "# MCP-only Benchmark Summary\n\n- MCP completion rate: {:.2}\n- Reference completion rate: {:.2}\n- MCP payload bytes: {}\n- Reference payload bytes: {}\n- Full artifact reads: {}\n- jq reads: {}\n",
        benchmark.mcp_completion_rate,
        benchmark.reference_completion_rate,
        summary.total_payload_bytes,
        benchmark.reference_payload_bytes,
        summary.full_artifact_read_count,
        summary.jq_count
    )
}

fn regression_report_markdown(
    passed: bool,
    projection: &ProjectionDiff,
    protocol: &ProtocolSnapshotReport,
    transcript: &AgentTranscriptAudit,
    token: &TokenReadPlanSummary,
    setup: &SetupPackagingReport,
) -> String {
    format!(
        "# Loom MCP-only Regression Report\n\nOverall: {}\n\n| Gate | Status | Issues |\n| --- | --- | ---: |\n| Canonical projection | {} | {} |\n| Protocol snapshot | {} | {} |\n| Agent transcript audit | {} | {} |\n| Token/read-plan | {} | {} |\n| Setup packaging | {} | {} |\n\n## Batch Coverage\n\nBatches 0-12 are covered by the MCP-only runtime tests, request protocol snapshot, setup packaging audit, TypeScript reference lane, and this final verification report. Batch 13 owns this report and has release blockers below.\n\nRelease blockers: {}\n",
        if passed { "passed" } else { "failed" },
        projection.status,
        projection.issue_count,
        protocol.status,
        protocol.issues.len(),
        transcript.status,
        transcript.issues.len(),
        token.status,
        token.issues.len(),
        setup.status,
        setup.issues.len(),
        if passed { 0 } else { 1 }
    )
}

fn issue<const N: usize>(
    code: impl Into<String>,
    message: impl Into<String>,
    details: [(&str, Value); N],
) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        details: details
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    }
}

pub fn require_final_verification_passed(summary: &FinalVerificationSummary) -> Result<()> {
    if summary.passed {
        Ok(())
    } else {
        Err(anyhow!(
            "MCP-only final verification failed with {} issue(s)",
            summary.issue_count
        ))
    }
}
