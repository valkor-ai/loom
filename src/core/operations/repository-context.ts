import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";
import { invalidArgument, stateNotInitialized } from "../errors";
import {
  type RepositoryContext,
  type RepositoryContextRequest,
  repositoryContextRequestSchema,
  repositoryContextSchema,
} from "../contracts";
import { pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { loadDeliveryIndex, resolveLocator } from "../state/delivery";
import { brainstormContractSchema } from "../schemas";
import {
  brainstormContractPath,
  brainstormDecisionPath,
  brainstormDecisionsIndexPath,
  brainstormRequestCandidatePath,
  brainstormSessionRequestPath,
  fromProjectRelative,
  requirementContextPath,
  requirementKeywordHintsPath,
  requirementNormalizedTextPath,
  repositoryContextCandidatePath,
  repositoryContextPath,
  repositoryContextRequestPath,
  technicalBaselinePath,
  toProjectRelative,
  workspaceLatestPath,
} from "../state/paths";
import type { BrainstormContract, DeliveryIndexPhase } from "../schemas";
import { loadRequiredTechnicalBaseline } from "./contracts";
import {
  closeOperationLease,
  createOperationLease,
  operationRef,
  updateRouteState,
} from "./control";
import { repairSubmitRouting } from "./repair-routing";
import { autoRunInstruction, withAutoRunnableTransition } from "./routing-instructions";
import { artifactGenerationProtocolPolicy, artifactInstructionPolicy, artifactRepairPolicy, brainstormAskUserInstructionPolicy, brainstormAskUserReadStep, compactContextReadStep } from "./output-policy";
import { agentActionContract, brainstormSessionAgentActionContract } from "./agent-action";
import { referencedArtifactReadGuide } from "./artifact-read-guide";
import {
  businessLifecycleScanRules,
  businessScenarioConfirmationRules,
  businessObjectOperationCandidateRules,
  businessObjectOperationClarificationRules,
  brainstormCandidateSelfReviewRules,
  brainstormRequirementSemanticCompactRules,
  brainstormRequirementSemanticRules,
  confirmedBlockDetailRetentionRules,
  conceptGroundingSelfCheckRules,
  conceptGroundingPresentationRules,
  decisionImpactOrderingRules,
  finalSummaryRequiredUserVisibleTopicsWhenApplicable,
  finalSummaryReviewRules,
  frontendExperiencePresentationRules,
  frontendExperienceSelfCheckRules,
  frontendOperationPathCandidateRules,
  frontendOperationPathClarificationRules,
  nextPhasePreviewCandidateRules,
  phaseScopeOptionComparisonRules,
  phaseScopeSelfCheckRules,
  scopeItemCoverageCandidateRules,
  scopeItemCoverageClarificationRules,
} from "./brainstorm-rules";
import { writeRequestManifestAtomic } from "./request-manifest";

const execFileAsync = promisify(execFile);

export type CreateRepositoryContextRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
};

export type AcceptRepositoryContextInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  requestId?: string;
  candidateFile: string;
};

const repositoryContextEnumRefs = {
  projectKind: ["greenfield", "existing_project", "unknown"],
  relevantSurfaceKind: ["entrypoint", "module", "service", "controller", "data_access", "ui", "config", "test", "script", "documentation", "unknown"],
  capabilityStatus: ["implemented", "partial", "missing", "unknown"],
  recommendedReadReason: ["implemented_capability", "dependency_context", "integration_boundary", "test_or_validation", "risk_review", "extension_point"],
  roadmapImplicationType: ["already_implemented", "needs_scope_adjustment", "future_scope_risk", "none"],
  surfaceRelevance: ["implemented_capability", "architecture_boundary", "extension_point", "validation_surface", "delivery_context", "unrelated"],
  surfaceSuggestedUse: ["inspect_only", "inspect_or_extend", "reuse_existing_pattern", "avoid_modifying"],
};

const repositoryContextReferenceRules = {
  relevantSurfaces: {
    idField: "surfaceId",
    rule: "Every relevantSurfaces[].surfaceId is a stable local id such as surface-api-onboarding. It is not a file path.",
  },
  existingCapabilitiesSurfaceRefs: {
    field: "existingCapabilities[].surfaceRefs[]",
    mustReference: "relevantSurfaces[].surfaceId",
    forbidden: "Do not put file paths such as src/app/App.tsx in surfaceRefs.",
  },
  recommendedReadRefsSurfaceRefs: {
    field: "recommendedReadRefs[].surfaceRefs[]",
    mustReference: "relevantSurfaces[].surfaceId",
    forbidden: "Do not put recommendedReadRefs[].path or any other file path in surfaceRefs.",
  },
  recommendedReadRefsReason: {
    field: "recommendedReadRefs[].reason",
    mustUseEnumRef: "enumRefs.recommendedReadReason",
    allowedValues: repositoryContextEnumRefs.recommendedReadReason,
    mapping: {
      implemented_capability: "Read this file because it contains already implemented behavior or UI/API capability.",
      dependency_context: "Read this file because it defines dependencies, scripts, shared domain types, schema, or configuration the next work depends on.",
      integration_boundary: "Read this file because it connects surfaces, routes, runtime entry points, service boundaries, or cross-module wiring.",
      test_or_validation: "Read this file because it contains tests, validation helpers, verification fixtures, or quality gates.",
      risk_review: "Read this file because it may contain risk, migration, compatibility, or fragile behavior to inspect before planning.",
      extension_point: "Read this file because it is the most likely extension point for upcoming work.",
    },
  },
};

type RequirementContextRefs = {
  originalRequirementContextRef: string;
  requirementContextRef: string;
  normalizedRequirementTextRef?: string;
  keywordHintsRef?: string;
};

type BrainstormDecisionRefs = {
  latestConfirmedRequirementDecisionRef?: string;
  confirmedRequirementDecisionsIndexRef?: string;
};

export async function createRepositoryContextRequest(input: CreateRepositoryContextRequestInput): Promise<{
  operation: "repository_context_request_created";
  deliveryId: string;
  phaseId: string;
  requestId: string;
  request: RepositoryContextRequest;
  requestRef: string;
  candidateFile: string;
  lease: ReturnType<typeof operationRef>;
  nextCommand: { argv: string[] };
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const delivery = await loadDeliveryIndex(root, locator.deliveryId);
  const phase = delivery.phases.find((item) => item.phaseId === locator.phaseId);
  if (!phase) {
    throw invalidArgument("Active phase does not exist.", locator);
  }
  const brainstormContractRef = toProjectRelative(root, brainstormContractPath(root, locator.deliveryId));
  const requestId = createId("repoctx-req");
  const candidateFile = toProjectRelative(root, repositoryContextCandidatePath(root, locator, requestId));
  const requestRef = toProjectRelative(root, repositoryContextRequestPath(root, locator, requestId));
  const submitCommand = {
    name: "repository-context accept",
    argv: [
      "repository-context",
      "accept",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--request-id",
      requestId,
      "--candidate-file",
      "{candidateFile}",
    ],
  };
  const request: RepositoryContextRequest = repositoryContextRequestSchema.parse({
    schemaVersion: "1.0",
    requestId,
    agentAction: agentActionContract({
      actionKind: "repository_context",
      instruction: "Scan the current repository facts only, write RepositoryContext to outputContract.candidateFile, then run submitCommand exactly.",
      read: {
        required: ["this request", "referencedArtifactReadGuide", "scanPurpose", "generationRules", "enumRefs", "outputContract.schemaShape", "outputContract.referenceRules"],
        optional: ["source.brainstormContractRef", "source.technicalBaselineRef", "project files"],
        displayPolicy: "compact",
      },
      write: {
        candidateFile,
        blockedFile: candidateFile.replace(/candidate\.json$/, "blocked.json"),
        rules: [
          "Summarize current code facts only.",
          "Do not confirm phase scope or produce Brainstorm/PGC/AAC/TaskPlan.",
          "Do not use nextPhasePreview as code fact.",
          "Use outputContract.referenceRules for surfaceRefs and recommendedReadRefs reason values.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--request-id", "--candidate-file"],
        placeholders: { "{candidateFile}": candidateFile },
        runAfter: "candidateFile exists and follows outputContract.schemaShape",
      },
      schema: {
        primary: "RepositoryContext",
        shapeLocation: "outputContract.schemaShape",
        enumLocation: "enumRefs",
      },
      stopConditions: ["repository cannot be read", "blockedOutput is required", "submitCommand returns non-repairable failure"],
    }),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: "pending",
    purpose: "generate_phase_start_repository_snapshot",
    projectKind: baseline.projectKind,
    source: {
      brainstormContractRef,
      technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      brainstormContractRef,
      technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
    }),
    scanPurpose: {
      type: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
      activePhase: {
        phaseId: locator.phaseId,
        name: phase.name,
      },
      completedPhases: completedPhaseSummaries(delivery, locator.phaseId),
      rules: [
        "This request is generated before current phase scope confirmation.",
        "Do not infer current phase included scope, excluded scope, deferred scope, or acceptance refs.",
        "Do not use nextPhasePreview or future roadmap scope in this request.",
        "Produce repository facts only; Brainstorm confirms phase scope later.",
      ],
    },
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    enumRefs: {
      ...repositoryContextEnumRefs,
    },
    generationRules: [
      "Inspect the repository as needed before producing RepositoryContext.",
      "Summarize the current repo after prior completed phases before the next Brainstorm scope confirmation.",
      "This is a phase-start repository snapshot, not a phase scope contract.",
      "Do not infer or output current phase scope, acceptance coverage, or task planning decisions.",
      "Do not treat empty scope or acceptance refs as a blocker; they are not part of this request.",
      "Do not produce PGC, AAC, TaskPlan, Review findings, or Repair plan.",
      "Do not modify project files.",
      "Do not inline source code.",
      "Use enumRefs.recommendedReadReason exactly for recommendedReadRefs[].reason; do not invent natural-language reason values.",
      "Use enumRefs.relevantSurfaceKind exactly for relevantSurfaces[].kind.",
      "Use enumRefs.surfaceRelevance and enumRefs.surfaceSuggestedUse exactly for relevantSurfaces.",
      "Every existingCapabilities[].surfaceRefs[] and recommendedReadRefs[].surfaceRefs[] value must be a relevantSurfaces[].surfaceId, never a file path.",
      "recommendedReadRefs[].path is the file path; recommendedReadRefs[].surfaceRefs[] links that path to surface ids from relevantSurfaces.",
    ],
    outputContract: {
      schema: "RepositoryContext",
      candidateFile,
      enumRefs: repositoryContextEnumRefs,
      referenceRules: repositoryContextReferenceRules,
      schemaShape: repositoryContextSchemaShape({
        locator,
        requestRef,
        brainstormContractRef,
        technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
        projectKind: baseline.projectKind,
      }),
    },
    submitCommand,
    blockedOutput: {
      schemaRef: "repository-context-blocked-v1",
      candidateFile: candidateFile.replace(/candidate\.json$/, "blocked.json"),
      schemaShape: {
        schemaVersion: "1.0",
        requestId,
        status: "blocked",
        blockedReasons: [{ code: "REPOSITORY_CONTEXT_UNAVAILABLE", message: "Repository cannot be inspected enough to produce context." }],
      },
    },
    validatorPolicy: {
      pathSafety: true,
      requireProjectRelativePaths: true,
      forbiddenPathPrefixes: [".git/", ".loom/", "node_modules/"],
      allowWarnings: true,
      blockingRules: ["invalid_json", "invalid_schema", "absolute_path", "path_traversal", "forbidden_path_prefix", "inline_source_content"],
    },
    failureRecovery: {
      onRepairableValidationFailure: "return validator issues and ask Agent to repair the same candidate",
      onNonRepairableStateMismatch: "return routeDecision with state_corrupted or manual_review according to route engine",
      maxCandidateRepairAttempts: 3,
    },
    createdAt: new Date().toISOString(),
  });
  const requestPath = repositoryContextRequestPath(root, locator, requestId);
  const lease = await createOperationLease({
    projectRoot: root,
    locator,
    operationType: "repository_context_generation",
    refs: {
      requestRef: toProjectRelative(root, requestPath),
      candidateFile,
    },
  });
  try {
    await writeRequestManifestAtomic(root, requestPath, request);
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "planning",
      phaseStatus: "planning",
      nextAction: {
        type: "repository_context_request",
        source: "repository_context_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, requestPath),
        reason: "REPOSITORY_CONTEXT_REQUEST_CREATED",
      },
    });
  } catch (error) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "repository_context_generation",
      reason: "request_write_failed",
    });
    throw error;
  }
  return {
    operation: "repository_context_request_created",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    requestId,
    request,
    requestRef: toProjectRelative(root, requestPath),
    candidateFile,
    lease: operationRef(lease),
    nextCommand: {
      argv: ["repository-context", "accept", "--request-id", requestId, "--candidate-file", candidateFile],
    },
    instruction: withAutoRunnableTransition({
      mode: "generate_candidate",
      ...artifactInstructionPolicy(),
      candidateKind: "RepositoryContext",
      requestRef: toProjectRelative(root, requestPath),
      candidateFile,
      blockedOutput: request.blockedOutput,
      submitCommand: request.submitCommand,
      generationSteps: [
        "Read requestRef.",
        compactContextReadStep,
        "Use referencedArtifactReadGuide for source artifact selectors; do not guess jq wrapper roots.",
        "Inspect the repository as needed.",
        "Write the RepositoryContext candidate JSON to candidateFile.",
        "Run submitCommand after candidateFile exists.",
        "Follow the submit command response instruction after submit succeeds.",
      ],
      routingRule: "Read requestRef, inspect the repository as needed, write the RepositoryContext candidate to candidateFile, then run submitCommand. Do not run loom continue before submitCommand succeeds.",
      userMessage: "RepositoryContextRequest created. Generate the candidate JSON and submit it with the provided command.",
    }, {
      sourceCommand: "repository-context request",
      sourceSummary: "RepositoryContextRequest was created.",
      primaryAction: "generate_repository_context_and_submit",
    }),
  };
}

