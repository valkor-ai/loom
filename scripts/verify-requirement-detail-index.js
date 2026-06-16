#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");
const now = "2026-05-24T00:00:00.000Z";

function run(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope.data;
}

function projectFile(projectRoot, relativePath) {
  return path.join(projectRoot, relativePath);
}

function readJson(projectRoot, relativePath) {
  return JSON.parse(fs.readFileSync(projectFile(projectRoot, relativePath), "utf8"));
}

function hydrateRequest(projectRoot, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(projectRoot, value);
  }
  return hydrated;
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function createCandidate(request) {
  return {
    schemaVersion: "1.0",
    candidateId: "brainstorm-candidate-detail-index",
    brainstormRunId: request.brainstormRunId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "confirmed",
    requestSummary: {
      title: "Operations console",
      oneLine: "Staff process account applications with query, review, blocking, and status feedback.",
      businessGoal: "Support staff account-application operations with traceable business rules.",
      complexity: "medium",
    },
    sources: [{
      sourceId: "src-001",
      type: "user_text",
      title: "User request",
      textDigest: "sha256:test",
      extracted: true,
    }],
    scope: {
      included: [{
        id: "scope-applications",
        label: "Application operations",
        items: [
          "Business scenario: staff search account applications, select a record, review submitted fields, approve or reject the application, and see the resulting status.",
          "Decision impact: approval status and blocking reasons affect data model fields, service interfaces, page feedback, and acceptance checks.",
          "Lifecycle scan: application query/select, view details, approve/process, state change, and blocking/exception handling are in scope.",
          "Key fields: application id, applicant name, submitted identity fields, approval status, blocking reason, and updated timestamp.",
        ],
        reason: "These details define the first operational phase.",
        source: "user_confirmed",
      }],
      excluded: [],
      deferred: [],
      assumptions: [{
        id: "assumption-auth",
        text: "Staff authentication exists or is handled outside this phase.",
        requiresConfirmation: false,
      }],
    },
    roadmap: {
      required: false,
      currentPhaseId: request.phaseId,
      phases: [{
        phaseId: request.phaseId,
        title: "Application operations",
        status: "scope_confirmed",
        goal: "Deliver query, review, approval, rejection, status feedback, and blocking evidence.",
        scopeRefs: ["scope-applications"],
        acceptanceRefs: ["AC-001"],
        dependsOn: [],
      }],
    },
    phasePlan: {
      current: {
        phaseId: request.phaseId,
        title: "Application operations",
        goal: "Deliver account application operation workflow.",
        scopeRefs: ["scope-applications"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        kind: "none",
        reason: "No deferred scope remains in this test case.",
      },
    },
    acceptance: [{
      id: "AC-001",
      statement: "Staff can query applications, select one record, approve or reject it, persist the status change, and see success or blocking feedback with the blocking reason when invalid.",
      capabilityRefs: ["cap-operations"],
      sourceRefs: ["src-001"],
      priority: "must",
    }],
    domainModel: {
      actors: [{ id: "actor-staff", name: "Staff", description: "Back-office operator." }],
      capabilityGroups: [{ id: "cap-operations", name: "Application operations", description: "Query and process applications." }],
      businessFlows: [{
        id: "flow-review-application",
        name: "Review account application",
        actors: ["actor-staff"],
        capabilityRefs: ["cap-operations"],
        summary: "Business scenario confirmation: staff query and select an application before processing. Decision impacts: status and blocking reason affect data model, interface response, frontend feedback, and acceptance. Lifecycle actions: query/select, view, approve/process, state change, and blocking/exception handling. Success changes application status and refreshes visible feedback.",
      }],
    },
    userConfirmation: {
      confirmed: true,
      confirmedAt: now,
      confirmationSummary: "User confirmed scope, business scenario, decision impact, lifecycle, frontend path, and final summary.",
      confirmationBasis: {
        initialRequestOnly: false,
        summaryPresentedToUser: true,
        confirmedAfterSummary: true,
        presentedItems: [
          "currentPhaseScopeSummary",
          "includedDeferredExcludedBoundary",
          "nextPhasePreview",
          "conceptSummary",
          "businessObjectOperationSummary",
        ],
      },
    },
    conceptGrounding: {
      deliveryConceptGlossary: {
        mode: "concepts_present",
        concepts: [{
          conceptId: "concept-application-status",
          term: "Application status",
          normalizedName: "application_status",
          explanation: "Application status is the business state changed by approve or reject operations; blocking reasons explain why invalid processing cannot continue.",
          mustNotMisinterpretAs: ["A visual-only label"],
          phaseRelevance: "current",
          priority: "must_understand",
          attentionRank: 1,
          riskFactors: ["state_transition", "business_invariant"],
          scopeRefs: ["scope-applications"],
          acceptanceRefs: ["AC-001"],
          humanReadableReason: "Wrong status semantics would break processing and feedback.",
        }],
      },
      phaseConceptGrounding: {
        mode: "concepts_present",
        concepts: [{
          conceptId: "concept-review-lifecycle",
          term: "Review lifecycle",
          normalizedName: "review_lifecycle",
          explanation: "The review lifecycle covers query/select, view details, approve/process, state change, blocking reason, and visible feedback.",
          mustNotMisinterpretAs: ["Direct id-only lookup without query/select"],
          phaseRelevance: "current",
          priority: "must_understand",
          attentionRank: 1,
          riskFactors: ["user_visible_flow", "state_transition"],
          scopeRefs: ["scope-applications"],
          acceptanceRefs: ["AC-001"],
          humanReadableReason: "The lifecycle determines task, API, UI, and verification responsibilities.",
        }],
      },
      glossaryUpdates: [],
    },
    conceptConfirmation: {
      shownToUser: true,
      confirmedConceptRefs: ["concept-application-status", "concept-review-lifecycle"],
      confirmationSummary: "Concepts were shown and confirmed.",
    },
    clarificationProgress: {
      mode: "progressive_blocks",
      confirmedBlocks: ["phase_scope", "concept_grounding", "frontend_experience", "final_summary"].map((block) => ({
        block,
        summary: `User confirmed ${block}.`,
        confirmedByUser: true,
      })),
      skippedBlocks: [],
      finalSummaryConfirmed: true,
    },
    frontendExperience: {
      required: true,
      kind: "business_application",
      experienceLevel: "usable_internal_product",
      audiences: [{ audienceId: "audience-staff", name: "Staff", primaryJobs: ["Process applications"] }],
      surfaces: [{ surfaceId: "surface-applications", name: "Application workspace", audienceRefs: ["audience-staff"], primaryJobs: ["Query and process applications"] }],
      dataViews: [{
        viewId: "view-application-list",
        name: "Application list",
        purpose: "Let staff query applications, select one record, and inspect status before processing.",
        targetObject: "Application",
        selectionMode: "query_and_select",
        paginationRequired: true,
        defaultLoadsFirstPage: true,
        searchCriteria: [{ criterionId: "criterion-status", label: "Status", fieldRef: "approvalStatus", reason: "Staff needs to find pending applications.", sourceRefs: ["src-001"] }],
        sourceRefs: ["src-001"],
      }],
      actions: [{
        actionId: "action-review-application",
        label: "Approve or reject application",
        targetObject: "Application",
        entryPoint: "result_row_action",
        inputFields: ["decision", "blockingReason"],
        resultObservation: ["list_refresh", "response_message"],
        refreshPolicy: "refresh_current_query",
        successFeedback: ["Updated status is visible in the list."],
        blockingOrErrorFeedback: ["Blocking reason is shown when processing is invalid."],
        sourceRefs: ["src-001"],
      }],
      operationPaths: [{
        pathId: "path-review-application",
        name: "Query, select, and review application",
        userGoal: "Process an application with visible status feedback.",
        surfaceRef: "surface-applications",
        workflowRef: "flow-review-application",
        targetObject: "Application",
        selectionMode: "query_and_select",
        selectionSummary: "Paginated query results -> select application -> approve or reject -> refresh current query and show status or blocking reason.",
        dataViewRefs: ["view-application-list"],
        actionRefs: ["action-review-application"],
        requiredStates: ["loading", "success", "error", "empty", "business_blocking"],
        sourceRefs: ["src-001"],
      }],
      mustNot: ["direct_id_only_operation"],
      confirmationSummary: "User confirmed paginated query/select and post-action refresh feedback.",
    },
    handoff: {
      ready: true,
      nextNode: "technical_baseline_generation",
      blockingReasons: [],
    },
  };
}

function writeTechnicalBaseline(projectRoot, deliveryId) {
  writeJson(projectFile(projectRoot, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-001",
    status: "auto_accepted",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: { languages: ["typescript"], packageManagers: ["npm"], runtime: "node" },
    constraints: [],
    evidence: [{ path: "package.json", reason: "test fixture" }],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "high",
    createdAt: now,
    updatedAt: now,
  });
}

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-requirement-detail-index-"));
run(["init"], projectRoot);

const started = run(["brainstorm", "start", "--request", "Build an account application operations console."], projectRoot);
const request = {
  ...hydrateRequest(projectRoot, readJson(projectRoot, started.requestPath ?? started.requestRef)),
  deliveryId: started.deliveryId,
  phaseId: started.phaseId,
  brainstormRunId: started.brainstormRunId,
  requestId: started.requestId,
};
writeJson(projectFile(projectRoot, request.outputContract.candidateFile), createCandidate(request));
run([
  "brainstorm", "accept",
  "--delivery-id", request.deliveryId,
  "--phase-id", request.phaseId,
  "--run-id", request.brainstormRunId,
  "--request-id", request.requestId,
  "--candidate-file", request.outputContract.candidateFile,
], projectRoot);

writeTechnicalBaseline(projectRoot, request.deliveryId);
const pgc = run(["planning-contract", "create", "--delivery-id", request.deliveryId, "--phase-id", request.phaseId], projectRoot);
const details = pgc.contract.requirementDetails;

assert.equal(details.schemaVersion, "1.0");
assert.equal(details.authority, "brainstorm_contract");
assert.ok(details.sourceBrainstormContractRef.endsWith("/brainstorms/contract.json"));
assert.ok(details.items.length >= 8, "PGC must extract multiple requirement detail items from Brainstorm fields.");
assert.ok(details.items.every((item) => item.detailId.startsWith("detail-")), "Every detail item must have a stable detailId.");
assert.ok(details.items.every((item) => item.sourceFieldRefs.length > 0), "Every detail item must point back to Brainstorm source fields.");

assert.ok(
  details.items.some((item) => item.summary.includes("Business scenario") && item.impactTags.includes("business_flow")),
  "Detail index must preserve business scenario confirmation.",
);
assert.ok(
  details.items.some((item) => item.summary.includes("Decision impact") && item.impactTags.includes("data_model")),
  "Detail index must preserve decision impact ordering.",
);
assert.ok(
  details.items.some((item) => item.summary.includes("Lifecycle scan") && item.lifecycleStage !== "not_applicable"),
  "Detail index must preserve lifecycle scan details.",
);
assert.ok(
  details.items.some((item) => item.kind === "frontend_operation_path" && item.frontendRefs.includes("path-review-application")),
  "Detail index must preserve frontend operation paths.",
);
assert.ok(
  details.items.some((item) => item.conceptRefs.includes("concept-review-lifecycle")),
  "Detail index must preserve concept refs for concept-derived details.",
);

console.log("Requirement detail index verification passed.");
