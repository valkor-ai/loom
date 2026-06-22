#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");

const { runCli } = require("../harness/cli");
const { buildDist } = require("../harness/root");
const { projectFile, readJson, requestFromCommand, tempProject, writeJson } = require("../harness/files");

function git(args, cwd) {
  return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeDeliveryAtPhaseHandoff(root) {
  const deliveryId = "delivery-phase-handoff";
  const requirementContextRef = `.loom/deliveries/${deliveryId}/requirements/context.json`;
  const normalizedRequirementTextRef = `.loom/deliveries/${deliveryId}/requirements/normalized.txt`;
  const keywordHintsRef = `.loom/deliveries/${deliveryId}/requirements/keyword-hints.json`;
  fs.mkdirSync(projectFile(root, `.loom/deliveries/${deliveryId}/requirements`), { recursive: true });
  fs.writeFileSync(projectFile(root, normalizedRequirementTextRef), "Verify git checkpoint advisory at phase handoff.\n");
  writeJson(projectFile(root, requirementContextRef), {
    schemaVersion: "1.0",
    deliveryId,
    createdAt: now(),
    sourceItems: [{
      itemId: "req-001",
      kind: "text",
      origin: "user_message",
      textRef: normalizedRequirementTextRef,
      extractionStatus: "completed",
      digest: "sha256:phase-handoff",
      characterCount: 49,
    }],
    normalizedTextRef: normalizedRequirementTextRef,
    normalizedTextStatus: "completed",
    keywordHintsRef,
    keywordHintsStatus: "empty",
    keywordHintsReason: "Fixture has no keyword hints.",
  });
  writeJson(projectFile(root, keywordHintsRef), {
    schemaVersion: "1.0",
    deliveryId,
    usage: "advisory_only",
    status: "empty",
    globalKeywords: [],
    sectionKeywords: [],
    rules: { mustNotTreatAsScope: true, mustNotTreatAsAcceptance: true, mustNotTreatAsConfirmedConcept: true },
    generatedAt: now(),
  });
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Verify git checkpoint advisory at phase handoff.",
      activePhaseId: "phase-1",
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: {
      type: "continue_to_next_phase",
      source: "review_result",
      deliveryId,
      phaseId: "phase-1",
      reason: "REVIEW_APPROVED_NEXT_PHASE",
      targetNode: "continue_to_next_phase",
      targetPhaseId: "phase-2",
    },
    phase: "planning",
    current: {
      requirementId: null,
      planId: null,
      taskId: null,
      reviewId: null,
      repairId: null,
      deploymentId: null,
    },
    lastAction: null,
    nextAction: "plan",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Verify git checkpoint advisory at phase handoff.",
    roadmapId: "roadmap-handoff",
    activePhaseId: "phase-1",
    phases: [
      {
        phaseId: "phase-1",
        name: "Phase 1",
        status: "completed",
        latestRefs: {},
        nextAction: {
          type: "continue_to_next_phase",
          source: "review_result",
          deliveryId,
          phaseId: "phase-1",
          reason: "REVIEW_APPROVED_NEXT_PHASE",
          targetNode: "continue_to_next_phase",
          targetPhaseId: "phase-2",
        },
      },
      {
        phaseId: "phase-2",
        name: "Phase 2",
        status: "pending",
        latestRefs: {},
        nextAction: null,
      },
    ],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`), {
    schemaVersion: "1.0",
    contractId: "brainstorm-contract-handoff",
    brainstormRunId: "brainstorm-phase-1",
    status: "confirmed",
    sources: [{ sourceId: "src-001", type: "text", title: "handoff fixture" }],
    summary: {
      title: "Phase handoff",
      oneLine: "Verify phase handoff advisory.",
      businessGoal: "Continue from phase 1 to phase 2.",
      complexity: "large",
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    scope: {
      included: [{ id: "scope-phase-1", label: "Phase 1", items: ["done"], source: "user_confirmed" }],
      deferred: [{ id: "scope-phase-2", label: "Phase 2", items: ["next"], source: "user_confirmed" }],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-001", statement: "Phase 1 complete.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    deliveryStrategy: {
      mode: "roadmap",
      reason: "Multi-phase fixture.",
      recommendedCurrentPhaseId: "phase-1",
    },
    deliveryContext: {
      originalRequest: {
        text: "Verify phase handoff advisory.",
        inputRefs: ["src-001"],
      },
      initialSummary: {
        title: "Phase handoff",
        oneLine: "Verify phase handoff advisory.",
        businessGoal: "Continue from phase 1 to phase 2.",
        complexity: "large",
      },
    },
    clarification: {
      status: "confirmed",
      turns: [],
      questions: [],
      answers: [],
      patches: [],
      confirmations: [],
      pendingQuestionIds: [],
      pendingConfirmationIds: [],
    },
    roadmap: {
      roadmapId: "roadmap-handoff",
      status: "active",
      strategy: "multi_phase",
      reason: "Continue to phase 2.",
      currentPhaseId: "phase-1",
      recommendedPhaseId: "phase-2",
      phases: [
        {
          phaseId: "phase-1",
          name: "Phase 1",
          status: "delivered",
          goal: "Completed phase.",
          scope: {
            includedRefs: ["scope-phase-1"],
            deferredRefs: ["scope-phase-2"],
            excludedRefs: [],
          },
          acceptanceRefs: ["AC-001"],
          dependsOn: [],
          handoff: {
            readyForPlanning: false,
            planningContractId: null,
            planId: null,
          },
          confirmation: {
            confirmedBy: "user",
            confirmedAt: now(),
            sourcePatchIds: [],
          },
          nextActions: [],
        },
        {
          phaseId: "phase-2",
          name: "Phase 2",
          status: "proposed",
          goal: "Confirm the next phase scope.",
          scope: {
            includedRefs: ["scope-phase-2"],
            deferredRefs: [],
            excludedRefs: [],
          },
          acceptanceRefs: [],
          dependsOn: ["phase-1"],
          handoff: {
            readyForPlanning: false,
            planningContractId: null,
            planId: null,
          },
          confirmation: {
            confirmedBy: null,
            confirmedAt: null,
            sourcePatchIds: [],
          },
          nextActions: [],
        },
      ],
      nextActions: [],
    },
    phasePlan: {
      current: {
        phaseId: "phase-1",
        title: "Phase 1",
        goal: "Completed phase.",
        scopeRefs: ["scope-phase-1"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        kind: "candidate",
        suggestedPhaseId: "phase-2",
        title: "Phase 2",
        goal: "Confirm the next phase scope.",
        scopePreview: ["next"],
        reason: "Phase 2 remains after phase 1.",
      },
    },
    handoff: {
      ready: true,
      nextNode: "planning_generation_contract",
      blockingReasons: [],
      confirmedAt: now(),
    },
    createdAt: now(),
    updatedAt: now(),
  });
  return deliveryId;
}

function writeTechnicalBaseline(root, deliveryId) {
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-handoff",
    status: "auto_accepted",
    source: "agent_recommended_for_greenfield",
    projectKind: "greenfield",
    scope: "roadmap",
    stack: {
      languages: ["typescript"],
      frameworks: [],
      packageManagers: ["npm"],
      runtime: "node",
      testFrameworks: [],
      buildTools: [],
    },
    constraints: [],
    evidence: [],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "medium",
    createdAt: now(),
    updatedAt: now(),
  });
}

function createRepositoryContextCandidate(requestData, root) {
  const request = requestFromCommand(requestData, root);
  return {
    schemaVersion: "1.0",
    repositoryContextId: "repoctx-handoff",
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    source: {
      requestRef: requestData.requestRef,
      brainstormContractRef: request.source.brainstormContractRef,
      technicalBaselineRef: request.source.technicalBaselineRef,
    },
    requestLens: {
      projectKind: request.projectKind,
      baselineProjectKind: request.projectKind,
      repositoryMode: request.repositoryMode,
      phaseDevelopmentMode: request.phaseDevelopmentMode,
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Repository after phase 1.",
      repositoryShape: "single_package",
      primaryApplications: [{ applicationId: "app-main", name: "Main", kind: "library", rootPath: "." }],
    },
    technologySignals: { primaryLanguages: ["typescript"], frameworks: [], packageManagers: ["npm"], buildCommands: [], testCommands: [], notes: [] },
    structureSignals: { rootPaths: [{ path: "src", role: "source_root" }], entryPoints: [], configurationFiles: ["package.json"] },
    existingCapabilities: [],
    relevantSurfaces: [],
    recommendedReadRefs: [],
    roadmapImplications: [],
    contextQuality: { coverage: "focused", confidence: "medium", warnings: [] },
    warnings: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function gitCheckpointAdvisory(decision) {
  const advisories = Array.isArray(decision.instruction.advisories)
    ? decision.instruction.advisories
    : [];
  return advisories.find((item) => item && item.kind === "git_checkpoint") ?? null;
}

function main() {
  buildDist();

  const root = tempProject("loom-phase-advisory-");
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));
    git(["init"], root);
    git(["config", "user.name", "Loom Test"], root);
    git(["config", "user.email", "loom-test@example.com"], root);
    git(["add", "package.json"], root);
    git(["commit", "-m", "initial"], root);

    runCli(["init"], root);
    const deliveryId = writeDeliveryAtPhaseHandoff(root);
    const firstContinue = runCli(["continue"], root);
    assert.equal(firstContinue.deliveryId, deliveryId);
    assert.equal(firstContinue.phaseId, "phase-2");
    assert.equal(firstContinue.nextAction.type, "repository_context_request");
    assert.equal(firstContinue.transition.type, "phase_activated");
    assert.equal(firstContinue.instruction.mode, "run_cli");
    assert.equal(gitCheckpointAdvisory(firstContinue), null);

    writeTechnicalBaseline(root, deliveryId);
    const repoRequest = runCli(["repository-context", "request", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    assert.equal(repoRequest.request, undefined);
    writeJson(projectFile(root, repoRequest.candidateFile), createRepositoryContextCandidate(repoRequest, root));
    const repoAccepted = runCli([
      "repository-context", "accept",
      "--delivery-id", deliveryId,
      "--phase-id", "phase-2",
      "--request-id", repoRequest.requestId,
      "--candidate-file", repoRequest.candidateFile,
    ], root);
    assert.equal(repoAccepted.instruction.mode, "ask_user");
    assert.equal(repoAccepted.instruction.expectedResponse.kind, "brainstorm_progressive_clarification");
    assert.equal(repoAccepted.instruction.expectedResponse.candidateFile, undefined);
    assert.equal(repoAccepted.instruction.expectedResponse.submitCommand, undefined);

    const advisory = gitCheckpointAdvisory(repoAccepted);
    assert.ok(advisory, "phase handoff ask_user should include git checkpoint advisory in git repo");
    assert.equal(advisory.blocking, false);
    assert.equal(advisory.phaseId, "phase-1");
    assert.ok(advisory.commands.includes("git status --short"));
    assert.ok(advisory.commands.includes("git push # optional"));

    const ordinaryContinue = runCli(["continue"], root);
    assert.equal(ordinaryContinue.phaseId, "phase-2");
    assert.equal(ordinaryContinue.nextAction.type, "brainstorm_confirmation");
    assert.equal(ordinaryContinue.instruction.mode, "ask_user");
    assert.equal(gitCheckpointAdvisory(ordinaryContinue), null);

    console.log("phase handoff git advisory verification passed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();