export async function acceptRepositoryContext(input: AcceptRepositoryContextInput): Promise<{
  operation: "repository_context_accepted";
  deliveryId: string;
  phaseId: string;
  repositoryContextId: string;
  repositoryContextRef: string;
  warnings: unknown[];
  instruction: Record<string, unknown>;
} | {
  operation: "repository_context_invalid_candidate";
  deliveryId: string;
  phaseId: string;
  issues: unknown[];
  repairInstruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const candidatePath = resolveCliPath(root, input.candidateFile);
  const parsed = repositoryContextSchema.safeParse(await readJsonFile(candidatePath));
  if (!parsed.success) {
    const issues = parsed.error.issues.map((issue) => ({
      path: issue.path.join("."),
      message: issue.message,
    }));
    return {
      operation: "repository_context_invalid_candidate",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      issues,
      repairInstruction: {
        mode: "repair_candidate",
        ...artifactRepairPolicy(),
        schema: "RepositoryContext",
        candidateFile: toProjectRelative(root, candidatePath),
        issues,
        enumRefs: repositoryContextEnumRefs,
        referenceRules: repositoryContextReferenceRules,
        schemaShape: repositoryContextSchemaShape({
          locator,
          requestRef: input.requestId ? toProjectRelative(root, repositoryContextRequestPath(root, locator, input.requestId)) : "requestRef from the original RepositoryContextRequest",
          brainstormContractRef: toProjectRelative(root, brainstormContractPath(root, locator.deliveryId)),
          technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
          projectKind: "existing_project | greenfield | unknown",
        }),
        repairSubmitRouting: repairSubmitRouting({
          kind: "candidate",
          submitCommandName: "repository-context accept",
        }),
        instructions: [
          "Repair only the RepositoryContext candidate JSON.",
          "Do not modify project source code.",
          "Do not produce PGC, AAC, TaskPlan, Review findings, or Repair plan.",
          "Use enumRefs.recommendedReadReason exactly for recommendedReadRefs[].reason.",
          "Every surfaceRefs value must reference relevantSurfaces[].surfaceId, never a file path.",
          "Return a complete replacement RepositoryContext to the same candidateFile.",
          "Run repository-context accept again with the same candidate-file.",
        ],
        submitCommand: {
          name: "repository-context accept",
          argv: [
            "repository-context",
            "accept",
            "--delivery-id",
            locator.deliveryId,
            "--phase-id",
            locator.phaseId,
            ...(input.requestId ? ["--request-id", input.requestId] : []),
            "--candidate-file",
            toProjectRelative(root, candidatePath),
          ],
        },
      },
    };
  }
  const candidate = parsed.data;
  if (candidate.deliveryId !== locator.deliveryId || candidate.phaseId !== locator.phaseId) {
    throw invalidArgument("RepositoryContext candidate deliveryId/phaseId does not match active delivery.", {
      active: locator,
      candidate: { deliveryId: candidate.deliveryId, phaseId: candidate.phaseId },
    });
  }
  if (input.requestId) {
    const requestPath = repositoryContextRequestPath(root, locator, input.requestId);
    if (!(await pathExists(requestPath))) {
      throw invalidArgument("RepositoryContextRequest does not exist.", { requestId: input.requestId });
    }
  }
  const context: RepositoryContext = repositoryContextSchema.parse({
    ...candidate,
    updatedAt: new Date().toISOString(),
  });
  const requestRef = input.requestId ? toProjectRelative(root, repositoryContextRequestPath(root, locator, input.requestId)) : null;
  const validationIssues = validateRepositoryContext(root, context, requestRef);
  if (validationIssues.length > 0) {
    return {
      operation: "repository_context_invalid_candidate",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      issues: validationIssues,
      repairInstruction: repositoryContextRepairInstruction(root, locator, input, candidatePath, validationIssues),
    };
  }
  const contextFile = repositoryContextPath(root, locator);
  await writeJsonAtomic(contextFile, context);
  await writeJsonAtomic(workspaceLatestPath(root, locator), {
    schemaVersion: "1.0",
    repositoryContextId: context.repositoryContextId,
    repositoryContextRef: toProjectRelative(root, contextFile),
    updatedAt: context.updatedAt,
  });
  const brainstormPlan = await maybePreparePhaseBrainstormAfterRepositoryContext(root, locator, contextFile);
  if (brainstormPlan) {
    await writeJsonAtomic(brainstormPlan.contractPath, brainstormPlan.contract);
    await writeRequestManifestAtomic(root, brainstormPlan.requestPath, brainstormPlan.request);
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "brainstorming",
      phaseStatus: "pending",
      latestRefs: {
        repositoryContext: toProjectRelative(root, contextFile),
        ...brainstormPlan.refs,
      },
      nextAction: {
        type: "brainstorm_confirmation",
        source: "repository_context",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: brainstormPlan.refs.brainstormRequest,
        reason: "REPOSITORY_CONTEXT_READY_FOR_PHASE_BRAINSTORM",
        targetNode: "brainstorm",
      },
    });
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "repository_context_generation",
      reason: "repository_context_accepted",
    });
    const instruction = await withPhaseTransitionGitAdvisory(root, brainstormPlan.fromPhaseId, {
      mode: "ask_user",
      autoContinue: false,
      ...brainstormAskUserInstructionPolicy(),
      requestRef: brainstormPlan.refs.brainstormRequest,
      nextAction: {
        type: "brainstorm_confirmation",
        targetNode: "brainstorm",
        reason: "REPOSITORY_CONTEXT_READY_FOR_PHASE_BRAINSTORM",
        ref: brainstormPlan.refs.brainstormRequest,
      },
      userMessage: "RepositoryContext accepted. Use the generated BrainstormSessionRequest to continue the progressive Brainstorm conversation for this phase. Do not write or submit BrainstormCandidate until the dedicated final_summary block is confirmed.",
      expectedResponse: {
        kind: "brainstorm_progressive_clarification",
        rule: "Read requestRef through agentAction.read.fieldGroups before presenting Brainstorm confirmation. Agent manages the conversation. For phase_scope, concept_grounding, and frontend_experience confirmations, continue to the next Brainstorm block in chat. Read outputContract/generationProtocol/enumRefs, write BrainstormCandidate, and run submitCommand only after the dedicated final_summary block is explicitly confirmed.",
        requestReadRule: brainstormAskUserReadStep,
        requestRef: brainstormPlan.refs.brainstormRequest,
      },
    });
    return {
      operation: "repository_context_accepted",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      repositoryContextId: context.repositoryContextId,
      repositoryContextRef: toProjectRelative(root, contextFile),
      warnings: context.warnings,
      instruction,
    };
  }
  await updateRouteState({
    projectRoot: root,
    locator,
    deliveryStatus: "planning",
    phaseStatus: "planning",
    latestRefs: {
      repositoryContext: toProjectRelative(root, contextFile),
    },
    nextAction: {
      type: "planning_contract_create",
      source: "repository_context",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      ref: toProjectRelative(root, contextFile),
      reason: "REPOSITORY_CONTEXT_ACCEPTED",
    },
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "repository_context_generation",
    reason: "repository_context_accepted",
  });
  return {
    operation: "repository_context_accepted",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    repositoryContextId: context.repositoryContextId,
    repositoryContextRef: toProjectRelative(root, contextFile),
    warnings: context.warnings,
    instruction: autoRunInstruction({
      actionType: "planning_contract_create",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: "REPOSITORY_CONTEXT_ACCEPTED",
      targetNode: "planning_contract_create",
      ref: toProjectRelative(root, contextFile),
      argv: ["planning-contract", "create", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "RepositoryContext accepted. Continue immediately by creating PlanningGenerationContract.",
    }),
  };
}

