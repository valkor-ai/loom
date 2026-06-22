#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cli = path.join(repoRoot, "dist", "cli.js");

function now() {
  return new Date().toISOString();
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function run(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope.data;
}

function fieldValue(inspectData, field) {
  assert.ok(inspectData?.fields?.[field], `inspect response must include ${field}`);
  return inspectData.fields[field].value;
}

function writeFrontendDeliveryFixture(root) {
  const deliveryId = "delivery-frontend-guidance";
  const phaseId = "phase-1";
  const aacRef = `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`;
  const taskPlanRef = `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/taskplan-frontend.json`;
  const taskPlanRunRef = `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-frontend.json`;

  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "executing",
      requestSummary: "Verify frontend execution guidance.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: {
      type: "continue_execution",
      deliveryId,
      phaseId,
      reason: "TASKPLAN_READY",
      targetNode: "task_execution",
    },
    phase: "building",
    current: {
      requirementId: null,
      planId: null,
      taskId: null,
      reviewId: null,
      repairId: null,
      deploymentId: null,
    },
    lastAction: null,
    nextAction: "next-task",
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "executing",
    requestSummary: "Verify frontend execution guidance.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "ready_for_execution",
      latestRefs: {
        taskPlan: taskPlanRef,
        taskPlanRun: taskPlanRunRef,
      },
      nextAction: {
        type: "continue_execution",
        deliveryId,
        phaseId,
        reason: "TASKPLAN_READY",
        targetNode: "task_execution",
      },
    }],
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-frontend",
    status: "confirmed",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: { runtime: "node", language: "typescript", packageManager: "npm" },
    constraints: [],
    evidence: [{ path: "package.json", reason: "fixture" }],
    approval: { type: "policy_auto_accept", reason: "fixture" },
    confidence: "high",
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-frontend",
    status: "ready",
    source: {
      brainstormRunId: "brainstorm-frontend",
      brainstormContractId: "brainstorm-contract-frontend",
      roadmapId: null,
      phaseId,
      technicalBaselineId: "tb-frontend",
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Deliver a usable booking surface.",
      included: [{ scopeId: "scope-booking", label: "Booking UI", items: ["create booking"], source: "fixture" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-booking", statement: "User can submit a booking from the web UI.", priority: "must" }],
    },
    technicalBaseline: {
      technicalBaselineId: "tb-frontend",
      status: "confirmed",
      scope: "project",
      summary: { runtime: "node" },
      mustFollow: true,
    },
    planningInputs: {
      businessGoal: "Deliver a usable booking surface.",
      actors: [],
      capabilityGroups: [],
      businessFlows: [],
      sourceRefs: [],
      contextNotes: [],
    },
    planningRules: {
      scopeIsolation: {
        onlyPlanCurrentPhase: true,
        forbidDeferredScopeImplementation: true,
        forbidFuturePhaseImplementation: true,
      },
      outputRequirements: {
        mustCreateArchitectureArtifactContract: true,
        mustCreateTaskPlan: true,
        taskPlanMustReferenceAcceptance: true,
      },
      deployment: { defaultEnabled: false, requiresExplicitUserRequest: true },
    },
    qualityGates: {
      requiresArchitectureBeforeTaskPlan: true,
      requiresAcceptanceCoverage: true,
      requiresVerificationEvidence: true,
    },
    handoff: { readyForArchitecture: true, readyForTaskPlan: true, blockingReasons: [], nextNode: "architecture_artifact_contract" },
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-frontend",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });

  writeJson(projectFile(root, aacRef), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-frontend",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-frontend",
      technicalBaselineId: "tb-frontend",
      brainstormContractId: "brainstorm-contract-frontend",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-web", type: "web", root: "." }],
      modules: [{ moduleId: "module-booking-ui", appId: "app-web", paths: ["src"], responsibility: "Booking UI." }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{
      moduleId: "module-booking-ui",
      name: "Booking UI",
      responsibility: "Let users create bookings.",
      dependsOn: [],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
    }],
    dataModel: {
      entities: [{
        entityId: "entity-booking",
        name: "Booking",
        type: "internal",
        implementationIntent: "full",
        moduleRefs: ["module-booking-ui"],
        scopeRefs: ["scope-booking"],
        acceptanceRefs: ["AC-booking"],
        fields: [{ fieldId: "field-customer-name", name: "customerName", type: "string", required: true }],
        constraints: [{ constraintId: "constraint-booking-customer-required", type: "validation", description: "Customer name is required before booking submission." }],
      }],
      relationships: [],
      constraints: [{
        constraintId: "constraint-booking-submit-valid",
        type: "business_rule",
        description: "A booking submission must include valid customer details before it can be accepted.",
        entityRefs: ["entity-booking"],
        acceptanceRefs: ["AC-booking"],
      }],
    },
    interfaces: [{
      interfaceId: "if-booking-form",
      name: "BookingForm",
      type: "component",
      role: "component_binding",
      moduleRefs: ["module-booking-ui"],
      entityRefs: ["entity-booking"],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
    }, {
      interfaceId: "if-create-booking",
      name: "Create booking API",
      type: "http_api",
      role: "command",
      moduleRefs: ["module-booking-ui"],
      entityRefs: ["entity-booking"],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
      method: "POST",
      path: "/api/bookings",
      requestSchema: [{ fieldId: "field-customer-name", name: "customerName", type: "string", required: true }],
      responseSchema: [{ fieldId: "field-booking-id", name: "bookingId", type: "string", required: true }],
      errorSchema: [{ fieldId: "field-error-message", name: "message", type: "string", required: true }],
    }],
    userFlows: [{
      flowId: "flow-submit-booking",
      name: "Submit booking",
      kind: "user_interaction",
      moduleRefs: ["module-booking-ui"],
      interfaceRefs: ["if-booking-form", "if-create-booking"],
      entityRefs: ["entity-booking"],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
      entry: { type: "page", ref: "surface-booking", label: "Booking" },
      steps: [{
        stepId: "step-submit",
        actor: "user",
        action: "Fill booking details and submit",
        systemResponse: "Booking result appears",
        interfaceRefs: ["if-create-booking"],
        stateMachineRefs: ["sm-booking"],
      }],
      outcomes: [{ type: "success", description: "Booking is accepted." }],
    }],
    stateMachines: [{
      stateMachineId: "sm-booking",
      name: "Booking submission state",
      entityRef: "entity-booking",
      entityRefs: ["entity-booking"],
      moduleRefs: ["module-booking-ui"],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
      states: [
        { stateId: "draft", name: "Draft", terminal: false },
        { stateId: "submitted", name: "Submitted", terminal: true },
      ],
      initialState: "draft",
      events: [{ eventId: "submit", name: "Submit booking" }],
      transitions: [{
        transitionId: "transition-submit-booking",
        from: "draft",
        to: "submitted",
        event: "submit",
        guards: ["Customer name is present."],
        effects: ["Booking result is rendered."],
      }],
      rules: [{ ruleId: "rule-booking-valid-before-submit", description: "Submission is blocked until required booking fields are valid.", acceptanceRefs: ["AC-booking"] }],
    }],
    frontendExperience: {
      required: true,
      kind: "web_app",
      experienceLevel: "usable_internal_product",
      surfaces: [{
        surfaceId: "surface-booking",
        name: "Booking",
        purpose: "Collect booking details and show submission result.",
        userRoleRefs: ["user"],
        workflowRefs: ["flow-submit-booking"],
        moduleRefs: ["module-booking-ui"],
      }],
      dataViews: [{
        viewId: "view-booking-form",
        name: "Booking form",
        purpose: "Let users enter booking details before submitting.",
        targetObject: "Booking",
        selectionMode: "not_applicable",
        paginationRequired: false,
        defaultLoadsFirstPage: false,
        searchCriteria: [],
        sourceRefs: ["AC-booking"],
      }],
      actions: [{
        actionId: "action-submit-booking",
        label: "Submit booking",
        targetObject: "Booking",
        entryPoint: "form_submit",
        inputFields: ["customerName"],
        resultObservation: ["response_message", "detail_refresh"],
        refreshPolicy: "refresh_detail",
        successFeedback: ["Booking result appears"],
        blockingOrErrorFeedback: ["Customer name is required"],
        sourceRefs: ["AC-booking"],
      }],
      operationPaths: [{
        pathId: "path-submit-booking",
        name: "Submit booking path",
        userGoal: "Submit a booking from the web UI and see the result.",
        surfaceRef: "surface-booking",
        workflowRef: "flow-submit-booking",
        targetObject: "Booking",
        selectionMode: "not_applicable",
        selectionSummary: "User opens the booking form, enters details, submits, and sees a refreshed booking result or blocking message.",
        dataViewRefs: ["view-booking-form"],
        actionRefs: ["action-submit-booking"],
        requiredStates: ["loading", "success", "error", "business_blocking"],
        sourceRefs: ["AC-booking"],
      }],
      navigation: {
        required: true,
        pattern: "single_page_primary_action",
        items: [{ label: "Booking", targetSurfaceRef: "surface-booking" }],
      },
      interactionStates: ["idle", "loading", "success", "error", "empty", "business_blocking"],
      mustNot: ["Do not deliver only descriptive static text for the booking workflow."],
      notes: ["Fixture frontend contract."],
    },
    acceptanceMatrix: [{
      acceptanceId: "AC-booking",
      priority: "must",
      statement: "User can submit a booking from the web UI.",
      coverageStatus: "covered",
      coverage: [{ type: "user_flow", refs: ["flow-submit-booking"], description: "Booking UI flow." }],
      verificationHints: [{ kind: "manual", description: "Submit the booking form." }],
    }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-frontend",
    contractRef: aacRef,
    planningContractId: "pgc-frontend",
    updatedAt: now(),
  });

  writeJson(projectFile(root, taskPlanRef), {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-frontend",
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId,
      planningGenerationContractId: "pgc-frontend",
      architectureArtifactContractId: "aac-frontend",
      technicalBaselineId: "tb-frontend",
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-booking"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-booking"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: [{
      groupId: "group-frontend",
      title: "Frontend task",
      objective: "Deliver usable booking UI.",
      dependsOn: [],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
      taskIds: ["task-booking-ui"],
    }],
    tasks: [{
      taskId: "task-booking-ui",
      groupId: "group-frontend",
      title: "Implement booking UI flow",
      taskKind: "ui_flow_increment",
      implementationActions: ["create_or_update_ui_flow", "wire_reference_in_api_or_ui", "add_or_update_tests"],
      objective: "Implement the user-visible booking submission flow.",
      dependsOn: [],
      scopeRefs: ["scope-booking"],
      acceptanceRefs: ["AC-booking"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: ["module-booking-ui"],
          entities: [],
          interfaces: ["if-create-booking"],
          userFlows: ["flow-submit-booking"],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "VI-booking-ui",
        acceptanceRefs: ["AC-booking"],
        behavior: "Booking UI accepts input and shows a result.",
        preferredEvidence: ["automated_test", "runtime_api_check"],
        acceptableEvidence: ["automated_test", "runtime_api_check", "manual_command_output"],
      }],
      frontendExperienceRequirement: {
        frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
        experienceLevel: "usable_internal_product",
        mustSatisfy: ["surface fits frontendExperience.surfaces"],
        executionGuidance: {
          legacySmallHint: "preserve non-payload guidance while replacing generated guidance",
          workflowClosureRequirements: [{
            closureId: "legacy-full-payload-should-not-survive",
            workflowRef: "flow-submit-booking",
            interfaces: [{
              interfaceId: "if-create-booking",
              requestSchema: [{ fieldId: "legacy-request", name: "legacyRequest", type: "string", required: true }],
              responseSchema: [{ fieldId: "legacy-response", name: "legacyResponse", type: "string", required: true }],
              errorSchema: [{ fieldId: "legacy-error", name: "legacyError", type: "string", required: true }],
            }],
          }],
        },
      },
    }],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/latest.json`), {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-frontend",
    taskPlanRef,
    updatedAt: now(),
  });

  writeJson(projectFile(root, taskPlanRunRef), {
    schemaVersion: "1.0",
    runId: "run-frontend",
    taskPlanId: "taskplan-frontend",
    status: "not_started",
    scheduler: { mode: "group_dag", startedAt: null, finishedAt: null },
    groupStates: [{
      groupId: "group-frontend",
      status: "pending",
      startedAt: null,
      finishedAt: null,
      dependsOn: [],
      taskIds: ["task-booking-ui"],
    }],
    taskStates: [{
      taskId: "task-booking-ui",
      groupId: "group-frontend",
      status: "pending",
      resultId: null,
      startedAt: null,
      finishedAt: null,
      dependsOn: [],
      attempts: [],
    }],
    summary: { total: 1, completed: 0, completedWithNotes: 0, blocked: 0, failed: 0, pending: 1, running: 0 },
    nextAction: { type: "continue_execution", reason: "TASKPLAN_READY", targetNode: "task_execution" },
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/latest.json`), {
    schemaVersion: "1.0",
    taskPlanRunId: "run-frontend",
    runRef: taskPlanRunRef,
    taskPlanId: "taskplan-frontend",
    updatedAt: now(),
  });

  return { deliveryId, phaseId };
}

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-frontend-guidance-"));
try {
  fs.writeFileSync(projectFile(projectRoot, "package.json"), JSON.stringify({ type: "module" }, null, 2));
  run(["init"], projectRoot);
  const { deliveryId, phaseId } = writeFrontendDeliveryFixture(projectRoot);
  const next = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], projectRoot);
  assert.equal(next.hasTask, true);
  assert.equal(next.task.taskId, "task-booking-ui");
  assert.ok(next.executionRequestPath, "next-task must return executionRequestPath.");

  const taskRead = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "task",
  ], projectRoot);
  const task = fieldValue(taskRead, "task");
  assert.deepEqual(task.writeBoundary.artifactRefs.interfaces, ["if-create-booking"], "fixture frontend task owns the backend API interface ref for workflow closure.");
  const guidance = task.frontendExperienceRequirement?.executionGuidance;
  assert.equal(guidance?.schemaVersion, "1.0");
  assert.equal(guidance?.responsibility, "workflow_implementation");
  assert.deepEqual(guidance?.surfacesInScope?.map((surface) => surface.surfaceId), ["surface-booking"]);
  assert.deepEqual(guidance?.workflowsInScope?.map((workflow) => workflow.workflowRef), ["flow-submit-booking"]);
  assert.deepEqual(guidance?.dataViewsInScope?.map((view) => view.viewId), ["view-booking-form"]);
  assert.deepEqual(guidance?.actionsInScope?.map((action) => action.actionId), ["action-submit-booking"]);
  assert.deepEqual(guidance?.operationPathsInScope?.map((operationPath) => operationPath.pathId), ["path-submit-booking"]);
  assert.equal(guidance?.operationPathsInScope?.[0]?.selectionSummary.includes("booking form"), true);
  assert.equal(guidance?.operationPathWarnings?.length, 0);
  assert.deepEqual(guidance?.dataBindingExpectation?.interfacesInScope, ["if-create-booking", "if-booking-form"]);
  assert.deepEqual(
    guidance?.dataBindingExpectation?.interfaceRolesInScope?.map((item) => [item.interfaceRef, item.role]),
    [["if-create-booking", "command"], ["if-booking-form", "component_binding"]],
  );
  assert.equal(guidance?.dataBindingExpectation?.requiredModeForSatisfaction, "wired");
  assert.deepEqual(guidance?.dataBindingExpectation?.closureRequirementIds, ["closure:flow-submit-booking:step-submit"]);
  assert.equal(guidance?.legacySmallHint, "preserve non-payload guidance while replacing generated guidance", "frontend guidance should preserve non-payload existing guidance.");
  assert.equal(guidance?.workflowClosureRequirements, undefined, "frontend guidance must not duplicate full workflow closure requirements.");
  assert.equal(guidance?.closureRequirementRefs?.length, 1, "frontend guidance must project lightweight workflow closure refs for assigned closure tasks.");
  assert.equal(guidance.closureRequirementRefs[0].workflowRef, "flow-submit-booking");
  assert.deepEqual(guidance.closureRequirementRefs[0].operationPathRefs, ["path-submit-booking"]);
  assert.deepEqual(guidance.closureRequirementRefs[0].dataViewRefs, ["view-booking-form"]);
  assert.deepEqual(guidance.closureRequirementRefs[0].actionRefs, ["action-submit-booking"]);
  assert.deepEqual(guidance.closureRequirementRefs[0].interfaceRefs, ["if-create-booking"]);
  assert.equal(guidance.closureRequirementRefs[0].requiredDataBindingMode, "wired");
  assert.equal(guidance.closureRequirementRefs[0].interfaces, undefined, "closure refs must not duplicate full interface schemas.");
  assert.deepEqual(guidance?.workflowClosureDetailSource?.closureRequirementIds, ["closure:flow-submit-booking:step-submit"]);
  assert.equal(guidance?.bindingProjectionRules?.apiContractAuthority, "AAC global interfaces. Task scope and workflow refs only select the most relevant bindings.");
  assert.equal(guidance?.bindingProjectionRules?.bindingsAreNotAllowlist, true);
  assert.equal(guidance?.frontendBackendBindings?.length, 1, "frontend guidance must project step-level API binding.");
  const binding = guidance.frontendBackendBindings[0];
  assert.equal(binding.workflowRef, "flow-submit-booking");
  assert.equal(binding.stepRef, "step-submit");
  assert.equal(binding.userAction, "Fill booking details and submit");
  assert.equal(binding.interfaces.length, 1);
  assert.equal(binding.interfaces[0].interfaceId, "if-create-booking");
  assert.equal(binding.interfaces[0].role, "command");
  assert.equal(binding.interfaces[0].method, "POST");
  assert.equal(binding.interfaces[0].path, "/api/bookings");
  assert.deepEqual(binding.interfaces[0].requestSchema, [{ fieldId: "field-customer-name", name: "customerName", type: "string", required: true }]);
  assert.deepEqual(binding.interfaces[0].responseSchema, [{ fieldId: "field-booking-id", name: "bookingId", type: "string", required: true }]);
  assert.ok(binding.uiResponsibilities.beforeRequest.length > 0, "binding must tell agents how to close the user action before request.");
  assert.ok(binding.uiResponsibilities.duringRequest.length > 0, "binding must include loading-state responsibility when declared.");
  assert.ok(binding.uiResponsibilities.onSuccess[0].includes("Booking result appears"), "binding must carry the userFlow system response.");
  assert.ok(binding.uiResponsibilities.onError.length > 0, "binding must include error-state responsibility when declared.");
  assert.deepEqual(guidance?.unresolvedBindingInputs, []);
  assert.equal(guidance?.executionGuidanceIsNonBlocking, undefined, "non-blocking flag belongs to executionRules, not task guidance.");

  const readPlan = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "agentAction.read.required",
  ], projectRoot);
  const requiredReads = fieldValue(readPlan, "agentAction.read.required");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"), "agent read plan must name frontendBackendBindings.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.unresolvedBindingInputs"), "agent read plan must name unresolved binding inputs.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"), "agent read plan must name workflow closure refs.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource"), "agent read plan must name workflow closure detail source.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.operationPathsInScope"), "agent read plan must name operation paths.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.dataViewsInScope"), "agent read plan must name data views.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.actionsInScope"), "agent read plan must name frontend actions.");
  assert.ok(requiredReads.includes("task.frontendExperienceRequirement.executionGuidance.dataBindingExpectation"), "agent read plan must name data-binding expectations for closure tasks.");
  assert.ok(requiredReads.includes("sourceContext.architectureArtifactProjection"), "agent read plan must require task-scoped AAC projection.");

  const projectionRead = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "sourceContext.architectureArtifactProjection",
  ], projectRoot);
  const projection = fieldValue(projectionRead, "sourceContext.architectureArtifactProjection");
  assert.equal(projection.projectionCompleteness?.includesTaskRelevantDescriptions, true, "AAC projection must declare task-relevant detail completeness.");
  assert.deepEqual(projection.interfaceContracts?.find((item) => item.interfaceId === "if-create-booking")?.requestSchema, [{ fieldId: "field-customer-name", name: "customerName", type: "string", required: true }]);
  assert.equal(projection.interfaceContracts?.find((item) => item.interfaceId === "if-create-booking")?.role, "command");
  assert.equal(projection.userFlowDetails?.[0]?.steps?.[0]?.systemResponse, "Booking result appears", "AAC projection must include user flow step response.");
  assert.equal(projection.userFlowDetails?.[0]?.outcomes?.[0]?.description, "Booking is accepted.", "AAC projection must include user flow outcomes.");
  assert.equal(projection.frontendOperationPathDetails?.operationPaths?.[0]?.pathId, "path-submit-booking", "AAC projection must include task-relevant operation paths.");
  assert.equal(projection.frontendOperationPathDetails?.dataViews?.[0]?.viewId, "view-booking-form", "AAC projection must include task-relevant data views.");
  assert.equal(projection.frontendOperationPathDetails?.actions?.[0]?.actionId, "action-submit-booking", "AAC projection must include task-relevant frontend actions.");
  assert.equal(projection.entityDetails?.[0]?.constraints?.[0]?.description, "Customer name is required before booking submission.", "AAC projection must include entity constraints.");
  assert.equal(projection.dataConstraints?.[0]?.description, "A booking submission must include valid customer details before it can be accepted.", "AAC projection must include global data constraints.");
  assert.equal(projection.stateMachineDetails?.[0]?.transitions?.[0]?.guards?.[0], "Customer name is present.", "AAC projection must include state transition guards.");
  assert.equal(projection.stateMachineDetails?.[0]?.transitions?.[0]?.effects?.[0], "Booking result is rendered.", "AAC projection must include state transition effects.");
  assert.equal(projection.stateMachineDetails?.[0]?.rules?.[0]?.description, "Submission is blocked until required booking fields are valid.", "AAC projection must include state machine rules.");

  const ruleRead = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "executionRules.frontendExperienceExecutionRules",
  ], projectRoot);
  const frontendRules = fieldValue(ruleRead, "executionRules.frontendExperienceExecutionRules");
  assert.equal(frontendRules.executionGuidanceField, "task.frontendExperienceRequirement.executionGuidance");
  assert.equal(frontendRules.frontendBackendBindingsField, "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings");
  assert.equal(frontendRules.unresolvedBindingInputsField, "task.frontendExperienceRequirement.executionGuidance.unresolvedBindingInputs");
  assert.equal(frontendRules.closureRequirementRefsField, "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs");
  assert.equal(frontendRules.workflowClosureDetailSourceField, "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource");
  assert.equal(frontendRules.operationPathsField, "task.frontendExperienceRequirement.executionGuidance.operationPathsInScope");
  assert.equal(frontendRules.dataViewsField, "task.frontendExperienceRequirement.executionGuidance.dataViewsInScope");
  assert.equal(frontendRules.actionsField, "task.frontendExperienceRequirement.executionGuidance.actionsInScope");
  assert.equal(frontendRules.useFrontendBackendBindingsFirst, true);
  assert.equal(frontendRules.frontendBackendBindingsAreNotAllowlist, true);
  assert.equal(frontendRules.executionGuidanceIsNonBlocking, true);
  assert.equal(frontendRules.workflowClosureRequired, true);
  assert.deepEqual(frontendRules.workflowClosureRequirementIds, ["closure:flow-submit-booking:step-submit"]);
  assert.ok(
    frontendRules.implementationOrganizationRules.some((rule) => rule.includes("add it as a new reachable entry")),
    "frontend execution rules must tell agents to add new modules as entries instead of replacing existing app-shell entries.",
  );

  const probePolicyRead = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "executionRules.interactiveVerificationProbePolicy",
  ], projectRoot);
  const probePolicy = fieldValue(probePolicyRead, "executionRules.interactiveVerificationProbePolicy");
  assert.ok(probePolicy, "executionRules must expose interactiveVerificationProbePolicy.");
  assert.ok(
    probePolicy.deriveProbePlanFrom.includes("task.verificationIntents[].behavior"),
    "probe policy must derive from task verification intents.",
  );
  assert.ok(
    probePolicy.deriveProbePlanFrom.includes("task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"),
    "probe policy must derive from frontendBackendBindings.",
  );
  assert.ok(
    probePolicy.deriveProbePlanFrom.includes("task.runtimeDeliveryRequirement.requiredCodeLevelChecks"),
    "probe policy must derive from runtimeDeliveryRequirement checks.",
  );
  assert.ok(
    probePolicy.requiredExecutionPattern.some((rule) => rule.includes("Select only probes that match the current task responsibility")),
    "probe policy must make probe selection task-scoped and dynamic.",
  );
  assert.ok(
    probePolicy.requiredExecutionPattern.some((rule) => rule.includes("Each probe must verify exactly one interaction target")),
    "probe policy must enforce one interaction target per probe.",
  );
  assert.ok(
    probePolicy.requiredExecutionPattern.some((rule) => rule.includes("Do not bundle multiple business workflows into one browser/e2e script")),
    "probe policy must prevent long multi-workflow browser/e2e scripts.",
  );
  assert.ok(
    probePolicy.requiredExecutionPattern.some((rule) => rule.includes("cover every verification intent assigned to the current task")),
    "probe policy must preserve verification coverage quality.",
  );
  assert.ok(
    probePolicy.failureProgressRule.some((rule) => rule.includes("same failure signature repeats without new observable evidence")),
    "probe policy must stop only repeated no-progress failure loops.",
  );

  const selfCheckRead = run([
    "inspect",
    "--request",
    next.executionRequestPath,
    "--field",
    "outputContract.schemaShape.frontendExperienceSelfCheck",
  ], projectRoot);
  const selfCheckShape = fieldValue(selfCheckRead, "outputContract.schemaShape.frontendExperienceSelfCheck");
  assert.ok(selfCheckShape.workflowsCovered, "frontend self-check shape must guide workflow evidence.");
  assert.ok(selfCheckShape.dataViewsUsed, "frontend self-check shape must guide data view evidence.");
  assert.ok(selfCheckShape.actionsImplemented, "frontend self-check shape must guide frontend action evidence.");
  assert.ok(selfCheckShape.operationPathsCovered, "frontend self-check shape must guide operation path evidence.");
  assert.ok(selfCheckShape.userActionsImplemented, "frontend self-check shape must guide user action evidence.");
  assert.ok(selfCheckShape.dataBinding, "frontend self-check shape must guide data-binding evidence.");
  assert.equal(selfCheckShape.dataBinding.requiredModeForSatisfaction, "wired", "frontend self-check shape must require wired mode for closure satisfaction.");
  assert.deepEqual(selfCheckShape.dataBinding.closureRequirementIds, ["closure:flow-submit-booking:step-submit"]);
  assert.deepEqual(selfCheckShape.closureRequirementIds, ["closure:flow-submit-booking:step-submit"], "frontend self-check shape must expose closure ids without full payload.");
  assert.equal(selfCheckShape.workflowClosureRequirements, undefined, "frontend self-check shape must not duplicate workflow closure requirements.");
  assert.ok(selfCheckShape.knownGaps, "frontend self-check shape must guide known gap recording.");
  assert.equal(
    Object.hasOwn(selfCheckShape, "browserVerification"),
    false,
    "frontend self-check must not add browser/tool-specific result fields.",
  );
} finally {
  fs.rmSync(projectRoot, { recursive: true, force: true });
}

console.log("frontend execution guidance verification passed");