function repositoryContextSchemaShape(input: {
  locator: { deliveryId: string; phaseId: string };
  requestRef: string;
  brainstormContractRef: string;
  technicalBaselineRef: string;
  projectKind: string;
}): Record<string, unknown> {
  const warningExample = {
    code: "LOW_CONFIDENCE_REPOSITORY_SCAN",
    message: "Explain the limitation as an object. Use [] only when there are no warnings.",
  };
  return {
    schemaVersion: "1.0",
    repositoryContextId: "repoctx-001",
    deliveryId: input.locator.deliveryId,
    phaseId: input.locator.phaseId,
    status: "ready | partial | insufficient",
    source: {
      requestRef: input.requestRef,
      brainstormContractRef: input.brainstormContractRef,
      technicalBaselineRef: input.technicalBaselineRef,
    },
    requestLens: {
      projectKind: input.projectKind,
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Short repository summary from current phase perspective.",
      repositoryShape: "single_package | monorepo | multi_application | unknown",
      primaryApplications: [{
        applicationId: "app-main",
        name: "Main application",
        kind: "service | cli | web_app | library | unknown",
        rootPath: ".",
      }],
    },
    technologySignals: {
      primaryLanguages: ["typescript"],
      frameworks: ["framework-name"],
      packageManagers: ["npm"],
      buildCommands: ["npm run build"],
      testCommands: ["npm test"],
      notes: ["Short technology note. Use [] only when none."],
    },
    structureSignals: {
      rootPaths: [{ path: "src", role: "source_root" }],
      entryPoints: [{
        path: "src/index.ts",
        kind: "module | cli | server | page | test | config | unknown",
        description: "Why this entry point matters. This array must contain objects, not strings.",
      }],
      configurationFiles: ["package.json"],
    },
    existingCapabilities: [{
      capabilityId: "cap-existing-example",
      name: "Existing capability name",
      status: "implemented | partial | missing | unknown",
      summary: "Observed repository capability from the current codebase.",
      surfaceRefs: ["surface-example"],
      confidence: "high | medium | low | unknown",
      deliveryRelevance: "Why this matters to the overall delivery or upcoming Brainstorm.",
    }],
    relevantSurfaces: [{
      surfaceId: "surface-example",
      kind: "entrypoint | module | service | controller | data_access | ui | config | test | script | documentation | unknown",
      path: "project-relative/path",
      summary: "Surface summary.",
      relevance: "implemented_capability | architecture_boundary | extension_point | validation_surface | delivery_context | unrelated",
      suggestedUse: "inspect_only | inspect_or_extend | reuse_existing_pattern | avoid_modifying",
    }],
    recommendedReadRefs: [{
      path: "project-relative/path",
      reason: "implemented_capability | dependency_context | integration_boundary | test_or_validation | risk_review | extension_point",
      priority: "high | medium | low",
      summary: "Why Agent should read this file first.",
      surfaceRefs: ["surface-example"],
    }],
    roadmapImplications: [{
      phaseRef: "phase-2",
      type: "already_implemented | needs_scope_adjustment | future_scope_risk | none",
      impactType: "avoid_structural_dead_end | preserve_extension_point | avoid_scope_conflict | defer_implementation | unknown",
      summary: "Optional implication summary. Use [] only when none.",
      affectedSurfaces: ["surface-example"],
    }],
    contextQuality: {
      coverage: "focused | partial | broad | insufficient",
      confidence: "high | medium | low | unknown",
      warnings: [warningExample],
    },
    warnings: [warningExample],
    createdAt: "1970-01-01T00:00:00.000Z",
    updatedAt: "1970-01-01T00:00:00.000Z",
  };
}

function repositoryContextRepairInstruction(
  root: string,
  locator: { deliveryId: string; phaseId: string },
  input: AcceptRepositoryContextInput,
  candidatePath: string,
  issues: unknown[],
): Record<string, unknown> {
  return {
    mode: "repair_candidate",
    schema: "RepositoryContext",
    ...artifactRepairPolicy(),
    candidateFile: toProjectRelative(root, candidatePath),
    issues,
    enumRefs: repositoryContextEnumRefs,
    referenceRules: repositoryContextReferenceRules,
    schemaShape: repositoryContextSchemaShape({
      locator,
      requestRef: input.requestId ? toProjectRelative(root, repositoryContextRequestPath(root, locator, input.requestId)) : "requestRef from the original RepositoryContextRequest",
      brainstormContractRef: toProjectRelative(root, brainstormContractPath(root, locator.deliveryId)),
      technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
      projectKind: "existing_project | greenfield | unknown",
    }),
    repairSubmitRouting: repairSubmitRouting({
      kind: "candidate",
      submitCommandName: "repository-context accept",
    }),
    instructions: [
      "Repair only the RepositoryContext candidate JSON.",
      "Do not modify project source code.",
      "Do not produce PGC, AAC, TaskPlan, Review findings, or Repair plan.",
      "Use enumRefs.recommendedReadReason exactly for recommendedReadRefs[].reason.",
      "Every surfaceRefs value must reference relevantSurfaces[].surfaceId, never a file path.",
      "Return a complete replacement RepositoryContext to the same candidateFile.",
      "Run repository-context accept again with the same candidate-file.",
    ],
    submitCommand: {
      name: "repository-context accept",
      argv: [
        "repository-context",
        "accept",
        "--delivery-id",
        locator.deliveryId,
        "--phase-id",
        locator.phaseId,
        ...(input.requestId ? ["--request-id", input.requestId] : []),
        "--candidate-file",
        toProjectRelative(root, candidatePath),
      ],
    },
  };
}

async function maybePreparePhaseBrainstormAfterRepositoryContext(
  projectRoot: string,
  locator: { deliveryId: string; phaseId: string },
  repositoryContextFile: string,
): Promise<{
  contract: BrainstormContract;
  contractPath: string;
  request: Record<string, unknown>;
  requestPath: string;
  refs: Record<string, string>;
  fromPhaseId: string | null;
} | null> {
  const contractPath = brainstormContractPath(projectRoot, locator.deliveryId);
  if (!(await pathExists(contractPath))) {
    throw invalidArgument("Cannot create phase Brainstorm request because Brainstorm contract is missing.", {
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      expectedRef: toProjectRelative(projectRoot, contractPath),
    });
  }
  const contract = brainstormContractSchema.parse(await readJsonFile(contractPath));
  const delivery = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const phase = delivery.phases.find((item) => item.phaseId === locator.phaseId);
  const currentRoadmapPhase = contract.roadmap?.phases.find((item) => item.phaseId === locator.phaseId);
  const alreadyConfirmed =
    contract.status === "confirmed" &&
    contract.phasePlan.current.phaseId === locator.phaseId &&
    currentRoadmapPhase?.status === "scope_confirmed";
  if (!phase || alreadyConfirmed) {
    return null;
  }
  const preview = contract.phasePlan.nextPhasePreview;
  if (!contract.roadmap) {
    throw invalidArgument("Brainstorm contract roadmap is required for phase continuation.", {
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
    });
  }
  const now = new Date().toISOString();
  const brainstormRunId = createId(`bs-${phase.phaseId}`);
  const requestId = createId(`brainstorm-session-${phase.phaseId}`);
  const candidateFile = toProjectRelative(projectRoot, brainstormRequestCandidatePath(projectRoot, locator.deliveryId, phase.phaseId, requestId));
  const roadmapPhase = ensureRoadmapPhaseForBrainstorm(contract, phase, preview);
  contract.roadmap.currentPhaseId = phase.phaseId;
  roadmapPhase.status = "scope_confirming";
  const fromPhaseId = lastCompletedPhaseId(delivery, phase.phaseId);
  const requirementRefs = await requirementContextRefsForPhaseContinuation(projectRoot, delivery);
  const decisionRefs = await brainstormDecisionRefsForPhaseContinuation(projectRoot, delivery, fromPhaseId);
  addPhaseClarificationQuestion(contract, phase, roadmapPhase.goal, now);
  contract.status = "needs_clarification";
  contract.handoff = {
    ready: false,
    nextNode: "planning_generation_contract",
    blockingReasons: [`${phase.phaseId} scope requires user clarification and confirmation.`],
    confirmedAt: null,
  };
  contract.updatedAt = now;
  const request = createPhaseBrainstormSessionRequest({
    projectRoot,
    deliveryId: locator.deliveryId,
    phase,
    contract,
    brainstormRunId,
    requestId,
    candidateFile,
    repositoryContextRef: toProjectRelative(projectRoot, repositoryContextFile),
    requirementRefs,
    decisionRefs,
    fromPhaseId,
    now,
  });
  const requestPath = brainstormSessionRequestPath(projectRoot, locator.deliveryId, requestId);
  return {
    contract,
    contractPath,
    request,
    requestPath,
    refs: {
      brainstormContract: toProjectRelative(projectRoot, contractPath),
      brainstormRequest: toProjectRelative(projectRoot, requestPath),
      brainstormRequestId: requestId,
      brainstormRunId,
      brainstormCandidateFile: candidateFile,
      ...requirementRefs,
      ...decisionRefs,
    },
    fromPhaseId,
  };
}

async function requirementContextRefsForPhaseContinuation(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
): Promise<RequirementContextRefs> {
  const requirementContextRef = await resolveRequiredDeliveryRef(projectRoot, delivery, ["originalRequirementContextRef", "requirementContextRef"], {
    stablePath: requirementContextPath(projectRoot, delivery.deliveryId),
    label: "original requirement context",
  });
  const context = await readJsonFile(path.resolve(projectRoot, requirementContextRef));
  const contextRecord = isRecord(context) ? context : {};
  const normalizedFromContext = typeof contextRecord.normalizedTextRef === "string" ? contextRecord.normalizedTextRef : null;
  const normalizedRequired = contextRecord.normalizedTextStatus === "completed" || typeof normalizedFromContext === "string";
  const normalizedRequirementTextRef = await resolveExistingDeliveryRef(projectRoot, delivery, ["normalizedRequirementTextRef"], {
    stablePath: normalizedFromContext ? path.resolve(projectRoot, normalizedFromContext) : requirementNormalizedTextPath(projectRoot, delivery.deliveryId),
    required: normalizedRequired,
    label: "normalized requirement text",
  });
  const keywordFromContext = typeof contextRecord.keywordHintsRef === "string" ? contextRecord.keywordHintsRef : null;
  const keywordRequired = typeof keywordFromContext === "string";
  const keywordHintsRef = await resolveExistingDeliveryRef(projectRoot, delivery, ["keywordHintsRef"], {
    stablePath: keywordFromContext ? path.resolve(projectRoot, keywordFromContext) : requirementKeywordHintsPath(projectRoot, delivery.deliveryId),
    required: keywordRequired,
    label: "requirement keyword hints",
  });

  return {
    originalRequirementContextRef: requirementContextRef,
    requirementContextRef,
    ...(normalizedRequirementTextRef ? { normalizedRequirementTextRef } : {}),
    ...(keywordHintsRef ? { keywordHintsRef } : {}),
  };
}

async function brainstormDecisionRefsForPhaseContinuation(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  latestCompletedPhaseId: string | null,
): Promise<BrainstormDecisionRefs> {
  const indexRef = await resolveExistingDeliveryRef(projectRoot, delivery, ["brainstormDecisionsIndex"], {
    stablePath: brainstormDecisionsIndexPath(projectRoot, delivery.deliveryId),
    required: false,
    label: "confirmed Brainstorm decisions index",
  });
  const latestPhase = latestCompletedPhaseId
    ? delivery.phases.find((phase) => phase.phaseId === latestCompletedPhaseId)
    : null;
  const latestRef = typeof latestPhase?.latestRefs.brainstormDecision === "string"
    ? latestPhase.latestRefs.brainstormDecision
    : null;
  const latestStablePath = latestCompletedPhaseId
    ? brainstormDecisionPath(projectRoot, delivery.deliveryId, latestCompletedPhaseId)
    : null;
  const latestConfirmedRequirementDecisionRef = latestStablePath
    ? await resolveRefOrStablePath(projectRoot, latestRef, {
        stablePath: latestStablePath,
        required: false,
        label: "latest confirmed requirement decision",
      })
    : null;

  return {
    ...(latestConfirmedRequirementDecisionRef ? { latestConfirmedRequirementDecisionRef } : {}),
    ...(indexRef ? { confirmedRequirementDecisionsIndexRef: indexRef } : {}),
  };
}

async function resolveExistingDeliveryRef(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  keys: string[],
  input: { stablePath: string; required: boolean; label: string },
): Promise<string | null> {
  const indexedRef = delivery.phases
    .flatMap((phase) => keys.map((key) => phase.latestRefs[key]))
    .find((ref): ref is string => typeof ref === "string" && ref.length > 0);
  return resolveRefOrStablePath(projectRoot, indexedRef ?? null, input);
}

async function resolveRequiredDeliveryRef(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  keys: string[],
  input: { stablePath: string; label: string },
): Promise<string> {
  const ref = await resolveExistingDeliveryRef(projectRoot, delivery, keys, {
    ...input,
    required: true,
  });
  if (!ref) {
    throw invalidArgument(`SOURCE_NOT_READY: ${input.label} is required for phase continuation but its file is missing.`, {
      code: "SOURCE_NOT_READY",
      sourceKind: input.label,
      stableRef: toProjectRelative(projectRoot, input.stablePath),
    });
  }
  return ref;
}

async function resolveRefOrStablePath(
  projectRoot: string,
  indexedRef: string | null,
  input: { stablePath: string; required: boolean; label: string },
): Promise<string | null> {
  if (indexedRef && await pathExists(path.resolve(projectRoot, indexedRef))) {
    return indexedRef;
  }
  if (await pathExists(input.stablePath)) {
    return toProjectRelative(projectRoot, input.stablePath);
  }
  if (!input.required) {
    return null;
  }
  throw invalidArgument(`SOURCE_NOT_READY: ${input.label} is required for phase continuation but its file is missing.`, {
    code: "SOURCE_NOT_READY",
    sourceKind: input.label,
    indexedRef,
    stableRef: toProjectRelative(projectRoot, input.stablePath),
    recovery: "Restore the missing requirement source file or restart the delivery with the original requirement input; do not continue with guessed phase scope.",
  });
}

function completedPhaseSummaries(
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  activePhaseId: string,
): Array<{ phaseId: string; name: string; status: string }> {
  return delivery.phases
    .filter((item) => item.phaseId !== activePhaseId && item.status === "completed")
    .map((item) => ({ phaseId: item.phaseId, name: item.name, status: item.status }));
}

function ensureRoadmapPhaseForBrainstorm(
  contract: BrainstormContract,
  phase: DeliveryIndexPhase,
  preview: BrainstormContract["phasePlan"]["nextPhasePreview"],
): NonNullable<BrainstormContract["roadmap"]>["phases"][number] {
  if (!contract.roadmap) {
    throw invalidArgument("Brainstorm contract roadmap is required for phase continuation.", {
      phaseId: phase.phaseId,
    });
  }
  const existing = contract.roadmap.phases.find((item) => item.phaseId === phase.phaseId);
  if (existing) return existing;
  const goal = preview.kind === "candidate" ? preview.goal : phase.name;
  const nextPhase = {
    phaseId: phase.phaseId,
    name: phase.name,
    status: "scope_confirming" as const,
    goal,
    scope: {
      includedRefs: [],
      deferredRefs: [],
      excludedRefs: [],
    },
    acceptanceRefs: [],
    dependsOn: [],
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
  };
  contract.roadmap.phases.push(nextPhase);
  return nextPhase;
}

function addPhaseClarificationQuestion(
  contract: BrainstormContract,
  phase: DeliveryIndexPhase,
  phaseGoal: string,
  now: string,
): void {
  const questionId = `q-${phase.phaseId}-scope`;
  if (!contract.clarification.questions.some((item) => item.questionId === questionId)) {
    const turnId = `turn-${pad(contract.clarification.turns.length + 1)}`;
    contract.clarification.turns.push({
      turnId,
      startedAt: now,
      completedAt: null,
      reason: `需要确认 ${phase.name} 的具体范围。`,
      questions: [questionId],
      answers: [],
      patches: [],
      confirmations: [],
      status: "needs_answer",
    });
    contract.clarification.questions.push({
      questionId,
      turnId,
      type: "open_choice",
      severity: "blocking",
      question: `${phase.name} 这一阶段你希望具体交付什么？`,
      whyAsked: "进入下一阶段前必须基于当前代码事实确认本阶段范围，避免重复实现或把未来能力直接纳入实现。",
      suggestedOptions: [
        {
          optionId: "follow-next-phase-seed",
          label: "按建议推进",
          description: phaseGoal,
          recommended: true,
        },
        {
          optionId: "adjust-phase-scope",
          label: "调整本阶段范围",
          description: "说明本阶段要新增、移除或延后的能力。",
        },
        {
          optionId: "finish-delivery",
          label: "结束本次交付",
          description: "确认后续能力暂不继续，本次交付到上一阶段为止。",
        },
      ],
      allowFreeform: true,
      freeformHint: "可以直接描述当前阶段包含什么、暂缓什么、不做什么。",
      defaultIfSkipped: {
        optionId: "follow-next-phase-seed",
        assumptionText: `默认按上一阶段建议推进 ${phase.name}，但仍需要用户确认后才能进入规划。`,
      },
      status: "pending",
    });
  }
  contract.clarification.pendingQuestionIds = uniqueStrings([
    ...contract.clarification.pendingQuestionIds,
    questionId,
  ]);
  contract.clarification.status = "needs_answer";
}

function createPhaseBrainstormSessionRequest(input: {
  projectRoot: string;
  deliveryId: string;
  phase: DeliveryIndexPhase;
  contract: BrainstormContract;
  brainstormRunId: string;
  requestId: string;
  candidateFile: string;
  repositoryContextRef: string;
  requirementRefs: RequirementContextRefs;
  decisionRefs: BrainstormDecisionRefs;
  fromPhaseId: string | null;
  now: string;
}): Record<string, unknown> {
  const blockedFile = input.candidateFile.replace(/candidate\.json$/, "blocked.json");
  const submitCommand = {
    name: "brainstorm accept",
    argv: [
      "brainstorm",
      "accept",
      "--delivery-id",
      input.deliveryId,
      "--phase-id",
      input.phase.phaseId,
      "--request-id",
      input.requestId,
      "--run-id",
      input.brainstormRunId,
      "--candidate-file",
      "{candidateFile}",
    ],
  };
  const sourceRefs = input.contract.sources
    .map((source) => source.path ?? source.sourceId)
    .filter((value): value is string => typeof value === "string" && value.length > 0);
  const nextPhaseSeed = input.contract.phasePlan.nextPhasePreview;
  return {
    schemaVersion: "1.0",
    requestId: input.requestId,
    requestType: "brainstorm_session",
    agentAction: brainstormSessionAgentActionContract({
      candidateFile: input.candidateFile,
      blockedFile,
      contextKind: "phase_continuation",
      submitCommand,
    }),
    brainstormRunId: input.brainstormRunId,
    deliveryId: input.deliveryId,
    phaseId: input.phase.phaseId,
    originalRequest: input.contract.deliveryContext.originalRequest,
    interactionMode: "agent_managed_conversation",
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    contextRefs: {
      deliveryContextRef: toProjectRelative(input.projectRoot, brainstormContractPath(input.projectRoot, input.deliveryId)),
      latestRepositoryContextRef: input.repositoryContextRef,
      ...input.requirementRefs,
      ...input.decisionRefs,
      ...(input.contract.conceptGroundingRefs?.deliveryConceptGlossaryRef ? { deliveryConceptGlossaryRef: input.contract.conceptGroundingRefs.deliveryConceptGlossaryRef } : {}),
      ...(input.contract.conceptGroundingRefs?.phaseConceptGroundingRef ? { phaseConceptGroundingRef: input.contract.conceptGroundingRefs.phaseConceptGroundingRef } : {}),
      ...(input.contract.frontendExperienceRefs?.currentFrontendExperienceRef ? { currentFrontendExperienceRef: input.contract.frontendExperienceRefs.currentFrontendExperienceRef } : {}),
      sourceRefs,
    },
    sourceFieldAccessHints: {
      previousContractInput: {
        sourcesSelector: ".sources[]",
        sourceIdField: "sourceId",
        typeField: "type",
        pathField: "path",
        titleField: "title",
        textDigestField: "textDigest",
      },
      requirementContextInput: {
        sourceItemsSelector: ".sourceItems[]",
        itemIdField: "itemId",
        kindField: "kind",
        originField: "origin",
        originalPathField: "path",
        extractedTextRefField: "extractedTextRef",
      },
      candidateOutput: {
        sourcesSelector: ".sources[]",
        sourceIdField: "sourceId",
        typeField: "type",
        pathField: "path",
        titleField: "title",
        textDigestField: "textDigest",
      },
      mappingRules: [
        "For phase continuation BrainstormRequests, read existing accepted source facts from contextRefs.deliveryContextRef at .sources[] using sourceId/type.",
        "Read contextRefs.originalRequirementContextRef/requirementContextRef as original requirement source records; original requirements remain the requirement authority across phases.",
        "When contextRefs.requirementContextRef is present, read original requirement source records from .sourceItems[] using itemId/kind, not sourceId/type.",
        "When contextRefs.normalizedRequirementTextRef is present, use it as the original requirement text source for current phase clarification and nextPhasePreview candidate wording.",
        "When contextRefs.keywordHintsRef is present, use it only as advisory input for clarification and concept candidate discovery; never treat keyword hints as scope or acceptance authority.",
        "When contextRefs.latestConfirmedRequirementDecisionRef is present, read it as the latest confirmed requirement decision snapshot before summarizing the current phase.",
        "When contextRefs.confirmedRequirementDecisionsIndexRef is present, read it only to locate an explicitly referenced older phase decision; do not ask the CLI to guess targeted history.",
        "When writing BrainstormCandidate.sources[], preserve sourceId/type from the previous delivery contract unless the user adds a new source.",
        "If a jq selector returns null for sourceId/type, use sourceFieldAccessHints instead of probing guessed paths.",
      ],
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      brainstormContractRef: toProjectRelative(input.projectRoot, brainstormContractPath(input.projectRoot, input.deliveryId)),
      latestRepositoryContextRef: input.repositoryContextRef,
      originalRequirementContextRef: input.requirementRefs.originalRequirementContextRef,
      requirementContextRef: input.requirementRefs.requirementContextRef,
      normalizedRequirementTextRef: input.requirementRefs.normalizedRequirementTextRef,
      keywordHintsRef: input.requirementRefs.keywordHintsRef,
      latestConfirmedRequirementDecisionRef: input.decisionRefs.latestConfirmedRequirementDecisionRef,
      confirmedRequirementDecisionsIndexRef: input.decisionRefs.confirmedRequirementDecisionsIndexRef,
      deliveryConceptGlossaryRef: input.contract.conceptGroundingRefs?.deliveryConceptGlossaryRef,
      phaseConceptGroundingRef: input.contract.conceptGroundingRefs?.phaseConceptGroundingRef,
      currentFrontendExperienceRef: input.contract.frontendExperienceRefs?.currentFrontendExperienceRef,
    }),
    keywordHintsPolicy: {
      status: "advisory_only",
      mustNotTreatAsScope: true,
      mustNotTreatAsAcceptance: true,
      mustNotTreatAsConfirmedConcept: true,
      mayUseForClarificationQuestions: true,
      mayUseForConceptGroundingCandidates: true,
      ignoreWhenIrrelevant: true,
    },
    phaseContinuationContext: {
      activePhase: {
        phaseId: input.phase.phaseId,
        title: input.phase.name,
        status: "scope_confirming",
      },
      previousPhaseId: input.fromPhaseId,
      nextPhaseSeed,
      rules: [
        "Use latestRepositoryContextRef as the current code fact source to avoid re-asking for capabilities already implemented.",
        "Use deliveryContextRef as the accepted BrainstormContract for prior confirmed scope, sources, acceptance, and roadmap facts.",
        "Use originalRequirementContextRef/requirementContextRef and normalizedRequirementTextRef, when present, as the original full-delivery requirement context for source-grounded phase clarification.",
        "Use latestConfirmedRequirementDecisionRef as the nearest confirmed requirement decision history, including prior user-confirmed scope, requirement changes, concept meaning, frontend target, and deferred/excluded boundaries.",
        "Use confirmedRequirementDecisionsIndexRef only when the user clearly refers to an earlier confirmed phase; read the listed decisionRef for that phase or ask the user which phase they mean.",
        "Use keywordHintsRef, when present, only as advisory extraction support for clarification questions and concept candidates.",
        "Use nextPhaseSeed as a non-binding starting point for this phase clarification.",
        "If nextPhaseSeed is broad, consult normalizedRequirementTextRef, requirementContextRef, latestConfirmedRequirementDecisionRef, latestRepositoryContextRef, and previous deferred/excluded scope before presenting a concrete current-phase candidate to the user.",
        "Do not preserve or regenerate a complete future roadmap. Confirm only the current phase and produce a new nextPhasePreview.",
        "Do not read or require prior phase AAC, TaskPlan, TaskResult, or ReviewResult for Brainstorm confirmation.",
        "After brainstorm accept succeeds, PGC/AAC/TaskPlan use current phase confirmed scope plus latestRepositoryContextRef.",
      ],
    },
    clarificationGuidance: {
      choiceFirstClarification: true,
      preferredOptionCount: { min: 3, max: 5 },
      eachOptionMustExplainImpact: true,
      allowFreeformAlternative: true,
      confirmIncludedExcludedDeferredExplicitly: true,
      confirmCurrentPhaseOnly: true,
      includeRepositoryContextWhenClarifying: true,
      phaseScopeOptionComparison: {
        requiredByDefault: true,
        nextPhaseSeedIsPreselectedAnswer: false,
        optionCount: { min: 2, max: 3 },
        recommendationRequired: true,
        recommendationMustPreserveCoreCurrentPhaseOutcome: true,
        narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems: true,
        scopeReductionRequiresExplicitUserConfirmation: true,
        atomicSingleScopeException: {
          allowedOnlyWhenNoSourceGroundedAlternativeCutExists: true,
          mustExplainMissingAlternativeCuts: [
            "narrower cut",
            "broader adjacent-workflow cut",
            "dependency or ordering cut",
            "deferred lifecycle action",
            "alternate UI or runtime boundary",
          ],
          disallowedWhenMultipleScopePreviewItemsOrLifecycleActionsExist: true,
        },
      },
    },
    conceptGroundingRequest: {
      stage: "phase_continuation",
      mustReuseDeliveryConceptGlossaryRef: true,
      mustProducePhaseConceptGrounding: true,
      mayProduceGlossaryUpdates: true,
      mustShowConceptsToUserBeforeAccept: true,
      mustNotTreatFutureConceptsAsCurrentScope: true,
      selectionGuidance: {
        deliveryGlossaryPurpose: "Reuse the delivery-wide concept glossary as stable context; update it only when the user confirms a real cross-phase concept change.",
        phaseGroundingPurpose: "Current-phase high-risk concepts that must be understood before architecture and task planning.",
        preferConceptsAffecting: [
          "business_invariant",
          "state_transition",
          "resource_consistency",
          "permission_boundary",
          "external_contract",
          "scope_confusion_risk",
          "user_visible_flow",
          "runtime_or_delivery_semantics",
          "frontend_experience_semantics",
        ],
        includeConceptTypes: [
          "Abstract concepts summarized across multiple requirement sections.",
          "Concrete terms explicitly appearing in the source requirement.",
        ],
        extractionRules: [
          "Use the latest repository context to avoid re-asking concepts already implemented, but do not let it replace the user's current phase scope confirmation.",
          "Explain low-frequency, domain-specific, or easy-to-overlook terms in common implementation language before coding begins.",
          "Rank concepts by phaseRelevance, priority, then attentionRank; put the highest-risk concepts first.",
        ],
        antiPatterns: [
          "Do not copy every delivery-wide concept into current phase concepts.",
          "Do not use only a generic project label as the current phase concept.",
          "Do not treat future, deferred, or excluded concepts as current scope.",
        ],
      },
      userPresentationGuidance: {
        showConceptSummaryInPlainLanguage: true,
        askUserToConfirmOrCorrect: true,
        avoidSchemaLanguage: true,
      },
    },
    firstClarificationGate: {
      required: true,
      initialUserRequestDoesNotCountAsConfirmation: true,
      mustPresentBeforeAccept: [
        "currentPhaseScopeSummary",
        "includedDeferredExcludedBoundary",
        "nextPhasePreview",
        "conceptSummary",
        "businessObjectOperationSummary",
      ],
      confirmationMustOccurAfterPresentation: true,
    },
    clarificationConversationProtocol: {
      mode: "progressive_blocks",
      oneTopicPerTurn: true,
      maxOptionsPerQuestion: 5,
      avoidSchemaLanguageToUser: true,
      requiredBlocks: [
        "phase_scope",
        "concept_grounding",
        "frontend_experience",
        "final_summary",
      ],
      blockExecutionRules: [
        "Do not merge required clarification blocks.",
        "Each required block must be presented as its own user-visible step or a clearly separated section before it can be marked confirmed.",
        "A phase_scope option may mention concept or frontend context, but those mentions are context only and do not satisfy concept_grounding or frontend_experience.",
        ...phaseScopeOptionComparisonRules(),
        ...phaseScopeSelfCheckRules(),
        "Do not set clarificationProgress.confirmedBlocks for a block until the user has seen that block's dedicated question or summary and confirmed or corrected it.",
        "The concept_grounding block must first map every confirmed scope.included item to its applicable requirement details before asking for concept confirmation.",
        ...scopeItemCoverageClarificationRules(),
        ...businessScenarioConfirmationRules(),
        ...decisionImpactOrderingRules(),
        ...businessLifecycleScanRules(),
        ...conceptGroundingPresentationRules(),
        ...conceptGroundingSelfCheckRules(),
        "The concept_grounding block must clarify applicable objects or subjects, actions or behaviors, inputs or fields, preconditions, validation or blocking reasons, success state/data/UI/API/result changes, visible or returned feedback, and unresolved notes before final_summary when those details apply.",
        ...businessObjectOperationClarificationRules(),
        "The final_summary block must be shown after all applicable prior blocks and must present one user-facing pre-submit coverage checklist, not a one-sentence summary and not separate reconfirmation rounds.",
        "The frontend_experience block must clarify page operation paths before final_summary when UI or user-visible workflow applies: how users find or receive target objects, where actions start, and how results are observed.",
        ...frontendExperienceSelfCheckRules(),
        ...frontendExperiencePresentationRules(),
        ...frontendOperationPathClarificationRules(),
        "When the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, frontend/backend interaction, or user-facing operation paths, those details must be confirmed in their owning blocks and written into structured BrainstormCandidate fields; final_summary should only present a user-facing coverage checklist and any corrections.",
        "When those business-detail categories do not apply, the final_summary block must state the concrete not-applicable reason before the user confirms.",
        ...finalSummaryReviewRules(),
        ...brainstormCandidateSelfReviewRules(),
      ],
      blockConfirmationRules: {
        phase_scope: "Satisfied only after the phase_scope self-check is complete and the user confirms current phase included, excluded, deferred scope and nextPhasePreview direction, including the recommended option when real alternative phase cuts were presented.",
        concept_grounding: "Satisfied only after the concept_grounding self-check is complete and the user sees a dedicated concept and business-rules summary that first covers every confirmed scope.included item, then lists business scenario confirmation, decision impact ordering, lifecycle scan, applicable key concepts, objects or subjects, actions or behaviors, inputs or fields, preconditions, rule boundaries, blocking reasons, success changes, visible feedback, source refs, unresolved notes, and must-not-misinterpret-as guards when applicable, then confirms or corrects it.",
        frontend_experience: "Satisfied only after the frontend_experience self-check is complete and the user sees a dedicated frontend target question or summary covering UI need, experience level, main users/workflows, how users find or receive target objects, action entry points, result/refresh feedback, and explicit unacceptable shapes, then confirms or corrects it.",
        final_summary: "Satisfied only after the user sees one pre-submit coverage checklist using user-facing labels, with current phase goal, concrete scope coverage, business-rule coverage or skip reason, page-operation coverage or skip reason, deferred/not-done boundaries, next phase preview in user language, and any corrections after the prior applicable blocks.",
      },
      frontendBlockRequiredWhen: [
        "current phase adds or changes user-visible workflow",
        "current phase changes frontend experience level",
        "user asks to change or skip UI",
      ],
    },
    riskGuidance: {
      askWhenRealMoneyOrExternalProductionSystemsAreImplied: true,
      separateSimulationFromProductionExecution: true,
      doNotAssumeDangerousCapabilitiesAreIncluded: true,
    },
    confirmationRules: {
      mustShowSummaryBeforeAccept: true,
      mustWaitForExplicitUserConfirmation: true,
      initialUserRequestNeverCountsAsConfirmation: true,
      mustPresentUnderstandingSummaryBeforeAccept: true,
      userConfirmationMustOccurAfterPresentedSummary: true,
      doNotAcceptOnAmbiguousApproval: true,
      currentTurnExplicitConfirmationCounts: true,
      ifCurrentUserMessageConfirmsScopeDoNotAskAgain: true,
      consumeCurrentUserMessageBeforePromptingAgain: true,
    },
    rules: {
      agentOwnsClarificationConversation: true,
      useOnlyThisPhaseId: input.phase.phaseId,
      doNotReusePreviousPhaseRequestOrCandidate: true,
      useLatestRepositoryContextInsteadOfPhaseHistory: true,
      useNextPhasePreviewInsteadOfFutureRoadmap: true,
      doNotRequireHistoricalAcceptanceInCandidate: true,
      doNotProducePgcAacTaskPlanReviewOrCode: true,
      ifCurrentUserMessageContainsClearConfirmationWriteCandidateAndSubmit: "only_after_understanding_summary_presented",
      phaseScopeOptionComparison: {
        validationMode: "generation_guidance_only",
        requiredByDefault: true,
        nextPhaseSeedIsPreselectedAnswer: false,
        optionCount: { min: 2, max: 3 },
        recommendationMustPreserveCoreCurrentPhaseOutcome: true,
        narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems: true,
        scopeReductionRequiresExplicitUserConfirmation: true,
        atomicSingleScopeExceptionAllowedOnlyForAtomicScope: true,
        rules: phaseScopeOptionComparisonRules(),
      },
      nextPhasePreviewGeneration: {
        validationMode: "generation_guidance_only",
        rules: nextPhasePreviewCandidateRules(),
      },
      candidateSelfReview: {
        validationMode: "generation_guidance_only",
        rules: brainstormCandidateSelfReviewRules(),
      },
      requirementSemanticGrounding: {
        validationMode: "generation_guidance_only",
        compactRules: brainstormRequirementSemanticCompactRules(),
        finalSummaryBusinessDetailContract: {
          appliesWhenAgentFinds: [
            "business flows",
            "user operations",
            "state changes",
            "forms or fields",
            "validation or blocking rules",
            "frontend/backend interaction",
            "user-facing operation paths",
          ],
          requiredUserVisibleTopicsWhenApplicable: finalSummaryRequiredUserVisibleTopicsWhenApplicable(),
          notApplicableRule: "If none of these categories applies, state the concrete not-applicable reason in final_summary instead of fabricating business rules.",
          candidateFieldMapping: {
            scopeIncludedItems: "modules/actions/rules/fields/boundaries",
            acceptanceStatements: "verifiable business outcomes",
            businessFlowSummary: "business scenario, lifecycle actions, flow steps, preconditions, validation/blocking, success state",
            conceptGrounding: "high-risk concepts, business scenario meaning, decision impact, lifecycle semantics, applicable objects or subjects, actions or behaviors, inputs or fields, hard rules, state changes, blocking reasons, visible or returned feedback, unresolved notes, misunderstanding boundaries",
            frontendExperience: "target discovery, selection, input, display, action entry, refresh, and feedback expectations",
          },
          confirmedBlockDetailRetentionContract: {
            sourceOfTruth: "all_confirmed_brainstorm_blocks_plus_final_summary_corrections",
            finalSummaryRole: "review_gate_and_delta_confirmation_not_the_only_candidate_source",
            candidateFields: [
              "scope.included[].items",
              "scope.excluded",
              "scope.deferred",
              "acceptance[].statement",
              "domainModel.businessFlows[].summary",
              "conceptGrounding.phaseConceptGrounding.concepts[].explanation",
              "frontendExperience.dataViews/actions/operationPaths",
              "frontendExperienceDelta.dataViewDeltas/actionDeltas/operationPathDeltas",
              "phasePlan.nextPhasePreview",
            ],
            rules: confirmedBlockDetailRetentionRules(),
          },
          blockSelfCheckContract: {
            phase_scope: {
              owningBlock: "phase_scope",
              rules: phaseScopeSelfCheckRules(),
            },
            concept_grounding: {
              owningBlock: "concept_grounding",
              rules: [
                ...businessScenarioConfirmationRules(),
                ...decisionImpactOrderingRules(),
                ...businessLifecycleScanRules(),
                ...conceptGroundingPresentationRules(),
                ...conceptGroundingSelfCheckRules(),
              ],
            },
            frontend_experience: {
              owningBlock: "frontend_experience",
              rules: [
                ...frontendExperienceSelfCheckRules(),
                ...frontendExperiencePresentationRules(),
              ],
            },
            final_summary: {
              owningBlock: "final_summary",
              rules: finalSummaryReviewRules(),
            },
          },
          finalSummaryReviewContract: {
            owningBlock: "final_summary",
            purpose: "Final summary is the user's last pre-submit coverage checklist before BrainstormCandidate submit. It confirms or corrects prior blocks; it is not the source of detailed requirements.",
            rules: finalSummaryReviewRules(),
          },
          scopeItemCoverageContract: {
            owningBlock: "concept_grounding",
            userLanguageRule: "Use the confirmed scope wording; do not expose internal schema language or force a fixed capability taxonomy.",
            candidateFields: ["scope.included[].items", "acceptance[].statement", "domainModel.businessFlows[].summary", "conceptGrounding.phaseConceptGrounding.concepts[].explanation", "frontendExperience/frontendExperienceDelta when UI applies"],
            rules: scopeItemCoverageCandidateRules(),
          },
          objectOperationContract: {
            owningBlock: "concept_grounding",
            userLanguageRule: "Use natural user-facing wording in the conversation; do not expose internal schema field names as if they were user choices.",
            candidateFields: ["scope.included[].items", "acceptance[].statement", "domainModel.businessFlows[].summary", "conceptGrounding.phaseConceptGrounding.concepts[].explanation", "frontendExperience/frontendExperienceDelta when UI applies"],
            rules: businessObjectOperationCandidateRules(),
          },
          frontendOperationPathContract: {
            owningBlock: "frontend_experience",
            userLanguageRule: "Use natural user-facing wording in the conversation; do not expose internal schema enum values.",
            presentationRules: frontendExperiencePresentationRules(),
            candidateFields: ["frontendExperience.dataViews", "frontendExperience.actions", "frontendExperience.operationPaths", "frontendExperienceDelta.dataViewDeltas", "frontendExperienceDelta.actionDeltas", "frontendExperienceDelta.operationPathDeltas"],
            rules: frontendOperationPathCandidateRules(),
          },
        },
        rules: brainstormRequirementSemanticRules(),
      },
    },
    enumRefs: {
      candidateStatus: ["confirmed", "needs_clarification", "blocked"],
      scopeBucket: ["included", "excluded", "deferred"],
      roadmapRequirement: ["required", "not_required"],
      phaseStatus: ["scope_confirmed", "proposed", "delivered", "paused", "skipped", "revised"],
      scopeSource: ["source_explicit", "user_confirmed", "user_overridden", "model_recommended", "derived"],
      acceptancePriority: ["must", "should", "could"],
      conceptGroundingMode: ["concepts_present", "none_required", "not_applicable"],
      conceptPhaseRelevance: ["current", "current_adjacent", "future", "deferred", "excluded"],
      conceptPriority: ["must_understand", "should_understand", "nice_to_understand"],
      conceptRiskFactor: ["business_invariant", "state_transition", "resource_consistency", "permission_boundary", "external_contract", "scope_confusion_risk", "user_visible_flow", "runtime_or_delivery_semantics", "frontend_experience_semantics"],
      clarificationBlock: ["phase_scope", "concept_grounding", "frontend_experience", "final_summary"],
      frontendExperienceLevel: ["none", "technical_demo", "usable_internal_product", "polished_product"],
      frontendTargetSelectionMode: ["query_and_select", "direct_id_lookup", "preselected_context", "not_applicable"],
      frontendActionEntryPoint: ["result_row_action", "detail_button", "form_submit", "bulk_action", "inline_action", "navigation_entry"],
      frontendResultObservationMode: ["list_refresh", "detail_refresh", "inline_status_update", "response_message", "not_applicable"],
      frontendInteractionState: ["loading", "success", "error", "empty", "business_blocking"],
    },
    outputContract: {
      format: "json",
      schemaRef: "brainstorm-candidate-v1",
      candidateFile: input.candidateFile,
      schemaShape: {
        schemaVersion: "1.0",
        candidateId: `brainstorm-candidate-${input.phase.phaseId}`,
        brainstormRunId: input.brainstormRunId,
        deliveryId: input.deliveryId,
        phaseId: input.phase.phaseId,
        status: "confirmed",
        requestSummary: {
          title: input.phase.name,
          oneLine: "Confirmed current phase goal.",
          businessGoal: "Confirmed current phase business goal.",
          complexity: "medium",
        },
        sources: input.contract.sources,
        scope: {
          included: [{
            id: `scope-${input.phase.phaseId}`,
            label: input.phase.name,
            items: [
              "Confirmed current phase item.",
              "Current phase key field set: identity/input/display/relationship/status/result-feedback fields when applicable.",
              "Business scenario, decision impact, or lifecycle action detail confirmed in the relevant Brainstorm block when applicable.",
            ],
            reason: "User confirmed this phase.",
            source: "user_confirmed",
          }],
          excluded: [],
          deferred: [],
          assumptions: [{
            id: `assumption-${input.phase.phaseId}-001`,
            text: "A concrete assumption that remains true for this phase candidate.",
            requiresConfirmation: false,
          }],
        },
        roadmap: {
          required: true,
          currentPhaseId: input.phase.phaseId,
          phases: [{
            phaseId: input.phase.phaseId,
            title: input.phase.name,
            status: "scope_confirmed",
            goal: input.phase.name,
            scopeRefs: [`scope-${input.phase.phaseId}`],
            acceptanceRefs: ["AC-current-phase-001"],
            dependsOn: input.fromPhaseId ? [input.fromPhaseId] : [],
          }],
        },
        phasePlan: {
          current: {
            phaseId: input.phase.phaseId,
            title: input.phase.name,
            goal: input.phase.name,
            scopeRefs: [`scope-${input.phase.phaseId}`],
            acceptanceRefs: ["AC-current-phase-001"],
            status: "scope_confirmed",
          },
          nextPhasePreview: {
            oneOf: [
              {
                kind: "candidate",
                suggestedPhaseId: "next phase id",
                title: "Source-grounded next phase candidate title",
                goal: "Concrete non-binding next phase candidate goal",
                scopePreview: ["Concrete source-grounded business object/action/workflow candidate"],
                reason: "Why another phase remains.",
              },
              {
                kind: "none",
                reason: "Why no next phase remains.",
              },
            ],
            rule: "Use kind=none only when the confirmed delivery has no deferred scope and no later roadmap phase.",
          },
        },
        domainModel: input.contract.domainModel,
        acceptance: [{
          id: "AC-current-phase-001",
          statement: "A concrete must acceptance for the confirmed current phase.",
          capabilityRefs: [],
          sourceRefs: input.contract.sources.map((source) => source.sourceId),
          priority: "must",
        }],
        userConfirmation: {
          confirmed: true,
          confirmedAt: new Date(0).toISOString(),
          confirmationSummary: "What the user explicitly confirmed for this phase.",
          confirmationBasis: {
            initialRequestOnly: false,
            summaryPresentedToUser: true,
            confirmedAfterSummary: true,
            presentedItems: ["currentPhaseScopeSummary", "includedDeferredExcludedBoundary", "nextPhasePreview", "conceptSummary", "businessObjectOperationSummary"],
          },
        },
        conceptGrounding: {
          phaseConceptGrounding: {
            mode: "concepts_present | none_required | not_applicable",
            reason: "Required when mode is none_required or not_applicable.",
            concepts: [{
              conceptId: `concept-${input.phase.phaseId}-001`,
              term: "Current phase key concept",
              normalizedName: "current_phase_key_concept",
              explanation: "Plain explanation shown to the user, including scope-item coverage, current business scenario meaning, decision impact, lifecycle semantics, current-phase object or subject semantics, key field meaning, supported actions or behaviors, inputs or fields, validation or blocking rules, state transition expectations, visible feedback, unresolved notes, and implementation misunderstanding boundaries when applicable.",
              mustNotMisinterpretAs: ["Incorrect interpretation"],
              phaseRelevance: "current",
              priority: "must_understand",
              attentionRank: 1,
              riskFactors: ["scope_confusion_risk"],
              scopeRefs: [`scope-${input.phase.phaseId}`],
              acceptanceRefs: ["AC-current-phase-001"],
              humanReadableReason: "Why this concept matters.",
            }],
          },
          glossaryUpdates: [],
        },
        conceptConfirmation: {
          shownToUser: true,
          confirmedConceptRefs: [`concept-${input.phase.phaseId}-001`],
          confirmationSummary: "Current phase concepts were shown to and confirmed by the user.",
        },
        clarificationProgress: {
          mode: "progressive_blocks",
          confirmedBlocks: [
            { block: "phase_scope", summary: "User confirmed current phase scope and boundaries.", confirmedByUser: true },
            { block: "concept_grounding", summary: "User confirmed current phase concepts.", confirmedByUser: true },
            { block: "frontend_experience", summary: "User confirmed or skipped frontend experience for this phase.", confirmedByUser: true },
            { block: "final_summary", summary: "User confirmed the final combined Brainstorm summary.", confirmedByUser: true },
          ],
          skippedBlocks: [],
          finalSummaryConfirmed: true,
        },
        frontendExperience: {
          required: true,
          kind: "business_application | technical_demo | none",
          experienceLevel: "none | technical_demo | usable_internal_product | polished_product",
          audiences: [{
            audienceId: "audience-current-phase",
            name: "Current phase user",
            primaryJobs: ["Operate the current phase workflow."],
          }],
          surfaces: [{
            surfaceId: "surface-current-phase",
            name: "Current phase workspace",
            audienceRefs: ["audience-current-phase"],
            primaryJobs: ["Complete current phase workflow."],
          }],
          dataViews: [{
            viewId: "view-current-phase-results",
            name: "Current phase result list or detail",
            purpose: "Let users find, select, or inspect the current phase target object before acting.",
            targetObject: "Business object users operate on, when applicable.",
            selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
            paginationRequired: true,
            defaultLoadsFirstPage: true,
            searchCriteria: [{
              criterionId: "criterion-confirmed-field",
              label: "User-facing query condition grounded in confirmed object fields or user wording.",
              fieldRef: "optional confirmed object/entity field ref",
              reason: "Why this query condition is needed for the operation path.",
              sourceRefs: input.contract.sources.map((source) => source.sourceId),
            }],
            criteriaUnclearNote: "If confirmed fields are insufficient, use a basic paginated list with no advanced filters and record this note.",
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          actions: [{
            actionId: "action-current-phase-operation",
            label: "User-facing operation name",
            targetObject: "Business object acted on, when applicable.",
            entryPoint: "result_row_action | detail_button | form_submit | bulk_action | inline_action | navigation_entry",
            inputFields: ["Confirmed input field needed for this action."],
            resultObservation: ["list_refresh", "response_message"],
            refreshPolicy: "refresh_current_query | refresh_detail | update_inline_state | show_message_only | not_applicable",
            successFeedback: ["Success message, refreshed row/detail, or changed status visible to the user."],
            blockingOrErrorFeedback: ["Business blocking reason or validation error visible to the user."],
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          operationPaths: [{
            pathId: "path-current-phase-operation",
            name: "Current phase operation path",
            userGoal: "What the user is trying to complete.",
            surfaceRef: "surface-current-phase",
            workflowRef: "flow-current-phase",
            targetObject: "Business object users operate on, when applicable.",
            selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
            selectionSummary: "Natural-language summary, e.g. paginated query results -> select record -> trigger action -> observe refreshed status.",
            dataViewRefs: ["view-current-phase-results"],
            actionRefs: ["action-current-phase-operation"],
            requiredStates: ["loading", "success", "error", "empty", "business_blocking"],
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          mustNot: ["single_page_form_stack", "unstyled_browser_default", "phase_by_phase_demo_append_only"],
          confirmationSummary: "User confirmed the frontend delivery level and page operation path for this phase in natural language.",
        },
        frontendExperienceDelta: {
          inheritsPrevious: true,
          currentPhaseImpact: "Use when this phase inherits the existing frontend target and only changes specific surfaces or workflows.",
          newSurfaceRequired: true,
          affectedSurfaceRefs: ["surface-current-phase"],
          affectedViewCandidates: ["Current phase workflow view"],
          dataViewDeltas: [{
            viewId: "view-current-phase-delta",
            name: "Current phase changed result list or detail",
            purpose: "Represent the current phase's changed target discovery or display path.",
            targetObject: "Business object users operate on, when applicable.",
            selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
            paginationRequired: true,
            defaultLoadsFirstPage: true,
            searchCriteria: [],
            criteriaUnclearNote: "Use when the current phase changes a UI path but confirmed search criteria remain unclear.",
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          actionDeltas: [{
            actionId: "action-current-phase-delta",
            label: "Changed current phase action",
            targetObject: "Business object acted on, when applicable.",
            entryPoint: "result_row_action | detail_button | form_submit | bulk_action | inline_action | navigation_entry",
            inputFields: [],
            resultObservation: ["response_message"],
            refreshPolicy: "refresh_current_query | refresh_detail | update_inline_state | show_message_only | not_applicable",
            successFeedback: ["Confirmed success feedback for the changed workflow."],
            blockingOrErrorFeedback: ["Confirmed blocking or error feedback for the changed workflow."],
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          operationPathDeltas: [{
            pathId: "path-current-phase-delta",
            name: "Changed current phase operation path",
            userGoal: "What the changed workflow lets the user complete.",
            surfaceRef: "surface-current-phase",
            workflowRef: "flow-current-phase",
            targetObject: "Business object users operate on, when applicable.",
            selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
            selectionSummary: "Natural-language summary of the changed discovery, action, and result observation path.",
            dataViewRefs: ["view-current-phase-delta"],
            actionRefs: ["action-current-phase-delta"],
            requiredStates: ["loading", "success", "error", "empty", "business_blocking"],
            sourceRefs: input.contract.sources.map((source) => source.sourceId),
          }],
          experienceLevelOverride: null,
          mustNotDelta: ["Do not downgrade the previous confirmed frontend experience."],
          confirmationSummary: "User confirmed how this phase changes or preserves the frontend target and page operation path.",
        },
        handoff: {
          ready: true,
          nextNode: "technical_baseline_generation",
          blockingReasons: [],
        },
        candidateRules: [
          "If clarificationProgress confirms frontend_experience, include frontendExperience or frontendExperienceDelta. If the frontend block is skipped, include skippedBlocks with a concrete reason and do not invent frontend work.",
          "Use currentFrontendExperienceRef as inherited frontend context only; the current phase still needs an explicit confirmed frontendExperience or frontendExperienceDelta when the frontend block is confirmed.",
          "frontendExperience/frontendExperienceDelta is a user-confirmed product target for AAC to consume later, not implementation detail.",
          "Write page operation path details into frontendExperience.dataViews/actions/operationPaths or frontendExperienceDelta.*Deltas; do not leave them only in confirmationSummary or chat.",
          "Do not show internal frontend enum values to the user during clarification. Use natural language when asking or summarizing.",
          "When deriving sources for this phase, read sourceFieldAccessHints: phase continuation input sources come from deliveryContextRef .sources[].sourceId/type and candidate output sources also use sources[].sourceId/type.",
          "Required clarification blocks must not be merged: phase_scope mentions are context only and do not satisfy concept_grounding or frontend_experience.",
          "Before setting conceptConfirmation.shownToUser=true, concept_grounding must show a scope-item coverage summary for every confirmed scope.included item. Each item must be covered, explicitly unresolved, or explicitly deferred; do not silently omit included scope.",
          "Set conceptConfirmation.shownToUser=true only after a dedicated concept_grounding block showed scope-item coverage plus applicable objects or subjects, actions or behaviors, inputs or fields, preconditions, validation or blocking reasons, success state/data/UI/API/result changes, visible or returned feedback, source refs, unresolved notes, and must-not-misinterpret-as guards.",
          "Set frontend_experience confirmed only after a dedicated frontend_experience block showed the UI target or skip reason to the user.",
          "Set finalSummaryConfirmed=true only after a dedicated final_summary block presented one user-facing pre-submit coverage checklist with concrete scope coverage, business-rule coverage or skip reason, page-operation coverage or skip reason, next phase preview in user language, deferred/not-done boundaries, and explicit corrections.",
          "If the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, or frontend/backend interaction, structured candidate fields must preserve those confirmed details using existing fields; do not rely on final_summary text and do not leave details for PGC, AAC, TaskPlan, or TaskExecution to rediscover from the original requirement.",
          "If those business-detail categories do not apply, final_summary must record the concrete reason and the candidate should avoid fabricating domain rules.",
          ...phaseScopeOptionComparisonRules(),
          ...brainstormCandidateSelfReviewRules(),
          ...brainstormRequirementSemanticRules(),
          ...nextPhasePreviewCandidateRules(),
        ],
      },
    },
    blockedOutput: {
      schemaRef: "brainstorm-candidate-blocked-v1",
      candidateFile: blockedFile,
      schemaShape: {
        schemaVersion: "1.0",
        candidateId: `brainstorm-candidate-${input.phase.phaseId}`,
        brainstormRunId: input.brainstormRunId,
        deliveryId: input.deliveryId,
        phaseId: input.phase.phaseId,
        status: "blocked",
        requestSummary: { title: input.phase.name, oneLine: "Why Brainstorm cannot proceed.", complexity: "unknown" },
        scope: { included: [], excluded: [], deferred: [], assumptions: [] },
        roadmap: { required: true, currentPhaseId: input.phase.phaseId, phases: [] },
        phasePlan: {
          current: {
            phaseId: input.phase.phaseId,
            title: input.phase.name,
            goal: input.phase.name,
            scopeRefs: [],
            acceptanceRefs: [],
            status: "scope_confirmed",
          },
          nextPhasePreview: {
            kind: "none",
            reason: "Blocked before next phase preview could be confirmed.",
          },
        },
        acceptance: [],
        userConfirmation: { confirmed: false, confirmationSummary: "Explain what is blocking." },
        handoff: { ready: false, nextNode: "blocked", blockingReasons: ["Describe missing input or unsafe ambiguity."] },
      },
    },
    submitCommand,
    createdAt: input.now,
  };
}

function lastCompletedPhaseId(delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>, activePhaseId: string): string | null {
  const activeIndex = delivery.phases.findIndex((phase) => phase.phaseId === activePhaseId);
  const candidates = activeIndex >= 0 ? delivery.phases.slice(0, activeIndex) : delivery.phases;
  return [...candidates].reverse().find((phase) => phase.status === "completed")?.phaseId ?? null;
}

async function withPhaseTransitionGitAdvisory(
  projectRoot: string,
  fromPhaseId: string | null,
  instruction: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  if (!fromPhaseId || !(await isGitRepo(projectRoot))) {
    return instruction;
  }
  return {
    ...instruction,
    advisories: [
      ...(
        Array.isArray(instruction.advisories)
          ? instruction.advisories.filter((item) => typeof item === "object" && item !== null)
          : []
      ),
      {
        kind: "git_checkpoint",
        blocking: false,
        phaseId: fromPhaseId,
        message: `建议在进入下一阶段需求确认前，为 ${fromPhaseId} 做一次 git checkpoint。这个操作不是必需的，不影响继续确认下一阶段范围。`,
        commands: [
          "git status --short",
          "git add <本阶段实际交付文件>",
          `git commit -m "Complete ${fromPhaseId}"`,
          "git push # optional",
        ],
        rules: [
          "Do not execute these commands unless the user explicitly asks.",
          "Do not block the current ask_user flow on this advisory.",
          "git commit is the recommended checkpoint; git push is optional.",
        ],
      },
    ],
  };
}

async function isGitRepo(projectRoot: string): Promise<boolean> {
  try {
    const result = await execFileAsync("git", ["rev-parse", "--is-inside-work-tree"], {
      cwd: projectRoot,
      maxBuffer: 1024 * 1024,
    });
    return result.stdout.trim() === "true";
  } catch {
    return false;
  }
}

function validateRepositoryContext(root: string, context: RepositoryContext, requestRef: string | null): Array<{ code: string; path: string; message: string }> {
  const issues: Array<{ code: string; path: string; message: string }> = [];
  if (requestRef && context.source.requestRef !== requestRef) {
    issues.push({ code: "REQUEST_REF_MISMATCH", path: "source.requestRef", message: "RepositoryContext source.requestRef must match the accepted request." });
  }
  const surfaceIds = new Set<string>();
  for (const [index, surface] of context.relevantSurfaces.entries()) {
    if (surfaceIds.has(surface.surfaceId)) {
      issues.push({ code: "DUPLICATE_SURFACE_ID", path: `relevantSurfaces.${index}.surfaceId`, message: "relevantSurfaces surfaceId must be unique." });
    }
    surfaceIds.add(surface.surfaceId);
    pushPathIssue(issues, root, `relevantSurfaces.${index}.path`, surface.path);
  }
  for (const [index, app] of context.repoOverview.primaryApplications.entries()) {
    pushPathIssue(issues, root, `repoOverview.primaryApplications.${index}.rootPath`, app.rootPath);
  }
  for (const [index, rootPath] of context.structureSignals.rootPaths.entries()) {
    pushPathIssue(issues, root, `structureSignals.rootPaths.${index}.path`, rootPath.path);
  }
  for (const [index, entry] of context.structureSignals.entryPoints.entries()) {
    pushPathIssue(issues, root, `structureSignals.entryPoints.${index}.path`, entry.path);
  }
  for (const [index, filePath] of context.structureSignals.configurationFiles.entries()) {
    pushPathIssue(issues, root, `structureSignals.configurationFiles.${index}`, filePath);
  }
  const capabilityIds = new Set<string>();
  for (const [index, capability] of context.existingCapabilities.entries()) {
    if (capabilityIds.has(capability.capabilityId)) {
      issues.push({ code: "DUPLICATE_CAPABILITY_ID", path: `existingCapabilities.${index}.capabilityId`, message: "existingCapabilities capabilityId must be unique." });
    }
    capabilityIds.add(capability.capabilityId);
    for (const surfaceRef of capability.surfaceRefs) {
      if (!surfaceIds.has(surfaceRef)) {
        issues.push({ code: "UNKNOWN_SURFACE_REF", path: `existingCapabilities.${index}.surfaceRefs.${surfaceRef}`, message: "Capability surfaceRefs must reference relevantSurfaces." });
      }
    }
  }
  for (const [index, readRef] of context.recommendedReadRefs.entries()) {
    pushPathIssue(issues, root, `recommendedReadRefs.${index}.path`, readRef.path);
    for (const surfaceRef of readRef.surfaceRefs ?? []) {
      if (!surfaceIds.has(surfaceRef)) {
        issues.push({ code: "UNKNOWN_SURFACE_REF", path: `recommendedReadRefs.${index}.surfaceRefs.${surfaceRef}`, message: "recommendedReadRefs surfaceRefs must reference relevantSurfaces." });
      }
    }
  }
  return issues;
}

function pushPathIssue(issues: Array<{ code: string; path: string; message: string }>, root: string, issuePath: string, filePath: string): void {
  if (path.isAbsolute(filePath)) {
    issues.push({ code: "ABSOLUTE_PATH", path: issuePath, message: "Path must be project-relative." });
    return;
  }
  if (filePath.split(/[\\/]+/).includes("..")) {
    issues.push({ code: "PATH_TRAVERSAL", path: issuePath, message: "Path cannot contain '..'." });
    return;
  }
  const normalized = filePath.replace(/\\/g, "/").replace(/^\.\/+/, "");
  if (normalized === ".git" || normalized.startsWith(".git/") || normalized === ".loom" || normalized.startsWith(".loom/") || normalized === "node_modules" || normalized.startsWith("node_modules/")) {
    issues.push({ code: "FORBIDDEN_PATH", path: issuePath, message: "Path cannot point to .git, .loom, or node_modules." });
    return;
  }
  const resolved = fromProjectRelative(root, normalized || ".");
  if (!resolved.startsWith(root)) {
    issues.push({ code: "PATH_OUTSIDE_PROJECT", path: issuePath, message: "Path must stay inside project root." });
  }
}

async function requireInitialized(projectRoot: string): Promise<void> {
  if (!(await pathExists(path.join(projectRoot, ".loom", "config.json")))) {
    throw stateNotInitialized(projectRoot);
  }
}

function resolveCliPath(projectRoot: string, filePath: string): string {
  return path.isAbsolute(filePath) ? filePath : path.join(projectRoot, filePath);
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values)];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function pad(value: number): string {
  return value.toString().padStart(3, "0");
}
