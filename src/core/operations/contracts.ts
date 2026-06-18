import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { ZodError } from "zod";
import { invalidArgument, stateCorrupted, stateNotInitialized } from "../errors";
import { brainstormContractSchema, type BrainstormContract } from "../schemas";
import {
  type ArchitectureAcceptResult,
  type ArchitectureArtifactContract,
  type ArchitectureArtifactRequest,
  type ArchitectureSectionCandidate,
  type ArchitectureSectionsGenerationRequest,
  type PlanningGenerationContract,
  type RepoSignalSet,
  type RequirementDetailsIndex,
  type TechnicalBaseline,
  type TechnicalBaselineRequest,
  architectureArtifactContractSchema,
  architectureArtifactRequestSchema,
  architectureSectionCandidateSchema,
  architectureSectionsGenerationRequestSchema,
  planningGenerationContractSchema,
  repoSignalSetSchema,
  runtimeDeliveryCodegenRequiredSchema,
  runtimeDeliveryStatusSchema,
  runtimeDeliveryVerificationBoundarySchema,
  technicalBaselineRequestSchema,
  technicalBaselineSchema,
} from "../contracts";
import { pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { getActiveLocator, getLocatorForBrainstormRun, loadDeliveryIndex, loadProjectStatus, resolveLocator, saveDeliveryIndex, updatePhase, upsertStatusDelivery } from "../state/delivery";
import {
  type DeliveryPhaseLocator,
  architectureContractPath,
  architectureCandidatePath,
  architectureLatestPath,
  architectureRequestPath,
  architectureSectionCandidatePath,
  architectureSectionVersionPath,
  architectureSessionPath,
  brainstormContractPath,
  fromProjectRelative,
  getLoomPaths,
  planningContractPath,
  planningLatestPath,
  repositoryContextPath,
  repoSignalSetPath,
  technicalBaselineCandidatePath,
  technicalBaselinePath,
  technicalBaselineRequestPath,
  toProjectRelative,
  workspaceLatestPath,
} from "../state/paths";
import {
  resetIssueCounter,
  issue,
  validateArchitectureArtifactCandidate,
  validatePlanningGenerationContract,
  validateTechnicalBaselineCandidate,
} from "../validators";
import {
  closeOperationLease,
  createOperationLease,
  operationRef,
  readOperationLease,
  updateRouteState,
} from "./control";
import { repairSubmitRouting } from "./repair-routing";
import { autoRunInstruction, instructionForRouteAction, postRepairSubmitRouting, withAutoRunnableTransition } from "./routing-instructions";
import { artifactGenerationProtocolPolicy, artifactInstructionPolicy, artifactRepairPolicy, compactContextReadStep } from "./output-policy";
import { agentActionContract } from "./agent-action";
import { referencedArtifactReadGuide } from "./artifact-read-guide";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "./request-manifest";
import {
  architectureSingleSectionCompletionBarrier,
  architectureSingleSectionCompletionCondition,
  architectureSingleSectionRequiredSteps,
  architectureSingleSectionWriteTarget,
} from "./architecture-section-completion";

export type DetectRepoSignalsInput = {
  projectRoot: string;
};

export type CreateTechnicalBaselineRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  brainstormRunId?: string;
  projectKind?: "greenfield" | "existing_project" | "unknown";
};

export type AcceptTechnicalBaselineInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  candidateFile: string;
};

export type CreatePlanningContractInput = {
  projectRoot: string;
  deliveryId?: string;
  brainstormRunId?: string;
  phaseId?: string;
};

export type CreateArchitectureRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  planningContractId?: string;
  replaceActive?: boolean;
};

export type AcceptArchitectureInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  candidateFile?: string;
  requestId?: string;
  repairId?: string;
};

export async function detectRepoSignals(input: DetectRepoSignalsInput): Promise<{
  signalSet: RepoSignalSet;
  signalSetPath: string;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const now = new Date().toISOString();
  const files = await listProjectFiles(root);
  const manifests = files.filter((file) => MANIFEST_FILES.has(file) || /(^|\/)(pom\.xml|build\.gradle|requirements\.txt|pyproject\.toml|go\.mod|Cargo\.toml|composer\.json|Gemfile|.*\.csproj)$/.test(file));
  const packageJson = await readJsonObjectIfExists(path.join(root, "package.json"));
  const scripts = extractScripts(packageJson);
  const signalSet: RepoSignalSet = {
    schemaVersion: "1.0",
    signalSetId: createId("rss"),
    projectKind: hasExistingProjectSignals(files, manifests) ? "existing_project" : "greenfield",
    signals: {
      manifests,
      packageManagers: detectPackageManagers(files),
      languages: detectLanguages(files),
      frameworkHints: detectFrameworkHints(files, packageJson),
      testHints: detectTestHints(files, packageJson),
      sourceSamples: files.filter((file) => /^(src|app|apps|packages|lib)\//.test(file)).slice(0, 30),
      scripts,
    },
    conflicts: [],
    confidenceHints: {},
    createdAt: now,
  };
  const parsed = repoSignalSetSchema.parse(signalSet);
  const absolutePath = repoSignalSetPath(root, parsed.signalSetId);
  await writeJsonAtomic(absolutePath, parsed);
  return {
    signalSet: parsed,
    signalSetPath: toProjectRelative(root, absolutePath),
  };
}

function hasExistingProjectSignals(files: string[], manifests: string[]): boolean {
  return manifests.length > 0 || files.some((file) =>
    file.startsWith("src/") ||
    file.startsWith("app/") ||
    file.startsWith("apps/") ||
    file.startsWith("packages/") ||
    file.startsWith("lib/")
  );
}

function technicalBaselineDecisionNeeds(
  projectKind: "greenfield" | "existing_project" | "unknown",
  hasPreviousTechnicalBaseline = false,
): string[] {
  if (projectKind === "greenfield") {
    return [
      "web client technology track when applicable",
      "app client technology track when applicable",
      "backend/service technology track",
      "database or persistence technology track",
      "ORM or data access technology track",
      "external services only when required by the confirmed requirement",
    ];
  }
  if (hasPreviousTechnicalBaseline) {
    return [
      "whether the current confirmed scope explicitly adds a new technology surface",
      "whether the current confirmed scope explicitly replaces a previous technology baseline element",
      "otherwise reuse the previous TechnicalBaseline unchanged for normal bugfix, repair, optimization, or feature work inside the existing stack",
    ];
  }
  return [
    "application architecture",
    "runtime and language",
    "frontend framework",
    "backend framework",
    "database",
    "test strategy",
    "local dev and deploy strategy",
  ];
}

function technicalBaselineSelectionGuidance(input: {
  projectKind: "greenfield" | "existing_project" | "unknown";
  hasPreviousTechnicalBaseline: boolean;
}): Record<string, unknown> | undefined {
  if (input.projectKind !== "greenfield" && !input.hasPreviousTechnicalBaseline) {
    return undefined;
  }
  return {
    schemaVersion: "1.0",
    purpose: input.projectKind === "greenfield"
      ? "Guide the agent-user technical baseline confirmation for a greenfield empty project before PGC."
      : "Guide the agent-user technical baseline confirmation when a previous baseline exists and the final candidate may add, replace, or conflict with stable baseline elements.",
    cliBoundary: {
      role: "CLI provides materials, common examples, output contract, and confirmation rules only.",
      doesNotDo: [
        "The CLI does not infer the concrete recommended stack for this requirement.",
        "The CLI does not parse the user's natural-language technology replies.",
        "The CLI does not participate in intermediate confirmation rounds.",
      ],
      requiredAgentLoop: [
        "Read the request refs and understand the confirmed requirement scope.",
        "Generate the concrete recommendation or baseline-change summary yourself.",
        "Talk with the user for as many rounds as needed.",
        "Submit technical-baseline accept only after the user explicitly confirms the final technology baseline.",
      ],
    },
    confirmationRules: [
      "User requirement confirmation is not technical-baseline confirmation.",
      "If the user accepts the recommendation directly, that reply can be the final technical-baseline confirmation.",
      "If the user adjusts part of the stack or specifies a custom stack, summarize the final baseline and ask for final confirmation before writing the candidate.",
      "Do not submit a confirmed TechnicalBaseline candidate while any core track is ambiguous. Mark a track as not_applicable/not_needed only when the requirement or user confirmation supports that.",
      "Testing, build, local run, and deployment preparation are derived later by AAC runtime_delivery, TaskPlan, TaskExecution, and deploy. Do not require first-screen user choices for them and do not reopen technical-baseline confirmation only to update those commands.",
      ...(input.hasPreviousTechnicalBaseline ? [
        "When previousTechnicalBaselineRef exists, unchanged baseline reuse is the default for normal bugfix, repair, optimization, or feature work inside the existing stack.",
        "Only a current confirmed scope that explicitly adds a new technology surface or replaces a previous baseline element needs explicit technical-baseline confirmation.",
        "Current repository scripts, test commands, build commands, start commands, generated files, or framework implementation nuances are implementation facts; do not treat them as user-facing technology baseline changes by themselves.",
        "Preserve previous baseline tracks that the user did not confirm changing.",
      ] : []),
    ],
    trackModel: {
      requiredFinalShape: "Use stack.tracks with web, app, backend, persistence, dataAccess, and externalServices keys. Each track should include status, selection, source, and rationale.",
      trackStatusValues: ["selected", "not_needed", "not_applicable", "user_custom"],
      sourceValues: ["agent_recommended_user_confirmed", "user_adjusted", "user_specified", "previous_baseline", "not_applicable"],
      coreTracks: ["web", "app", "backend", "persistence", "dataAccess", "externalServices"],
      customTechnologyPolicy: "Common options are examples, not a whitelist. User-specified technologies outside these examples are allowed, but mark the relevant track source as user_specified or user_custom and include it in the final confirmation summary and reasoningSummary.",
    },
    recommendationBasis: {
      authority: "Use the complete BrainstormContract as the product-scope authority for the first greenfield TechnicalBaseline recommendation.",
      mustRead: [
        "contextRefs.brainstormContractRef summary and deliveryContext.originalRequest",
        "scope.included, scope.deferred, scope.excluded, and assumptions",
        "domainModel capability groups and business flows",
        "frontendExperience when present",
        "roadmap phases, deferred scope, and known next-phase previews",
      ],
      currentPhaseLensRole: "currentPhaseLens identifies the first implementation slice only. Do not choose the initial technology baseline from the current phase scope alone when the full requirement or roadmap implies later product surfaces, persistence scale, app clients, services, integrations, or operational needs.",
      recommendationRule: "Recommend a stable baseline for the full confirmed delivery/roadmap horizon; explain when the current phase can start small inside that baseline without hiding later known needs.",
    },
    userFacingConfirmationProtocol: {
      mandatorySections: [
        "Recommendation basis: summarize the full requirement/roadmap signals used, not only the current phase.",
        "Recommended final baseline: list every core track with selection and short rationale.",
        "Adjustable technology range: show common examples for every core track so the user knows how to modify the recommendation.",
        "Reply format: show canonical key=value examples using web, app, backend, persistence, dataAccess, and externalServices.",
        "Final confirmation rule: if the user changes anything, summarize the final baseline and ask for explicit confirmation before submitting.",
      ],
      wordingRules: [
        "Do not present the recommendation as based only on the first phase or current small implementation slice.",
        "Do not omit the adjustable technology range.",
        "Do not present backend options as bare language-only labels when a mainstream framework choice is expected; show language + framework combinations in user-facing examples.",
        "Do not use db or orm as the primary reply keys; use persistence and dataAccess in the primary examples.",
        "It is fine to understand db as persistence and orm as dataAccess when the user writes those aliases, but normalize the final candidate to stack.tracks.persistence and stack.tracks.dataAccess.",
      ],
    },
    commonOptions: {
      web: {
        label: "Web client",
        examples: ["Next.js", "React + Vite", "Vue + Vite", "SvelteKit", "Astro", "No Web client"],
      },
      app: {
        label: "App client",
        examples: ["No App client", "React Native + Expo", "Flutter", "iOS Native (Swift / SwiftUI)", "Android Native (Kotlin / Jetpack Compose)", "Hybrid WebView (Capacitor / Ionic)", "PWA"],
      },
      backend: {
        label: "Backend / service",
        examples: ["Next.js + Server Actions / Route Handlers / SSR", "Node.js + Fastify", "Node.js + Express", "Node.js + NestJS", "Python + FastAPI", "Python + Django", "Java + Spring Boot", "Go + net/http or Gin", ".NET + ASP.NET Core", "No independent backend"],
      },
      persistence: {
        label: "Database / persistence",
        examples: ["SQLite", "PostgreSQL", "MySQL", "MongoDB", "File storage / local JSON", "No persistence yet"],
      },
      dataAccess: {
        label: "ORM / data access",
        examples: ["Prisma", "Drizzle", "TypeORM", "SQLAlchemy", "Django ORM", "Spring Data JPA", "MyBatis Plus", "Entity Framework", "Raw SQL / lightweight wrapper", "No ORM"],
      },
      externalServices: {
        label: "External services",
        examples: ["None", "User specified", "Only recommend services explicitly required by the confirmed requirement"],
      },
    },
    shorthandNormalization: {
      backend: [
        "If the user writes backend=Java without a framework, normalize it to Java + Spring Boot unless they explicitly name a different Java backend stack.",
        "If the user writes backend=Python without a framework, normalize it to Python + FastAPI for service/backend work unless the requirement or user explicitly points to Django-style site/admin/content capabilities.",
        "If the user writes backend=Node.js without a framework, ask for or summarize a concrete Node.js framework choice such as Fastify, Express, or NestJS before final confirmation.",
        "If the user writes backend=.NET without a framework, normalize it to .NET + ASP.NET Core unless they explicitly name another .NET backend stack.",
      ],
      dataAccessCompatibility: [
        "When backend is Java + Spring Boot and dataAccess is not specified, recommend Spring Data JPA or MyBatis Plus explicitly before final confirmation; do not leave it as generic Java persistence.",
        "When backend is Python + FastAPI and dataAccess is not specified, recommend SQLAlchemy or SQLModel explicitly before final confirmation.",
        "When backend is Python + Django and dataAccess is not specified, recommend Django ORM explicitly before final confirmation.",
      ],
    },
    recommendationPrinciples: [
      "Prefer mainstream, maintainable, community-mature technologies.",
      "Prefer technologies that match the confirmed product shape and implementation effort.",
      "For Web UI, TypeScript is preferred unless the user chooses otherwise.",
      "For small or medium local-first CRUD/admin systems, SQLite is a reasonable default unless the user needs a production multi-user database.",
      "Prefer integrated fullstack options when they reduce orchestration cost and still satisfy the product need.",
      "Respect explicit user technology choices even when they are outside common examples.",
      "Avoid niche stacks unless the user asks for them or the requirement clearly needs them.",
    ],
    replyProtocolForUser: {
      acceptRecommendation: "确认推荐方案",
      partialAdjustmentExample: "web=Vue+Vite, backend=Java+Spring Boot, persistence=PostgreSQL, dataAccess=Spring Data JPA, app=不需要, externalServices=不需要",
      fullCustomExample: "web=React+Vite, app=React Native+Expo, backend=Fastify, persistence=SQLite, dataAccess=Prisma, externalServices=不需要",
      finalConfirmationPrompt: "When the user did not directly accept the recommendation, present a final technology baseline summary and ask them to reply 确认技术栈 or 修改: ...",
    },
  };
}

export async function createTechnicalBaselineRequest(input: CreateTechnicalBaselineRequestInput): Promise<{
  request: TechnicalBaselineRequest;
  requestPath: string;
  lease: ReturnType<typeof operationRef>;
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = input.brainstormRunId
    ? await getLocatorForBrainstormRun(root, input.brainstormRunId)
    : await resolveLocator(root, input.deliveryId, input.phaseId);
  const now = new Date().toISOString();
  const projectKind = input.projectKind ?? await inferProjectKindForBaseline(root, locator);
  const requestId = createId("tbr");
  const brainstorm = await loadBrainstormForPlanning(root, input.brainstormRunId);
  const latestBrainstormRunId = await latestBrainstormRunIdForPhase(root, locator);
  const authoritativeBrainstormRunId = latestBrainstormRunId ?? input.brainstormRunId ?? brainstorm.brainstormRunId;
  const phase = selectPlanningPhase(brainstorm, locator.phaseId);
  const repositoryContextRef = await repositoryContextRefForBaseline(root, locator, projectKind);
  const previousTechnicalBaselineRef = await previousTechnicalBaselineRefForBaseline(root, locator);
  const repoSignalSetRef = projectKind === "existing_project"
    ? (await detectRepoSignals({ projectRoot: root })).signalSetPath
    : undefined;
  const brainstormContractRef = toProjectRelative(root, brainstormContractPath(root, locator.deliveryId));
  const candidateFile = toProjectRelative(root, technicalBaselineCandidatePath(root, locator, requestId));
  const blockedFile = candidateFile.replace(/candidate\.json$/, "blocked.json");
  const requiredContextReadFields = [
    ...(repoSignalSetRef ? ["contextRefs.repoSignalSetRef"] : []),
    ...(previousTechnicalBaselineRef ? ["contextRefs.previousTechnicalBaselineRef"] : []),
  ];
  const selectionGuidance = technicalBaselineSelectionGuidance({
    projectKind,
    hasPreviousTechnicalBaseline: Boolean(previousTechnicalBaselineRef),
  });
  const selectionGuidanceReadFields = selectionGuidance ? ["selectionGuidance"] : [];
  const optionalContextReadFields = [
    ...(repositoryContextRef ? ["contextRefs.latestRepositoryContextRef"] : []),
  ];
  const submitCommand = {
    name: "technical-baseline accept",
    argv: [
      "technical-baseline",
      "accept",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--candidate-file",
      "{candidateFile}",
    ],
  };
  const request: TechnicalBaselineRequest = {
    schemaVersion: "1.0",
    requestId,
    agentAction: agentActionContract({
      actionKind: "generate_candidate",
      instruction: selectionGuidance
        ? "Follow selectionGuidance.userFacingConfirmationProtocol first. Generate the final TechnicalBaseline candidate only after the required user technical-baseline confirmation is complete. Write it to outputContract.candidateFile, then run submitCommand exactly."
        : "Generate one TechnicalBaseline candidate from this request, write it to outputContract.candidateFile, then run submitCommand exactly.",
      read: {
        required: ["this request", "referencedArtifactReadGuide", "contextRefs.brainstormContractRef", ...requiredContextReadFields, ...selectionGuidanceReadFields, "currentPhaseLens", "decisionNeeds", "constraints", "enumRefs", "outputContract.schemaShape"],
        optional: optionalContextReadFields,
        displayPolicy: "compact",
      },
      write: {
        candidateFile,
        blockedFile,
        rules: [
          "Write only outputContract.candidateFile for the candidate.",
          "Use outputContract.schemaShape field names exactly, including alternatives[].name and alternatives[].tradeoff.",
          "Do not write accepted TechnicalBaseline files directly.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--candidate-file"],
        placeholders: { "{candidateFile}": candidateFile },
        runAfter: "candidateFile exists and validates against outputContract.schemaShape",
      },
      schema: {
        primary: "TechnicalBaseline",
        shapeLocation: "outputContract.schemaShape",
        enumLocation: "enumRefs",
      },
      stopConditions: ["blockedOutput is required", "submitCommand returns non-repairable failure"],
    }),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    operation: projectKind === "existing_project" ? "infer_existing_project_baseline" : "recommend_greenfield_baseline",
    projectKind,
    scope: projectKind === "existing_project" ? "project" : "roadmap",
    inputs: {
      brainstormContractPath: brainstormContractRef,
      brainstormRunId: authoritativeBrainstormRunId,
      phaseId: locator.phaseId,
    },
    contextRefs: {
      brainstormContractRef,
      ...(repoSignalSetRef ? { repoSignalSetRef } : {}),
      ...(repositoryContextRef ? { latestRepositoryContextRef: repositoryContextRef } : {}),
      ...(previousTechnicalBaselineRef ? { previousTechnicalBaselineRef } : {}),
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      brainstormContractRef,
      repoSignalSetRef,
      latestRepositoryContextRef: repositoryContextRef,
      previousTechnicalBaselineRef,
    }),
    currentPhaseLens: {
      phaseId: phase.phaseId,
      title: phase.name,
      includedScopeRefs: phase.scope.includedRefs,
      excludedScopeRefs: phase.scope.excludedRefs,
      deferredScopeRefs: phase.scope.deferredRefs,
      acceptanceRefs: phase.acceptanceRefs,
    },
    reusePolicy: {
      previousTechnicalBaseline: previousTechnicalBaselineRef ? "reuse_stable_stack_only" : "none",
      currentPhaseScopeAuthority: "brainstorm_contract",
      ...(repoSignalSetRef ? { repoSignalSetAuthority: "current_repo_signals" as const } : {}),
      ...(repositoryContextRef ? { repositoryContextAuthority: "current_code_facts" } : {}),
      ...(previousTechnicalBaselineRef ? {
        baselineConflictRule: "Reuse the previous TechnicalBaseline stable stack by default. Do not rewrite the baseline solely to restate current repo facts, scripts, generated files, or implementation details. If the current confirmed scope explicitly adds a new technology surface or replaces a previous stable stack element, do not silently continue: produce a TechnicalBaseline with status=needs_user_confirmation, requiresUserConfirmation=true, approval.type=none, and explain the baseline change for user confirmation.",
      } : {}),
    },
    ...(selectionGuidance ? { selectionGuidance } : {}),
    decisionNeeds: technicalBaselineDecisionNeeds(projectKind, Boolean(previousTechnicalBaselineRef)),
    constraints: {
      mustUse: [],
      mustAvoid: [],
      userPreferences: [],
      deploymentPreference: "local_first",
    },
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      technicalBaselineSourceRules: [
        ...(projectKind === "greenfield" ? [
          "For greenfield, read selectionGuidance. The CLI provides materials, examples, and confirmation rules only; you must understand the requirement, recommend the concrete technology baseline, and complete any user confirmation dialogue yourself before submitting a confirmed candidate.",
          "For the first greenfield baseline, base the recommendation on the complete BrainstormContract product scope and roadmap/deferred scope, not only on currentPhaseLens. currentPhaseLens is only the first implementation slice.",
          "Before asking the user to confirm, show the required user-facing sections from selectionGuidance.userFacingConfirmationProtocol: recommendation basis, recommended tracks, adjustable technology range, canonical reply format, and final confirmation rule.",
          "Do not submit a greenfield TechnicalBaseline candidate until the user explicitly confirms the final technology baseline. Intermediate recommendation/adjustment rounds stay in the chat and are not CLI interactions.",
          "Testing, build, local run, and deployment preparation are derived later by AAC runtime_delivery, TaskPlan, TaskExecution, and deploy; do not ask the user to choose them as first-screen technology tracks unless the user volunteers a preference.",
        ] : []),
        ...(repoSignalSetRef ? [
          "For existing_project, read contextRefs.repoSignalSetRef and use RepoSignalSet as current repository technology evidence.",
        ] : []),
        ...(previousTechnicalBaselineRef ? [
          "Read contextRefs.previousTechnicalBaselineRef before choosing stack fields.",
          "Reuse previous TechnicalBaseline stable stack by default for bugfix, repair, optimization, or feature work inside the existing stack.",
          "Do not rewrite TechnicalBaseline only because RepositoryContext or RepoSignalSet contains more precise implementation facts, scripts, generated files, test commands, build commands, start commands, or framework-level details than the previous baseline.",
          "If the current confirmed scope explicitly adds a new technology surface or replaces a previous stable stack element, write status=needs_user_confirmation, requiresUserConfirmation=true, approval.type=none, and explain the baseline change. A normal user_confirmed source/approval from requirement clarification is not enough to bypass this technical-baseline confirmation gate.",
        ] : []),
      ],
      ...artifactGenerationProtocolPolicy(),
    },
    enumRefs: {
      projectKind: ["greenfield", "existing_project", "unknown"],
      baselineStatus: ["draft", "needs_user_confirmation", "auto_accepted", "confirmed", "blocked", "superseded"],
      baselineSource: [
        "user_specified",
        "user_confirmed",
        "detected_from_repo",
        "agent_inferred_from_repo_signals",
        "agent_recommended_for_greenfield",
      ],
      deploymentPreference: ["local_first", "deploy_requested", "unknown"],
    },
    outputContract: {
      candidateFile,
      schemaRef: "technical-baseline-v1",
      schemaShape: technicalBaselineSchemaShape(projectKind, locator, { hasPreviousTechnicalBaseline: Boolean(previousTechnicalBaselineRef) }),
    },
    submitCommand,
    blockedOutput: {
      schemaRef: "technical-baseline-blocked-v1",
      candidateFile: blockedFile,
      schemaShape: {
        schemaVersion: "1.0",
        requestId,
        status: "blocked",
        blockedReasons: [{ code: "BASELINE_INPUT_INSUFFICIENT", message: "TechnicalBaseline cannot be produced from available inputs." }],
      },
    },
    createdAt: now,
  };
  const parsed = technicalBaselineRequestSchema.parse(request);
  const absolutePath = technicalBaselineRequestPath(root, requestId, locator);
  const lease = await createOperationLease({
    projectRoot: root,
    locator,
    operationType: "technical_baseline_generation",
    refs: {
      requestRef: toProjectRelative(root, absolutePath),
      candidateFile: parsed.outputContract.candidateFile,
    },
  });
  try {
    await writeRequestManifestAtomic(root, absolutePath, parsed);
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "planning",
      phaseStatus: "planning",
      latestRefs: {
        technicalBaselineRequestId: requestId,
        technicalBaselineRequest: toProjectRelative(root, absolutePath),
      },
      nextAction: {
        type: "technical_baseline_request",
        source: "technical_baseline_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, absolutePath),
        reason: "TECHNICAL_BASELINE_REQUEST_CREATED",
        refs: {
          requestRef: toProjectRelative(root, absolutePath),
          candidateFile: parsed.outputContract.candidateFile,
          activeOperationType: "technical_baseline_generation",
        },
      },
    });
  } catch (error) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "technical_baseline_generation",
      reason: "request_write_failed",
    });
    throw error;
  }
  return {
    request: parsed,
    requestPath: toProjectRelative(root, absolutePath),
    lease: operationRef(lease),
    instruction: withAutoRunnableTransition({
      mode: "generate_candidate",
      ...artifactInstructionPolicy(),
      candidateKind: "TechnicalBaseline",
      requestRef: toProjectRelative(root, absolutePath),
      candidateFile: parsed.outputContract.candidateFile,
      submitCommand: parsed.submitCommand,
      generationSteps: [
        "Read requestRef.",
        compactContextReadStep,
        "Use referencedArtifactReadGuide for contextRefs; do not guess jq wrapper roots when reading Brainstorm, RepositoryContext, or previous TechnicalBaseline artifacts.",
        "Write the TechnicalBaseline candidate JSON to candidateFile.",
        "Run submitCommand after candidateFile exists.",
        "Follow the submit command response instruction after submit succeeds.",
      ],
      routingRule: "Read requestRef, write the TechnicalBaseline candidate to candidateFile, then run submitCommand. Do not run loom continue before submitCommand succeeds.",
      userMessage: "TechnicalBaselineRequest created. Generate the candidate JSON and submit it with the provided command.",
    }, {
      sourceCommand: "technical-baseline request",
      sourceSummary: "TechnicalBaselineRequest was created.",
      primaryAction: "generate_technical_baseline_and_submit",
    }),
  };
}

async function inferProjectKindForBaseline(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
): Promise<"greenfield" | "existing_project" | "unknown"> {
  const index = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const activeIndex = index.phases.findIndex((phase) => phase.phaseId === locator.phaseId);
  if (activeIndex > 0 || index.phases.some((phase, idx) => idx < activeIndex && phase.status === "completed")) {
    return "existing_project";
  }
  const projectSignalFiles = [
    "package.json",
    "tsconfig.json",
    "pom.xml",
    "build.gradle",
    "pyproject.toml",
    "go.mod",
    "Cargo.toml",
    "requirements.txt",
    "composer.json",
    "Gemfile",
  ];
  for (const file of projectSignalFiles) {
    if (await pathExists(path.join(projectRoot, file))) {
      return "existing_project";
    }
  }
  const files = await listProjectFiles(projectRoot);
  if (hasExistingProjectSignals(files, [])) {
    return "existing_project";
  }
  return "greenfield";
}

async function previousTechnicalBaselineRefForBaseline(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
): Promise<string | undefined> {
  const currentDeliveryBaseline = technicalBaselinePath(projectRoot, locator.deliveryId);
  if (await pathExists(currentDeliveryBaseline)) {
    return toProjectRelative(projectRoot, currentDeliveryBaseline);
  }

  const status = await loadProjectStatus(projectRoot);
  const candidateDeliveryIds: string[] = [];
  if (status.lastCompletedDeliveryId && status.lastCompletedDeliveryId !== locator.deliveryId) {
    candidateDeliveryIds.push(status.lastCompletedDeliveryId);
  }

  const historicalDeliveries = [...(status.deliveries ?? [])]
    .filter((delivery) =>
      delivery.status === "completed" &&
      delivery.deliveryId !== locator.deliveryId &&
      delivery.deliveryId !== status.lastCompletedDeliveryId
    )
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
  for (const delivery of historicalDeliveries) {
    candidateDeliveryIds.push(delivery.deliveryId);
  }

  for (const deliveryId of candidateDeliveryIds) {
    const candidate = technicalBaselinePath(projectRoot, deliveryId);
    if (await pathExists(candidate)) {
      return toProjectRelative(projectRoot, candidate);
    }
  }
  return undefined;
}

async function technicalBaselineRequestForCandidate(
  root: string,
  locator: DeliveryPhaseLocator,
  candidatePath: string,
): Promise<TechnicalBaselineRequest | null> {
  const relative = toProjectRelative(root, candidatePath);
  const parts = relative.split("/");
  const markerIndex = parts.lastIndexOf("technical-baseline");
  const requestId = markerIndex >= 0 ? parts[markerIndex + 1] : undefined;
  if (!requestId || parts[markerIndex + 2] !== "candidate.json") {
    return null;
  }
  const requestPath = technicalBaselineRequestPath(root, requestId, locator);
  if (!(await pathExists(requestPath))) {
    return null;
  }
  return technicalBaselineRequestSchema.parse(await hydrateRequestManifest(root, requestPath));
}

async function validateTechnicalBaselineReuseBoundary(
  root: string,
  request: TechnicalBaselineRequest,
  candidate: TechnicalBaseline,
): Promise<ReturnType<typeof validateTechnicalBaselineCandidate>["issues"]> {
  const issues: ReturnType<typeof validateTechnicalBaselineCandidate>["issues"] = [];
  if (request.projectKind !== "existing_project") {
    return issues;
  }
  const previousRef = request.contextRefs?.previousTechnicalBaselineRef;
  if (!previousRef) {
    return issues;
  }
  const previous = technicalBaselineSchema.parse(await readJsonFile(fromProjectRelative(root, previousRef)));
  const repoSignals = request.contextRefs?.repoSignalSetRef
    ? repoSignalSetSchema.parse(await readJsonFile(fromProjectRelative(root, request.contextRefs.repoSignalSetRef)))
    : null;
  const stackChanged = !stableStackEquivalent(previous.stack, candidate.stack);
  const signalConflicts = repoSignals ? repoSignalsConflictWithBaseline(repoSignals, previous) : [];
  const surfacedForUser =
    candidate.status === "needs_user_confirmation" ||
    candidate.requiresUserConfirmation === true;
  if ((stackChanged || signalConflicts.length > 0) && !surfacedForUser) {
    issues.push(issue("BASELINE_CONFLICT_REQUIRES_USER_CONFIRMATION", "/stack", "requires_user_decision"));
  }
  return issues;
}

function validateGreenfieldTechnicalBaselineBoundary(
  request: TechnicalBaselineRequest,
  candidate: TechnicalBaseline,
): ReturnType<typeof validateTechnicalBaselineCandidate>["issues"] {
  const issues: ReturnType<typeof validateTechnicalBaselineCandidate>["issues"] = [];
  if (request.projectKind !== "greenfield") {
    return issues;
  }
  if (
    candidate.projectKind !== "greenfield" ||
    candidate.status !== "confirmed" ||
    candidate.approval.type !== "user_confirmed" ||
    !candidate.approval.confirmedAt ||
    candidate.requiresUserConfirmation === true
  ) {
    issues.push(issue("GREENFIELD_BASELINE_CONFIRMATION_REQUIRED", "/approval", "requires_user_decision"));
  }
  if (!greenfieldStackTracksComplete(candidate.stack)) {
    issues.push(issue("GREENFIELD_BASELINE_TRACKS_INCOMPLETE", "/stack/tracks", "agent_repairable"));
  }
  return issues;
}

const GREENFIELD_CORE_TRACKS = ["web", "app", "backend", "persistence", "dataAccess", "externalServices"] as const;
const GREENFIELD_TRACK_STATUSES = new Set(["selected", "not_needed", "not_applicable", "user_custom"]);

function greenfieldStackTracksComplete(stack: Record<string, unknown>): boolean {
  const tracks = recordValue(stack.tracks);
  if (!tracks) {
    return false;
  }
  return GREENFIELD_CORE_TRACKS.every((track) => {
    const value = recordValue(tracks[track]);
    if (!value) {
      return false;
    }
    const status = typeof value.status === "string" ? value.status.trim() : "";
    const selection = typeof value.selection === "string" ? value.selection.trim() : "";
    return GREENFIELD_TRACK_STATUSES.has(status) && selection.length > 0;
  });
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function stableStackEquivalent(left: Record<string, unknown>, right: Record<string, unknown>): boolean {
  return JSON.stringify(normalizeStackForComparison(left)) === JSON.stringify(normalizeStackForComparison(right));
}

function normalizeStackForComparison(stack: Record<string, unknown>): Record<string, string[]> {
  return {
    runtimes: valuesForKeys(stack, ["runtime", "runtimes", "runtimeKind"]),
    languages: valuesForKeys(stack, ["language", "languages"]),
    frameworks: valuesForKeys(stack, ["framework", "frameworks", "frontendFramework", "backendFramework"]),
    packageManagers: valuesForKeys(stack, ["packageManager", "packageManagers"]),
    databases: valuesForKeys(stack, ["database", "databases", "databaseProvider"]),
    tracks: stackTrackSelectionsForComparison(stack),
  };
}

function stackTrackSelectionsForComparison(stack: Record<string, unknown>): string[] {
  const tracks = recordValue(stack.tracks);
  if (!tracks) {
    return [];
  }
  return Object.keys(tracks).sort().flatMap((trackName) => {
    const track = recordValue(tracks[trackName]);
    if (!track) {
      return [];
    }
    const status = typeof track.status === "string" ? normalizeStackToken(track.status) : "";
    const selection = typeof track.selection === "string" ? normalizeStackToken(track.selection) : "";
    if (!status && !selection) {
      return [];
    }
    return [`${normalizeStackToken(trackName)}:${status}:${selection}`];
  });
}

function valuesForKeys(value: Record<string, unknown>, keys: string[]): string[] {
  const values = new Set<string>();
  for (const key of keys) {
    collectStackValue(value[key], values);
  }
  return [...values].sort();
}

function collectStackValue(value: unknown, output: Set<string>): void {
  if (typeof value === "string" && value.trim().length > 0) {
    output.add(normalizeStackToken(value));
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) collectStackValue(item, output);
  }
}

function repoSignalsConflictWithBaseline(signals: RepoSignalSet, baseline: TechnicalBaseline): string[] {
  const stack = normalizeStackForComparison(baseline.stack);
  const conflicts: string[] = [];
  pushSignalConflict(conflicts, "packageManagers", stack.packageManagers, signals.signals.packageManagers);
  pushSignalConflict(conflicts, "frameworks", stack.frameworks, signals.signals.frameworkHints);
  pushSignalConflict(conflicts, "languages", stack.languages, signals.signals.languages);
  if (stack.runtimes.includes("node")) {
    const nodeSignals = new Set([
      ...signals.signals.packageManagers.map(normalizeStackToken),
      ...signals.signals.languages.map(normalizeStackToken),
    ]);
    const hasNodeSignal = ["npm", "pnpm", "yarn", "bun", "typescript", "javascript"].some((item) => nodeSignals.has(item));
    const hasOtherRuntimeSignal = ["maven", "gradle", "java", "python", "go", "rust"].some((item) => nodeSignals.has(item));
    if (!hasNodeSignal && hasOtherRuntimeSignal) {
      conflicts.push("runtime");
    }
  }
  return conflicts;
}

function pushSignalConflict(conflicts: string[], label: string, baselineValues: string[], signalValues: string[]): void {
  const baselineSet = new Set(baselineValues.map(normalizeStackToken));
  const signalSet = new Set(signalValues.map(normalizeStackToken));
  if (baselineSet.size === 0 || signalSet.size === 0) {
    return;
  }
  const intersects = [...baselineSet].some((item) => signalSet.has(item));
  if (!intersects) {
    conflicts.push(label);
  }
}

function normalizeStackToken(value: string): string {
  return value.trim().toLowerCase().replace(/\.js$/g, "").replace(/\s+/g, "-");
}

export async function acceptTechnicalBaseline(input: AcceptTechnicalBaselineInput): Promise<{
  accepted: boolean;
  status: TechnicalBaseline["status"] | "invalid_candidate";
  technicalBaselineId: string | null;
  issues: ReturnType<typeof validateTechnicalBaselineCandidate>["issues"];
  baselinePath: string | null;
  nextAction?: Record<string, unknown>;
  instruction?: Record<string, unknown>;
  repairInstruction?: Record<string, unknown>;
  postRepairSubmitRouting?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const candidatePath = resolveCliPath(root, input.candidateFile);
  const candidate = await readJsonFile(candidatePath);
  const result = validateTechnicalBaselineCandidate(candidate, root);
  const request = await technicalBaselineRequestForCandidate(root, locator, candidatePath);
  if (result.value && request) {
    result.issues.push(...validateGreenfieldTechnicalBaselineBoundary(request, result.value));
    result.issues.push(...await validateTechnicalBaselineReuseBoundary(root, request, result.value));
  }
  if (!result.value || result.issues.length > 0) {
    const candidateFile = toProjectRelative(root, candidatePath);
    const requiresUserDecision = result.issues.some((item) => item.repairability === "requires_user_decision");
    if (requiresUserDecision) {
      const nextAction = {
        type: "needs_user_decision" as const,
        source: "technical_baseline_accept",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: request ? toProjectRelative(root, technicalBaselineRequestPath(root, request.requestId, locator)) : null,
        reason: "TECHNICAL_BASELINE_REQUIRES_USER_CONFIRMATION",
        targetNode: "technical_baseline_request",
        refs: {
          candidateFile,
          requestRef: request ? toProjectRelative(root, technicalBaselineRequestPath(root, request.requestId, locator)) : null,
          activeOperationType: "technical_baseline_generation",
        },
      };
      if (request) {
        await updateRouteState({
          projectRoot: root,
          locator,
          deliveryStatus: "waiting_user",
          phaseStatus: "waiting_user",
          latestRefs: {
            technicalBaselineCandidate: candidateFile,
          },
          nextAction,
        });
      }
      return {
        accepted: false,
        status: "invalid_candidate",
        technicalBaselineId: result.value?.technicalBaselineId ?? null,
        issues: result.issues,
        baselinePath: null,
        nextAction,
        instruction: {
          mode: "ask_user",
          autoContinue: false,
          nextAction,
          candidateFile,
          schema: "TechnicalBaseline",
          issues: result.issues,
          userMessage: "TechnicalBaseline needs explicit user confirmation before planning can continue. Present the recommended or changed technology baseline, ask the user to confirm or correct it, then rewrite the same candidate file with the final confirmed baseline and submit technical-baseline accept again.",
          instructions: [
            "Do not repair this as an auto-runnable candidate issue.",
            "Do not fabricate approval.type=user_confirmed or confirmedAt.",
            "Ask the user to explicitly confirm or correct the final technology baseline.",
            "After the user confirms, rewrite the same candidateFile with status=confirmed, approval.type=user_confirmed, approval.confirmedAt, and requiresUserConfirmation=false or omitted.",
            "If the user changes the baseline, update stack, stack.tracks, constraints, evidence, reasoningSummary, and alternatives to match the confirmed decision.",
            "Then run technical-baseline accept again with the same candidate-file.",
          ],
          submitCommand: {
            name: "technical-baseline accept",
            argv: [
              "technical-baseline",
              "accept",
              "--delivery-id",
              locator.deliveryId,
              "--phase-id",
              locator.phaseId,
              "--candidate-file",
              candidateFile,
            ],
          },
        },
      };
    }
    return {
      accepted: false,
      status: "invalid_candidate",
      technicalBaselineId: result.value?.technicalBaselineId ?? null,
      issues: result.issues,
      baselinePath: null,
      nextAction: {
        type: "task_result_repair",
        reason: "TECHNICAL_BASELINE_CANDIDATE_INVALID",
      },
      repairInstruction: {
        mode: "repair_candidate",
        ...artifactRepairPolicy(),
        candidateFile,
        schema: "TechnicalBaseline",
        issues: result.issues,
        repairSubmitRouting: repairSubmitRouting({
          kind: "candidate",
          submitCommandName: "technical-baseline accept",
        }),
        instructions: [
          "Repair the TechnicalBaseline candidate JSON only.",
          "Do not modify project source code.",
          "Do not modify Brainstorm contract.",
          "Return a complete replacement candidate to the same candidateFile.",
          "Run technical-baseline accept again with the same candidate-file.",
        ],
        submitCommand: {
          name: "technical-baseline accept",
          argv: [
            "technical-baseline",
            "accept",
            "--delivery-id",
            locator.deliveryId,
            "--phase-id",
            locator.phaseId,
            "--candidate-file",
            candidateFile,
          ],
        },
      },
    };
  }

  const baseline = technicalBaselineSchema.parse({
    ...result.value,
    updatedAt: new Date().toISOString(),
  });
  const absolutePath = technicalBaselinePath(root, locator.deliveryId);
  await writeJsonAtomic(absolutePath, baseline);
  await updateDeliveryAfterTechnicalBaseline(root, locator, baseline, absolutePath);
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "technical_baseline_generation",
    reason: "technical_baseline_accepted",
  });
  return {
    accepted: true,
    status: baseline.status,
    technicalBaselineId: baseline.technicalBaselineId,
    issues: [],
    baselinePath: toProjectRelative(root, absolutePath),
    nextAction: await technicalBaselineNextAction(root, locator, baseline),
    instruction: await technicalBaselineInstruction(root, locator, baseline),
  };
}

async function technicalBaselineNextAction(projectRoot: string, locator: DeliveryPhaseLocator, baseline: TechnicalBaseline): Promise<Record<string, unknown>> {
  if (baseline.status === "needs_user_confirmation" || baseline.requiresUserConfirmation === true) {
    return {
      type: "needs_user_decision",
      reason: "TECHNICAL_BASELINE_REQUIRES_USER_CONFIRMATION",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      targetNode: "technical_baseline_request",
    };
  }
  const hasRepositoryContext = baseline.projectKind === "existing_project" && await phaseRepositoryContextExists(projectRoot, locator);
  const type = baseline.projectKind === "existing_project"
    ? hasRepositoryContext ? "planning_contract_create" : "repository_context_request"
    : baseline.projectKind === "greenfield"
      ? "planning_contract_create"
      : "needs_user_decision";
  return {
    type,
    reason: baseline.projectKind === "existing_project"
      ? hasRepositoryContext ? "TECHNICAL_BASELINE_READY_WITH_REPOSITORY_CONTEXT" : "TECHNICAL_BASELINE_READY_EXISTING_PROJECT"
      : baseline.projectKind === "greenfield"
        ? "TECHNICAL_BASELINE_READY_GREENFIELD"
        : "TECHNICAL_BASELINE_PROJECT_KIND_UNKNOWN",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
  };
}

async function technicalBaselineInstruction(projectRoot: string, locator: DeliveryPhaseLocator, baseline: TechnicalBaseline): Promise<Record<string, unknown> | undefined> {
  const action = await technicalBaselineNextAction(projectRoot, locator, baseline);
  if (baseline.projectKind === "unknown" || baseline.status === "needs_user_confirmation" || baseline.requiresUserConfirmation === true) {
    return {
      mode: "ask_user",
      autoContinue: false,
      nextAction: action,
      command: null,
      userMessage: baseline.projectKind === "unknown"
        ? "TechnicalBaseline projectKind is unknown. Ask the user to confirm whether this is a greenfield project or an existing project continuation."
        : "TechnicalBaseline requires user confirmation before changing or confirming the project technology baseline. Present the baseline conflict or change summary and ask the user to confirm or correct it.",
    };
  }
  const shouldCreateRepositoryContext = action.type === "repository_context_request";
  const argv = shouldCreateRepositoryContext
    ? ["repository-context", "request", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId]
    : ["planning-contract", "create", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId];
  return autoRunInstruction({
    actionType: String(action.type),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    reason: String(action.reason),
    targetNode: String(action.type),
    argv,
    userMessage: shouldCreateRepositoryContext
      ? "TechnicalBaseline accepted. Continue immediately by creating RepositoryContextRequest."
      : "TechnicalBaseline accepted. Continue immediately by creating PlanningGenerationContract.",
  });
}

async function updateDeliveryAfterTechnicalBaseline(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  baseline: TechnicalBaseline,
  baselinePath: string,
): Promise<void> {
  const index = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const needsUserConfirmation = baseline.status === "needs_user_confirmation" || baseline.requiresUserConfirmation === true;
  const hasRepositoryContext = baseline.projectKind === "existing_project" && await phaseRepositoryContextExists(projectRoot, locator);
  const nextType = needsUserConfirmation
    ? "needs_user_decision"
    : baseline.projectKind === "existing_project"
    ? hasRepositoryContext ? "planning_contract_create" : "repository_context_request"
    : baseline.projectKind === "greenfield"
      ? "planning_contract_create"
      : "needs_user_decision";
  updatePhase(index, locator.phaseId, {
    status: needsUserConfirmation ? "waiting_user" : "planning",
    latestRefs: {
      ...index.phases.find((phase) => phase.phaseId === locator.phaseId)?.latestRefs,
      technicalBaseline: toProjectRelative(projectRoot, baselinePath),
    },
    nextAction: {
      type: nextType,
      source: "technical_baseline",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      ref: toProjectRelative(projectRoot, baselinePath),
      reason: needsUserConfirmation
        ? "TECHNICAL_BASELINE_REQUIRES_USER_CONFIRMATION"
        : baseline.projectKind === "existing_project"
        ? hasRepositoryContext ? "TECHNICAL_BASELINE_READY_WITH_REPOSITORY_CONTEXT" : "TECHNICAL_BASELINE_READY_EXISTING_PROJECT"
        : baseline.projectKind === "greenfield"
          ? "TECHNICAL_BASELINE_READY_GREENFIELD"
          : "TECHNICAL_BASELINE_PROJECT_KIND_UNKNOWN",
      ...(baseline.projectKind === "unknown" || needsUserConfirmation ? { targetNode: "technical_baseline_request" } : {}),
    },
  });
  index.status = needsUserConfirmation ? "waiting_user" : "planning";
  index.updatedAt = baseline.updatedAt;
  await saveDeliveryIndex(projectRoot, index);
  await upsertStatusDelivery(projectRoot, index);
}

async function phaseRepositoryContextExists(projectRoot: string, locator: DeliveryPhaseLocator): Promise<boolean> {
  if (await pathExists(repositoryContextPath(projectRoot, locator))) {
    return true;
  }
  return pathExists(workspaceLatestPath(projectRoot, locator));
}

export async function createPlanningContract(input: CreatePlanningContractInput): Promise<{
  created: boolean;
  status: PlanningGenerationContract["status"];
  planningContractId: string;
  contract: PlanningGenerationContract;
  issues: ReturnType<typeof validatePlanningGenerationContract>["issues"];
  contractPath: string;
  instruction?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const now = new Date().toISOString();
  const locator = input.brainstormRunId
    ? await getLocatorForBrainstormRun(root, input.brainstormRunId)
    : await resolveLocator(root, input.deliveryId, input.phaseId);
  const latestBrainstormRunId = await latestBrainstormRunIdForPhase(root, locator);
  const brainstorm = await loadBrainstormForPlanning(root, latestBrainstormRunId ?? input.brainstormRunId);
  const baseline = await loadTechnicalBaseline(root);
  const phase = selectPlanningPhase(brainstorm, locator.phaseId);
  const repositoryContextRef = baseline?.projectKind === "existing_project" && await pathExists(repositoryContextPath(root, locator))
    ? toProjectRelative(root, repositoryContextPath(root, locator))
    : undefined;
  const brainstormContractRef = toProjectRelative(root, brainstormContractPath(root, locator.deliveryId));
  const requirementDetails = buildRequirementDetailsIndex(brainstorm, phase, brainstormContractRef);
  const contextRefs = {
    brainstormContractRef,
    ...(repositoryContextRef ? { repositoryContextRef } : {}),
    ...(brainstorm.conceptGroundingRefs?.deliveryConceptGlossaryRef ? { deliveryConceptGlossaryRef: brainstorm.conceptGroundingRefs.deliveryConceptGlossaryRef } : {}),
    ...(brainstorm.conceptGroundingRefs?.phaseConceptGroundingRef ? { phaseConceptGroundingRef: brainstorm.conceptGroundingRefs.phaseConceptGroundingRef } : {}),
    ...(brainstorm.frontendExperienceRefs?.confirmedFrontendExperienceRef ? { confirmedFrontendExperienceRef: brainstorm.frontendExperienceRefs.confirmedFrontendExperienceRef } : {}),
    ...(brainstorm.frontendExperienceRefs?.currentFrontendExperienceRef ? { currentFrontendExperienceRef: brainstorm.frontendExperienceRefs.currentFrontendExperienceRef } : {}),
  };
  const planningContractId = phase.handoff.planningContractId ?? `pgc-${phase.phaseId}`;
  const acceptanceById = new Map(brainstorm.acceptance.candidates.map((item) => [item.id, item]));
  const preconditionsReady =
    brainstorm.status === "confirmed" &&
    phase.status === "scope_confirmed" &&
    phase.handoff.readyForPlanning &&
    baseline !== null;

  const contract: PlanningGenerationContract = {
    schemaVersion: "1.0",
    planningContractId,
    status: preconditionsReady ? "ready" : "blocked",
    source: {
      brainstormRunId: latestBrainstormRunId ?? brainstorm.brainstormRunId,
      brainstormContractId: brainstorm.contractId,
      roadmapId: brainstorm.roadmap?.roadmapId ?? null,
      phaseId: phase.phaseId,
      technicalBaselineId: baseline?.technicalBaselineId ?? "missing-technical-baseline",
    },
    phaseScope: {
      phaseName: phase.name,
      phaseGoal: phase.goal,
      included: scopeItemsForRefs(brainstorm.scope.included, phase.scope.includedRefs),
      deferred: scopeItemsForRefs(brainstorm.scope.deferred, phase.scope.deferredRefs),
      excluded: scopeItemsForRefs(brainstorm.scope.excluded, phase.scope.excludedRefs),
      acceptanceCandidates: phase.acceptanceRefs.map((id) => {
        const acceptance = acceptanceById.get(id);
        return {
          id,
          statement: acceptance?.statement ?? `Missing acceptance candidate ${id}`,
          capabilityRefs: acceptance?.capabilityRefs ?? [],
          sourceRefs: acceptance?.sourceRefs ?? [],
          priority: acceptance?.priority ?? "must",
        };
      }),
    },
    contextRefs,
    referencedArtifactReadGuide: referencedArtifactReadGuide(contextRefs),
    ...(Object.keys(contextRefs).length > 0 ? {
      contextUsageRules: [
        "BrainstormContract is the authority for current phase scope and acceptance refs.",
        "RepositoryContext is repo-state evidence for existing_project flows.",
        "PGC only references RepositoryContext; Agent consumes it in AAC and TaskPlan generation.",
        "RepositoryContext must not be treated as current phase scope or acceptance coverage.",
        "Concept refs are confirmed Brainstorm semantic facts. CLI validates refs only; Agent performs semantic use.",
        "Frontend experience refs are user-confirmed product targets. AAC may engineer them but must not downgrade or override them without user decision.",
        "User-facing language is a delivery-level copy constraint. It applies to visible product text only; it must not translate code identifiers, API paths, database fields, enum values, or internal artifact ids.",
        "PGC mechanically preserves Brainstorm current-phase detail fields; do not summarize away phaseScope.*.items, phaseScope.acceptanceCandidates[].sourceRefs/capabilityRefs, planningInputs.businessFlows[].summary, concept refs, frontend refs, or frontend operation path details carried by those frontend refs.",
        "PGC requirementDetails.items is the canonical detail index after Brainstorm. AAC, TaskPlan, TaskExecution, and Review should reference detailId instead of duplicating full detail text.",
      ],
    } : {}),
    technicalBaseline: {
      technicalBaselineId: baseline?.technicalBaselineId ?? "missing-technical-baseline",
      status: baseline?.status ?? "blocked",
      scope: baseline?.scope ?? "project",
      summary: baseline ? summarizeBaseline(baseline) : {},
      mustFollow: true,
    },
    planningInputs: {
      businessGoal: brainstorm.summary.businessGoal,
      actors: brainstorm.domainModel.actors,
      capabilityGroups: brainstorm.domainModel.capabilityGroups,
      businessFlows: brainstorm.domainModel.businessFlows,
      frontendExperience: brainstorm.frontendExperience ?? null,
      userFacingLanguage: brainstorm.deliveryContext.userFacingLanguage ?? null,
      sourceRefs: brainstorm.sources.map((source) => source.sourceId),
      contextNotes: ["Brainstorm contract 已确认当前阶段范围。"],
    },
    requirementDetails,
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
      deployment: {
        defaultEnabled: false,
        requiresExplicitUserRequest: true,
      },
    },
    qualityGates: {
      requiresArchitectureBeforeTaskPlan: true,
      requiresAcceptanceCoverage: true,
      requiresVerificationEvidence: true,
    },
    handoff: {
      readyForArchitecture: preconditionsReady,
      readyForTaskPlan: false,
      blockingReasons: preconditionsReady ? [] : ["Brainstorm phase scope or TechnicalBaseline is not ready."],
      nextNode: preconditionsReady ? "architecture_artifact_contract" : "blocked",
    },
    createdAt: now,
    updatedAt: now,
  };

  const validation = validatePlanningGenerationContract(contract, baseline);
  const blockingReasons = validation.issues.map((item) => ({
    code: item.code,
    path: item.path,
    message: item.message,
  }));
  const finalContract = planningGenerationContractSchema.parse({
    ...contract,
    status: validation.issues.length === 0 ? "ready" : "blocked",
    handoff: {
      ...contract.handoff,
      readyForArchitecture: validation.issues.length === 0,
      blockingReasons,
      nextNode: validation.issues.length === 0 ? "architecture_artifact_contract" : "blocked",
    },
    updatedAt: new Date().toISOString(),
  });
  const absolutePath = planningContractPath(root, finalContract.planningContractId, locator);
  await writeJsonAtomic(absolutePath, finalContract);
  await writeJsonAtomic(planningLatestPath(root, locator), {
    schemaVersion: "1.0",
    planningContractId: finalContract.planningContractId,
    contractRef: toProjectRelative(root, absolutePath),
    updatedAt: finalContract.updatedAt,
  });
  await updateRouteState({
    projectRoot: root,
    locator,
    deliveryStatus: finalContract.status === "ready" ? "planning" : "blocked",
    phaseStatus: finalContract.status === "ready" ? "planning" : "blocked",
    latestRefs: {
      planningContract: toProjectRelative(root, absolutePath),
    },
    nextAction: finalContract.status === "ready"
      ? {
          type: "architecture_artifact_contract",
          source: "planning_contract",
          deliveryId: locator.deliveryId,
          phaseId: locator.phaseId,
          ref: toProjectRelative(root, absolutePath),
          reason: "PLANNING_CONTRACT_READY",
        }
      : {
          type: "needs_user_decision",
          source: "planning_contract",
          deliveryId: locator.deliveryId,
          phaseId: locator.phaseId,
          ref: toProjectRelative(root, absolutePath),
          reason: "PLANNING_CONTRACT_BLOCKED",
        },
  });

  return {
    created: true,
    status: finalContract.status,
    planningContractId: finalContract.planningContractId,
    contract: finalContract,
    issues: validation.issues,
    contractPath: toProjectRelative(root, absolutePath),
    ...(finalContract.status === "ready" ? {
      instruction: instructionForRouteAction({
        type: "architecture_artifact_contract",
        source: "planning_contract",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        targetNode: "architecture_artifact_contract",
        ref: toProjectRelative(root, absolutePath),
        reason: "PLANNING_CONTRACT_READY",
      }, locator),
    } : {}),
  };
}

export async function createArchitectureRequest(input: CreateArchitectureRequestInput): Promise<{
  request: ArchitectureSectionsGenerationRequest;
  requestPath: string;
  lease: ReturnType<typeof operationRef>;
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const activeLease = await readOperationLease(root, locator.deliveryId);
  if (activeLease?.status === "active" && new Date(activeLease.expiresAt).getTime() > Date.now()) {
    if (!input.replaceActive && activeLease.operationType === "architecture_generation" && activeLease.phaseId === locator.phaseId) {
      const existingRequestRef = typeof activeLease.refs.requestRef === "string" ? activeLease.refs.requestRef : null;
      if (existingRequestRef && await pathExists(path.join(root, existingRequestRef))) {
        const existingRequest = architectureSectionsGenerationRequestSchema.parse(await hydrateRequestManifest(root, path.join(root, existingRequestRef)));
        return {
          request: existingRequest,
          requestPath: existingRequestRef,
          lease: operationRef(activeLease),
          instruction: architectureGenerationInstruction(existingRequestRef, existingRequest, {
            recovery: true,
            userMessage: "ArchitectureSectionsGenerationRequest is already active. Generate the existing request candidate files and submit them; do not create another request.",
          }),
        };
      }
    }
    if (!input.replaceActive || activeLease.operationType !== "architecture_generation" || activeLease.phaseId !== locator.phaseId) {
      throw invalidArgument("Another loom operation is already active.", {
        operationId: activeLease.operationId,
        operationType: activeLease.operationType,
        expiresAt: activeLease.expiresAt,
      });
    }
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "architecture_generation",
      reason: "replaced_by_architecture_request",
    });
  } else if (input.replaceActive && activeLease?.status === "active" && activeLease.operationType === "architecture_generation" && activeLease.phaseId === locator.phaseId) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "architecture_generation",
      reason: "replaced_by_architecture_request",
    });
  } else if (input.replaceActive && activeLease?.status === "active") {
    throw invalidArgument("Cannot replace a different active loom operation.", {
      operationId: activeLease.operationId,
      operationType: activeLease.operationType,
      expiresAt: activeLease.expiresAt,
    });
  }
  const pgc = await loadPlanningContract(root, input.planningContractId, locator);
  const requestId = createId("arch-req");
  const sectionOutputs = architectureSectionOutputs(root, locator, requestId);
  const initialTarget = sectionOutputs[0] ?? null;
  const blockedFile = toProjectRelative(root, architectureCandidatePath(root, locator, requestId).replace(/candidate\.json$/, "blocked.json"));
  const previousRuntimeRef = await previousRuntimeDeliveryRef(root, locator);
  const sourceRefs = {
    planningContractRef: toProjectRelative(root, planningContractPath(root, pgc.planningContractId, locator)),
    technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
    brainstormContractRef: toProjectRelative(root, brainstormContractPath(root, locator.deliveryId)),
    ...(previousRuntimeRef ? { previousRuntimeDeliveryRef: previousRuntimeRef } : {}),
    ...(pgc.contextRefs?.repositoryContextRef ? { repositoryContextRef: pgc.contextRefs.repositoryContextRef } : {}),
    ...(pgc.contextRefs?.deliveryConceptGlossaryRef ? { deliveryConceptGlossaryRef: pgc.contextRefs.deliveryConceptGlossaryRef } : {}),
    ...(pgc.contextRefs?.phaseConceptGroundingRef ? { phaseConceptGroundingRef: pgc.contextRefs.phaseConceptGroundingRef } : {}),
    ...(pgc.contextRefs?.confirmedFrontendExperienceRef ? { confirmedFrontendExperienceRef: pgc.contextRefs.confirmedFrontendExperienceRef } : {}),
    ...(pgc.contextRefs?.currentFrontendExperienceRef ? { currentFrontendExperienceRef: pgc.contextRefs.currentFrontendExperienceRef } : {}),
  };
  const frontendExperienceSource = {
    confirmedFrontendExperienceRef: pgc.contextRefs?.confirmedFrontendExperienceRef ?? null,
    currentFrontendExperienceRef: pgc.contextRefs?.currentFrontendExperienceRef ?? null,
    repositoryContextRef: pgc.contextRefs?.repositoryContextRef ?? null,
    technicalBaselineRef: sourceRefs.technicalBaselineRef,
    authorityRule: "Brainstorm frontendExperience is the user-confirmed product target. RepositoryContext and TechnicalBaseline provide implementation facts only. AAC may refine architecture, surfaces, views, components, and review expectations, but must not downgrade or override the user-confirmed frontend target without a user decision.",
  };
  const requirementDetailTransfer = requirementDetailTransferProjection(pgc);
  const submitCommand = {
    name: "architecture accept",
    argv: [
      "architecture",
      "accept",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--request-id",
      requestId,
    ],
  };
  const request: ArchitectureSectionsGenerationRequest = {
    schemaVersion: "1.0",
    requestId,
    requestType: "architecture_sections_generation",
    agentAction: agentActionContract({
      actionKind: "generate_sections",
      instruction: "Generate Architecture section candidate files through the single-section continuation protocol. Write only the current target section file, then immediately run instruction.completionBarrier.followUpCommand.commandInvocation as the next agent tool action. When the follow-up command returns submit_existing_candidate, run submitCommand exactly.",
      read: {
        required: [
          "this request",
          "referencedArtifactReadGuide",
          "agentAction.write.currentTarget",
          "sourceRefs.planningContractRef",
          "sourceRefs.technicalBaselineRef",
          "contextProjection.phaseId",
          "contextProjection.planningContractId",
          "contextProjection.phaseScope",
          "contextProjection.requirementDetailTransfer",
          "allowedRefs",
          "generationProtocol",
          "rules.requirementDetailTransfer",
          "rules.allowedRefsAuthority",
          "outputContract.allowedRefsUsage",
          "frontendExperienceSource when generating frontend_experience",
        ],
        optional: ["sourceRefs.brainstormContractRef", "sourceRefs.repositoryContextRef", "sourceRefs.previousRuntimeDeliveryRef", "sourceRefs.deliveryConceptGlossaryRef", "sourceRefs.phaseConceptGroundingRef", "sourceRefs.confirmedFrontendExperienceRef", "sourceRefs.currentFrontendExperienceRef"],
        displayPolicy: "compact",
      },
      write: {
        currentTarget: architectureSingleSectionWriteTarget(initialTarget),
        sectionOutputs: sectionOutputs.map((item) => ({ section: item.section, candidateFile: item.candidateFile })),
        blockedFile,
        rules: [
          "Read agentAction.write.currentTarget before writing; it is the current instruction's concrete section/candidateFile.",
          "Single-section protocol: each auto-runnable instruction selects one targetSection and targetCandidateFile.",
          "For the current instruction, write only the selected targetSection to targetCandidateFile.",
          "After targetCandidateFile exists, immediately run instruction.completionBarrier.followUpCommand.commandInvocation as the next agent tool action; do not summarize progress or ask whether to continue first.",
          "Only write every outputContract.sectionOutputs[] entry in one turn if the instruction has no targetSection.",
          "Use agentAction.write.currentTarget.schemaShape, currentTarget.enumRefs, and currentTarget.generationRules as the current section contract.",
          "Use outputContract.sectionOutputs only as a fallback if the current instruction has no targetSection or currentTarget lacks schemaShape.",
          "Before writing any section candidate, read allowedRefs and outputContract.allowedRefsUsage.",
          "Every generated scopeRefs[] value must be selected exactly from allowedRefs.scopeRefs; if no allowed scope ref applies, write [] or block instead of inventing an id.",
          "Every generated acceptanceRefs[] value and coverage acceptanceId must be selected exactly from allowedRefs.acceptanceRefs; if no allowed acceptance ref applies, write [] or block instead of inventing AC ids.",
          "Every deferredRef must be selected exactly from allowedRefs.deferredScopeRefs and every excludedRef must be selected exactly from allowedRefs.excludedScopeRefs.",
          "Do not probe guessed jq paths. Use fieldAccessHints and agentAction.read/write when locating request fields.",
          "Use contextProjection.requirementDetailTransfer and sourceRefs.planningContractRef as the current phase requirement-detail authority. Do not drop phaseScope.*.items, acceptance sourceRefs/capabilityRefs, planningInputs.businessFlows summaries, concept refs, frontend refs, or frontendExperienceDetails while generating AAC sections.",
          "Use contextProjection.requirementDetailTransfer.requirementDetails.items as the canonical detail index when generating AAC artifact responsibilities and coverage.detailCoverage. Do not copy full detail summaries into detailCoverage.",
          "For frontend_experience, read contextProjection.requirementDetailTransfer.frontendExperienceDetails and frontendExperienceSource, then write frontendExperience.sourceRefs.brainstormFrontendExperienceRef from confirmedFrontendExperienceRef or currentFrontendExperienceRef when present.",
          "For runtime_delivery with runtimeDelivery.status=unchanged, set basis.previousRuntimeDeliveryRef exactly to sourceRefs.previousRuntimeDeliveryRef. If that source ref is absent, do not write status=unchanged; write status=modified or blocked based on the current phase facts.",
          "Do not assemble or write final aac.json.",
          "Do not use source code schemas; this request is the schema authority.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--request-id"],
        placeholders: {},
        runAfter: "the follow-up command returns submit_existing_candidate, or all outputContract.sectionOutputs candidate files already exist",
      },
      schema: {
        primary: "ArchitectureSectionCandidate",
        shapeLocation: "agentAction.write.currentTarget.schemaShape",
        enumLocation: "agentAction.write.currentTarget.enumRefs",
        allowedRefsLocation: "allowedRefs",
      },
      stopConditions: ["blockedOutput is required", "submitCommand returns non-repairable failure"],
    }),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      progressSignal: "candidate_files",
      heartbeatRequired: false,
      resumeViaContinue: true,
      ...artifactGenerationProtocolPolicy(),
    },
    requestOptimization: {
      profile: "compact_section_schema",
      version: 1,
      intent: "Keep the request small enough for Agent to generate section files without re-reading source code schemas.",
    },
    sourceRefs,
    referencedArtifactReadGuide: referencedArtifactReadGuide(sourceRefs),
    fieldAccessHints: {
      sourceRefs: "Use .sourceRefs for source contract refs.",
      previousRuntimeDeliveryRef: "Use .sourceRefs.previousRuntimeDeliveryRef as the only valid value for runtimeDelivery.basis.previousRuntimeDeliveryRef when runtimeDelivery.status=unchanged. Do not invent or derive a different previous runtime ref.",
      frontendExperienceSource: "Use .frontendExperienceSource to locate the user-confirmed frontend target before generating frontend_experience.",
      requirementDetailTransfer: "Use .contextProjection.requirementDetailTransfer plus sourceRefs.planningContractRef to preserve Brainstorm-confirmed current phase details: phaseScope.included/deferred/excluded items, acceptance statement/sourceRefs/capabilityRefs, planningInputs.businessFlows summaries, concept refs, frontend refs, and frontend operation path details from frontendExperienceDetails.",
      planningContractDetailSelectors: [
        ".phaseScope.included[].items",
        ".phaseScope.deferred[].items",
        ".phaseScope.excluded[].items",
        ".phaseScope.acceptanceCandidates[] | {id,statement,priority,sourceRefs,capabilityRefs}",
        ".planningInputs.businessFlows[].summary",
        ".planningInputs.frontendExperience.dataViews/actions/operationPaths",
        ".contextRefs.phaseConceptGroundingRef",
        ".contextRefs.deliveryConceptGlossaryRef",
        ".contextRefs.confirmedFrontendExperienceRef",
        ".contextRefs.currentFrontendExperienceRef",
        "confirmed/current frontend experience ref .dataViews/.actions/.operationPaths when present",
      ],
      sectionOutputs: "Use .outputContract.sectionOutputs for section outputs.",
      targetSection: "Use agentAction.write.currentTarget plus instruction.targetSection and instruction.targetCandidateFile. currentTarget is refreshed by loom continue for the active missing section.",
      compactJqExamples: [
        ".agentAction.write.currentTarget",
        ".sourceRefs",
        ".outputContract.sectionOutputs[] | {section,candidateFile,schemaRef}",
        ".agentAction.write.sectionOutputs",
      ],
    },
    contextProjection: {
      phaseId: locator.phaseId,
      planningContractId: pgc.planningContractId,
      phaseScope: pgc.phaseScope,
      requirementDetailTransfer,
    },
    frontendExperienceSource,
    allowedRefs: allowedArchitectureRefsFromPgc(pgc),
    rules: {
      onlyCurrentPhase: true,
      followTechnicalBaseline: true,
      doNotImplementDeferredScope: true,
      doNotProduceProseOnlyOutput: true,
      validatorIsMechanicalOnly: true,
      writeOneSectionPerTurn: true,
      doNotWriteFinalAacJson: true,
      frontendExperienceAuthority: "When frontendExperienceSource has confirmedFrontendExperienceRef or currentFrontendExperienceRef, the frontend_experience section must consume it and must not omit, downgrade, or override the user-confirmed frontend target without a user decision ref.",
      requirementDetailTransfer: {
        authority: "PlanningGenerationContract is the current phase requirement-detail authority for AAC generation.",
        rule: "Consume contextProjection.requirementDetailTransfer and sourceRefs.planningContractRef selectors while writing domain_contract, behavior, frontend_experience, and coverage. Preserve details instead of rediscovering them from raw requirements. Use requirementDetails.items as the canonical detail index and coverage.detailCoverage as detailId-to-artifact coverage.",
        selectors: [
          "planningContractRef.phaseScope.included[].items",
          "planningContractRef.phaseScope.acceptanceCandidates[].statement/sourceRefs/capabilityRefs",
          "planningContractRef.planningInputs.businessFlows[].summary",
          "planningContractRef.planningInputs.frontendExperience.dataViews/actions/operationPaths",
          "planningContractRef.contextRefs.phaseConceptGroundingRef",
          "planningContractRef.contextRefs.confirmedFrontendExperienceRef/currentFrontendExperienceRef and the referenced frontend experience .dataViews/.actions/.operationPaths when present",
        ],
      },
      singleSectionRouting: "For auto-runnable ArchitectureSections instructions with targetSection, write only targetCandidateFile, then immediately run instruction.completionBarrier.followUpCommand.commandInvocation before any chat summary.",
      allowedRefsAuthority: "All AAC scopeRefs, acceptanceRefs, acceptanceMatrix.acceptanceId, deferredRef, and excludedRef values must come from allowedRefs exactly; the Agent must not invent ids and the CLI validates this mechanically.",
    },
    enumRefs: architectureEnumRefs(),
    outputContract: {
      format: "json",
      schema: "ArchitectureSections",
      sectionOutputs,
      allowedRefsUsage: architectureAllowedRefsUsage(),
      blockedOutput: {
        candidateFile: blockedFile,
      },
      pathAuthority: {
        currentRequestOnly: true,
        currentPhaseId: locator.phaseId,
        currentRequestId: requestId,
        writeOnly: sectionOutputs.map((output) => output.candidateFile),
        rule: "Only section candidate files listed in outputContract.sectionOutputs belong to this architecture generation request.",
      },
    },
    blockedOutput: {
      schemaRef: "architecture-generation-blocked-v1",
      candidateFile: blockedFile,
      schemaShape: {
        schemaVersion: "1.0",
        requestId,
        status: "blocked",
        blockedReasons: [{
          code: "PGC_INSUFFICIENT",
          message: "PlanningGenerationContract is insufficient for AAC section generation.",
          nextNode: "planning_contract_create",
        }],
      },
    },
    submitCommand,
    validatorPolicy: {
      cliValidates: ["section shape", "references", "path safety", "assembled AAC"],
      cliDoesNotValidate: ["business semantics", "UI design quality", "best architecture choice"],
    },
    createdAt: new Date().toISOString(),
  };
  const parsed = architectureSectionsGenerationRequestSchema.parse(request);
  const absolutePath = architectureRequestPath(root, requestId, locator);
  const lease = await createOperationLease({
    projectRoot: root,
    locator,
    operationType: "architecture_generation",
    refs: {
      requestRef: toProjectRelative(root, absolutePath),
      sectionOutputs: architectureSectionOutputRefs(architectureRequestSectionOutputs(parsed)),
    },
  });
  try {
    await writeRequestManifestAtomic(root, absolutePath, parsed);
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "planning",
      phaseStatus: "planning",
      latestRefs: {
        architectureRequestId: requestId,
        architectureRequest: toProjectRelative(root, absolutePath),
      },
      nextAction: {
        type: "architecture_artifact_contract",
        source: "architecture_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, absolutePath),
        reason: "ARCHITECTURE_REQUEST_CREATED",
        refs: {
          requestRef: toProjectRelative(root, absolutePath),
          sectionOutputs: architectureSectionOutputRefs(architectureRequestSectionOutputs(parsed)),
          activeOperationType: "architecture_generation",
        },
      },
    });
  } catch (error) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "architecture_generation",
      reason: "request_write_failed",
    });
    throw error;
  }
  return {
    request: parsed,
    requestPath: toProjectRelative(root, absolutePath),
    lease: operationRef(lease),
    instruction: architectureGenerationInstruction(toProjectRelative(root, absolutePath), parsed),
  };
}

function architectureGenerationInstruction(
  requestRef: string,
  request: ArchitectureSectionsGenerationRequest,
  options?: {
    recovery?: boolean;
    userMessage?: string;
  },
): Record<string, unknown> {
  const sectionOutputs = architectureRequestSectionOutputs(request);
  const targetSection = sectionOutputs[0]?.section ?? null;
  const targetCandidateFile = sectionOutputs[0]?.candidateFile ?? null;
  return withAutoRunnableTransition({
    mode: "generate_candidate",
    ...artifactInstructionPolicy(),
    candidateKind: "ArchitectureSections",
    requestRef,
    blockedOutput: request.blockedOutput,
    submitCommand: request.submitCommand,
    completionBarrier: architectureSingleSectionCompletionBarrier(targetCandidateFile),
    recovery: options?.recovery ?? false,
    requestAlreadyExists: true,
    sectionGenerationMode: "single_section",
    targetSection,
    targetCandidateFile,
    mustNotRunCommandsBeforeSubmit: [
      "architecture request",
      "task-plan request",
      "technical-baseline request",
      "repository-context request",
    ],
    generationSteps: [
      "Read requestRef.",
      compactContextReadStep,
      "Use referencedArtifactReadGuide for sourceRefs before reading PGC, TechnicalBaseline, Brainstorm, or RepositoryContext artifacts.",
      "Use agentAction.write.currentTarget.schemaShape, currentTarget.enumRefs, allowedRefs, fieldAccessHints, and generationProtocol as the current section contract.",
      "Use request.allowedRefs and outputContract.allowedRefsUsage as the exact allowed id domain for scopeRefs, acceptanceRefs, acceptanceId, deferredRef, and excludedRef fields.",
      "Do not probe guessed jq paths; if a lookup returns null, use fieldAccessHints and agentAction.write.sectionOutputs.",
      "Write only targetSection to targetCandidateFile.",
      "The section candidate files are the only required progress signal; do not run a heartbeat command.",
      "After targetCandidateFile exists, immediately run instruction.completionBarrier.followUpCommand.commandInvocation as the next agent tool action so the CLI can scan file progress and return the next missing section or submit_existing_candidate.",
      "Do not send a progress summary or ask whether to continue between writing targetCandidateFile and running the follow-up command.",
      "Do not run submitCommand until the follow-up command returns submit_existing_candidate or all section files exist.",
    ],
    routingRule: "Generate only the target architecture section for the existing request, then immediately run instruction.completionBarrier.followUpCommand.commandInvocation to get the next section or submit instruction. Do not create another request and do not stop after writing one section.",
    userMessage: options?.userMessage ?? "ArchitectureSectionsGenerationRequest created. Generate the first section candidate file, then run the instruction completion follow-up command.",
  }, {
    sourceCommand: "architecture request",
    sourceSummary: "ArchitectureSectionsGenerationRequest was created.",
    primaryAction: "generate_architecture_section_and_continue",
    completionCondition: architectureSingleSectionCompletionCondition,
    requiredSteps: architectureSingleSectionRequiredSteps(),
  });
}

function architectureSectionOutputRefs(
  sectionOutputs: ArchitectureSectionsGenerationRequest["outputContract"]["sectionOutputs"],
): Array<{ section: string; schemaRef: string; candidateFile: string }> {
  return sectionOutputs.map((output) => ({
    section: output.section,
    schemaRef: output.schemaRef,
    candidateFile: output.candidateFile,
  }));
}

function architectureRequestSectionOutputs(
  request: ArchitectureSectionsGenerationRequest,
): ArchitectureSectionsGenerationRequest["outputContract"]["sectionOutputs"] {
  return request.outputContract.sectionOutputs;
}

export async function acceptArchitectureArtifact(input: AcceptArchitectureInput): Promise<ArchitectureAcceptResult> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const stale = input.requestId ? await staleArchitectureSubmitResult(root, locator, input.requestId) : null;
  if (stale) {
    return stale;
  }
  const assembledCandidate = input.requestId
    ? await assembleArchitectureCandidateFromSections(root, locator, input.requestId)
    : { value: await readJsonFile(resolveCliPath(root, requireCandidateFile(input.candidateFile))), issues: [] };
  if (assembledCandidate.issues.length > 0) {
    const pgc = await loadPlanningContract(root, undefined, locator);
    return {
      accepted: false,
      status: "needs_candidate_repair",
      architectureArtifactContractId: null,
      issues: assembledCandidate.issues,
      contractPath: null,
      repairRequest: buildArchitectureRepairRequest(root, locator, null, pgc, assembledCandidate.issues, input.requestId),
      repairInstruction: buildArchitectureRepairInstruction(root, locator, null, pgc, assembledCandidate.issues, input.requestId),
    };
  }
  const rawCandidate = assembledCandidate.value;
  const candidateSource = typeof rawCandidate === "object" && rawCandidate !== null && "source" in rawCandidate
    ? (rawCandidate as { source?: { planningGenerationContractId?: unknown } }).source
    : undefined;
  const planningContractId = typeof candidateSource?.planningGenerationContractId === "string"
    ? candidateSource.planningGenerationContractId
    : undefined;
  const pgc = await loadPlanningContract(root, planningContractId, locator);
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const validation = validateArchitectureArtifactCandidate(rawCandidate, pgc, baseline);

  if (!validation.value || validation.status !== "ready" || validation.issues.length > 0) {
    return {
      accepted: false,
      status: validation.status,
      architectureArtifactContractId: validation.value?.architectureArtifactContractId ?? null,
      issues: validation.issues,
      contractPath: null,
      repairRequest: buildArchitectureRepairRequest(root, locator, validation.value, pgc, validation.issues, input.requestId),
      repairInstruction: buildArchitectureRepairInstruction(root, locator, validation.value, pgc, validation.issues, input.requestId),
    };
  }

  const now = new Date().toISOString();
  const contract: ArchitectureArtifactContract = {
    ...validation.value,
    status: "ready",
    handoff: {
      ...validation.value.handoff,
      readyForTaskPlan: true,
      blockingReasons: [],
      nextNode: "task_plan",
    },
    updatedAt: now,
  };
  const absolutePath = architectureContractPath(root, contract.architectureArtifactContractId, locator);
  await writeJsonAtomic(absolutePath, contract);
  await writeJsonAtomic(architectureLatestPath(root, locator), {
    schemaVersion: "1.0",
    architectureArtifactContractId: contract.architectureArtifactContractId,
    contractRef: toProjectRelative(root, absolutePath),
    planningContractId: pgc.planningContractId,
    updatedAt: now,
  });
  await updateRouteState({
    projectRoot: root,
    locator,
    deliveryStatus: "planning",
    phaseStatus: "planning",
    latestRefs: {
      architectureArtifact: toProjectRelative(root, absolutePath),
      ...(input.requestId ? {
        architectureRequestId: input.requestId,
        architectureRequest: toProjectRelative(root, architectureRequestPath(root, input.requestId, locator)),
      } : {}),
    },
    nextAction: {
      type: "taskplan_generation",
      source: "architecture_artifact",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      ref: toProjectRelative(root, absolutePath),
      reason: "ARCHITECTURE_ARTIFACT_READY",
    },
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "architecture_generation",
    expectedRefs: input.requestId ? {
      requestRef: toProjectRelative(root, architectureRequestPath(root, input.requestId, locator)),
    } : undefined,
    reason: "architecture_accepted",
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "architecture_artifact_repair",
    reason: "architecture_repair_accepted",
  });

  return {
    accepted: true,
    status: "ready",
    architectureArtifactContractId: contract.architectureArtifactContractId,
    issues: [],
    contractPath: toProjectRelative(root, absolutePath),
    repairRequest: null,
    instruction: autoRunInstruction({
      actionType: "taskplan_generation",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: "ARCHITECTURE_ARTIFACT_READY",
      targetNode: "taskplan_generation",
      ref: toProjectRelative(root, absolutePath),
      argv: ["task-plan", "request", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "ArchitectureArtifactContract accepted. Continue immediately by creating TaskPlanGenerationRequest.",
    }),
    ...(input.repairId ? {
      postRepairSubmitRouting: postRepairSubmitRouting({
        source: "architecture_artifact_repair",
        submitCommand: "architecture accept",
        nextActionTypes: ["taskplan_generation"],
        repairedArtifact: { architectureArtifactContractId: contract.architectureArtifactContractId },
      }),
    } : {}),
  };
}

export async function loadRequiredTechnicalBaseline(projectRoot: string, locator?: DeliveryPhaseLocator): Promise<TechnicalBaseline> {
  const baseline = await loadTechnicalBaseline(projectRoot, locator);
  if (!baseline) {
    throw invalidArgument("TechnicalBaseline does not exist. Run technical-baseline request/accept first.");
  }
  return baseline;
}

async function staleArchitectureSubmitResult(
  root: string,
  locator: DeliveryPhaseLocator,
  requestId: string,
): Promise<ArchitectureAcceptResult | null> {
  const lease = await readOperationLease(root, locator.deliveryId);
  if (!lease || lease.status !== "active") {
    return null;
  }
  const requestRef = toProjectRelative(root, architectureRequestPath(root, requestId, locator));
  if (
    lease.operationType === "architecture_generation" &&
    lease.phaseId === locator.phaseId &&
    lease.refs.requestRef === requestRef
  ) {
    return null;
  }
  if (
    lease.operationType === "architecture_artifact_repair" &&
    lease.phaseId === locator.phaseId &&
    (lease.refs.requestRef === requestRef || lease.refs.originalRequestRef === requestRef)
  ) {
    return null;
  }
  return {
    accepted: false,
    status: "needs_candidate_repair",
    architectureArtifactContractId: null,
    issues: [{
      issueId: "issue-stale-architecture-submit",
      code: "STALE_INSTRUCTION",
      severity: "blocking",
      path: "/requestId",
      message: "Architecture accept targets a request that is no longer the active loom operation.",
      repairability: "blocked",
      repairHint: "Do not resubmit this stale architecture request. Run loom continue and follow the current active operation instruction.",
    }],
    contractPath: null,
    repairRequest: null,
    repairInstruction: null,
    instruction: withAutoRunnableTransition({
      mode: "run_cli",
      nextAction: {
        type: "continue_execution",
        reason: "STALE_ARCHITECTURE_SUBMIT",
        targetNode: "continue",
      },
      command: {
        name: "continue",
        argv: ["continue", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      },
      routingRule: "The submitted architecture request is stale. Run loom continue now and resume the active operation; do not retry architecture accept for this request.",
      userMessage: "Architecture accept request is stale. Continue from the current loom state.",
      activeOperation: {
        operationType: lease.operationType,
        requestRef: lease.refs.requestRef ?? null,
        taskId: lease.refs.taskId ?? null,
        taskPlanRunId: lease.refs.taskPlanRunId ?? null,
      },
    }, {
      sourceCommand: "architecture accept",
      sourceSucceeded: false,
      sourceSummary: "Architecture accept targeted a stale request and the active operation must be resumed.",
      primaryAction: "resume_active_operation",
    }),
  };
}

export async function loadTechnicalBaseline(projectRoot: string, locator?: DeliveryPhaseLocator): Promise<TechnicalBaseline | null> {
  const resolvedLocator = locator ?? await getActiveLocator(projectRoot);
  const absolutePath = technicalBaselinePath(projectRoot, resolvedLocator.deliveryId);
  if (!(await pathExists(absolutePath))) {
    return null;
  }
  return parseStored(technicalBaselineSchema, await readJsonFile(absolutePath), absolutePath);
}

export async function loadPlanningContract(projectRoot: string, planningContractId?: string, locator?: DeliveryPhaseLocator): Promise<PlanningGenerationContract> {
  const resolvedLocator = locator ?? await getActiveLocator(projectRoot);
  const id = planningContractId ?? await latestId(planningLatestPath(projectRoot, resolvedLocator), "planningContractId");
  if (!id) {
    throw invalidArgument("PlanningGenerationContract does not exist. Run planning-contract create first.");
  }
  const absolutePath = planningContractPath(projectRoot, id, resolvedLocator);
  if (!(await pathExists(absolutePath))) {
    throw invalidArgument("PlanningGenerationContract file does not exist.", { planningContractId: id });
  }
  return parseStored(planningGenerationContractSchema, await readJsonFile(absolutePath), absolutePath);
}

export async function loadArchitectureArtifact(projectRoot: string, architectureArtifactContractId?: string, locator?: DeliveryPhaseLocator): Promise<ArchitectureArtifactContract> {
  const resolvedLocator = locator ?? await getActiveLocator(projectRoot);
  const id = architectureArtifactContractId ?? await latestId(architectureLatestPath(projectRoot, resolvedLocator), "architectureArtifactContractId");
  if (!id) {
    throw invalidArgument("ArchitectureArtifactContract does not exist. Run architecture request/accept first.");
  }
  const absolutePath = architectureContractPath(projectRoot, id, resolvedLocator);
  if (!(await pathExists(absolutePath))) {
    throw invalidArgument("ArchitectureArtifactContract file does not exist.", { architectureArtifactContractId: id });
  }
  return parseStored(architectureArtifactContractSchema, await readJsonFile(absolutePath), absolutePath);
}

async function requireInitialized(projectRoot: string): Promise<void> {
  const paths = getLoomPaths(projectRoot);
  if (!(await pathExists(paths.configFile)) || !(await pathExists(paths.statusFile))) {
    throw stateNotInitialized(paths.root);
  }
}

async function loadBrainstormForPlanning(projectRoot: string, brainstormRunId?: string): Promise<BrainstormContract> {
  const locator = brainstormRunId
    ? await getLocatorForBrainstormRun(projectRoot, brainstormRunId)
    : await getActiveLocator(projectRoot);
  const absolutePath = brainstormContractPath(projectRoot, locator.deliveryId);
  if (!(await pathExists(absolutePath))) {
    throw invalidArgument("Brainstorm contract file does not exist.", { deliveryId: locator.deliveryId });
  }
  return parseStored(brainstormContractSchema, await readJsonFile(absolutePath), absolutePath);
}

async function latestBrainstormRunIdForPhase(projectRoot: string, locator: DeliveryPhaseLocator): Promise<string | undefined> {
  const index = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const phase = index.phases.find((item) => item.phaseId === locator.phaseId);
  return phase?.latestRefs.brainstormRunId ?? undefined;
}

async function repositoryContextRefForBaseline(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  projectKind: "greenfield" | "existing_project" | "unknown",
): Promise<string | undefined> {
  if (projectKind !== "existing_project") {
    return undefined;
  }
  const absolutePath = repositoryContextPath(projectRoot, locator);
  return await pathExists(absolutePath) ? toProjectRelative(projectRoot, absolutePath) : undefined;
}

function selectPlanningPhase(brainstorm: BrainstormContract, phaseId?: string): NonNullable<BrainstormContract["roadmap"]>["phases"][number] {
  if (!brainstorm.roadmap) {
    return {
      phaseId: phaseId ?? "phase-1",
      name: brainstorm.summary.title,
      status: brainstorm.status === "confirmed" ? "scope_confirmed" : "scope_confirming",
      goal: brainstorm.summary.businessGoal,
      scope: {
        includedRefs: brainstorm.scope.included.map((item) => item.id),
        deferredRefs: brainstorm.scope.deferred.map((item) => item.id),
        excludedRefs: brainstorm.scope.excluded.map((item) => item.id),
      },
      acceptanceRefs: brainstorm.acceptance.candidates.map((item) => item.id),
      dependsOn: [],
      handoff: {
        readyForPlanning: brainstorm.status === "confirmed",
        planningContractId: phaseId ? `pgc-${phaseId}` : "pgc-phase-1",
        planId: null,
      },
      confirmation: {
        confirmedBy: "user",
        confirmedAt: brainstorm.handoff.confirmedAt,
        sourcePatchIds: [],
      },
      nextActions: [],
    };
  }
  const selected = phaseId
    ? brainstorm.roadmap.phases.find((phase) => phase.phaseId === phaseId)
    : brainstorm.roadmap.phases.find((phase) => phase.phaseId === brainstorm.roadmap?.currentPhaseId) ?? brainstorm.roadmap.phases[0];
  if (!selected) {
    throw invalidArgument("Phase does not exist in roadmap.", { phaseId });
  }
  return selected;
}

function scopeItemsForRefs<T extends { id: string; label: string; items?: string[]; source: string }>(
  items: T[],
  refs: string[],
): Array<{ scopeId: string; label: string; items?: string[]; source?: string }> {
  return refs.map((ref) => {
    const item = items.find((candidate) => candidate.id === ref);
    return {
      scopeId: ref,
      label: item?.label ?? ref,
      ...(item?.items ? { items: item.items } : {}),
      source: item?.source,
    };
  });
}

function buildRequirementDetailsIndex(
  brainstorm: BrainstormContract,
  phase: NonNullable<BrainstormContract["roadmap"]>["phases"][number],
  brainstormContractRef: string,
): RequirementDetailsIndex {
  const allSourceIds = uniqueStrings(brainstorm.sources.map((source) => source.sourceId));
  const currentScopeRefs = new Set(phase.scope.includedRefs);
  const deferredScopeRefs = new Set(phase.scope.deferredRefs);
  const excludedScopeRefs = new Set(phase.scope.excludedRefs);
  const currentAcceptanceRefs = new Set(phase.acceptanceRefs);
  const items: RequirementDetailsIndex["items"] = [];
  const extractionWarnings: RequirementDetailsIndex["extractionWarnings"] = [];

  const addItem = (input: {
    kind?: RequirementDetailsIndex["items"][number]["kind"];
    title: string;
    summary: string;
    requiredForCurrentPhase: boolean;
    priority?: "must" | "should" | "could";
    sourceFieldRefs: string[];
    sourceRefs?: string[];
    scopeRefs?: string[];
    acceptanceRefs?: string[];
    conceptRefs?: string[];
    frontendRefs?: string[];
    unresolvedNote?: string | null;
  }): void => {
    const summary = input.summary.trim();
    if (!summary) {
      return;
    }
    const sourceFieldRefs = uniqueStrings(input.sourceFieldRefs);
    const kind = input.kind ?? inferRequirementDetailKind(summary);
    const detailId = stableRequirementDetailId(kind, sourceFieldRefs, summary);
    if (items.some((item) => item.detailId === detailId)) {
      return;
    }
    const item: RequirementDetailsIndex["items"][number] = {
      detailId,
      kind,
      title: truncateDetailTitle(input.title),
      summary,
      requiredForCurrentPhase: input.requiredForCurrentPhase,
      priority: input.priority ?? (input.requiredForCurrentPhase ? "must" : "could"),
      sourceFieldRefs,
      sourceRefs: uniqueStrings(input.sourceRefs && input.sourceRefs.length > 0 ? input.sourceRefs : allSourceIds),
      scopeRefs: uniqueStrings(input.scopeRefs ?? []),
      acceptanceRefs: uniqueStrings(input.acceptanceRefs ?? []),
      conceptRefs: uniqueStrings(input.conceptRefs ?? []),
      frontendRefs: uniqueStrings(input.frontendRefs ?? []),
      impactTags: inferRequirementDetailImpactTags(summary, kind),
      lifecycleStage: inferRequirementDetailLifecycleStage(summary),
      quality: inferRequirementDetailQuality(summary),
      unresolvedNote: input.unresolvedNote ?? inferRequirementDetailUnresolvedNote(summary),
    };
    items.push(item);
    if (item.quality === "thin") {
      extractionWarnings.push({
        warningId: stableRequirementDetailWarningId(item.detailId, "thin"),
        detailId: item.detailId,
        sourceFieldRef: item.sourceFieldRefs[0] ?? null,
        severity: "warning",
        message: "Requirement detail was extracted, but the source text is thin. Downstream stages must not invent missing business rules.",
      });
    }
  };

  const addScopeItems = (
    bucket: "included" | "deferred" | "excluded",
    scopeItems: BrainstormContract["scope"]["included"],
    activeRefs: Set<string>,
  ): void => {
    scopeItems.forEach((scopeItem, scopeIndex) => {
      if (!activeRefs.has(scopeItem.id)) {
        return;
      }
      const requiredForCurrentPhase = bucket === "included";
      const kind = bucket === "included" ? "scope_boundary" : "deferred_or_excluded_boundary";
      const values = scopeItem.items && scopeItem.items.length > 0 ? scopeItem.items : [scopeItem.label];
      values.forEach((value, itemIndex) => {
        const sourceFieldRef = scopeItem.items && scopeItem.items.length > 0
          ? `brainstorm.scope.${bucket}[${scopeIndex}].items[${itemIndex}]`
          : `brainstorm.scope.${bucket}[${scopeIndex}].label`;
        addItem({
          kind,
          title: `${scopeItem.label}: ${value}`,
          summary: value,
          requiredForCurrentPhase,
          priority: requiredForCurrentPhase ? "must" : "could",
          sourceFieldRefs: [sourceFieldRef],
          scopeRefs: [scopeItem.id],
          unresolvedNote: scopeItem.items && scopeItem.items.length > 0 ? null : "Scope item has no detailed items array; label was used as the detail source.",
        });
      });
      if (!scopeItem.items || scopeItem.items.length === 0) {
        extractionWarnings.push({
          warningId: stableRequirementDetailWarningId(scopeItem.id, "scope-items-empty"),
          detailId: null,
          sourceFieldRef: `brainstorm.scope.${bucket}[${scopeIndex}]`,
          severity: "warning",
          message: `Scope ${scopeItem.id} has no items array, so PGC could only index the scope label.`,
        });
      }
    });
  };

  addScopeItems("included", brainstorm.scope.included, currentScopeRefs);
  addScopeItems("deferred", brainstorm.scope.deferred, deferredScopeRefs);
  addScopeItems("excluded", brainstorm.scope.excluded, excludedScopeRefs);

  brainstorm.acceptance.candidates.forEach((acceptance, index) => {
    if (!currentAcceptanceRefs.has(acceptance.id)) {
      return;
    }
    addItem({
      kind: inferRequirementDetailKind(acceptance.statement, "acceptance_outcome"),
      title: acceptance.id,
      summary: acceptance.statement,
      requiredForCurrentPhase: true,
      priority: acceptance.priority,
      sourceFieldRefs: [`brainstorm.acceptance.candidates[${index}].statement`],
      sourceRefs: acceptance.sourceRefs,
      acceptanceRefs: [acceptance.id],
    });
  });

  brainstorm.domainModel.businessFlows.forEach((flow, index) => {
    const acceptanceRefs = flow.capabilityRefs.flatMap((capabilityRef) =>
      brainstorm.acceptance.candidates
        .filter((acceptance) => acceptance.capabilityRefs.includes(capabilityRef) && currentAcceptanceRefs.has(acceptance.id))
        .map((acceptance) => acceptance.id)
    );
    addItem({
      kind: "business_flow",
      title: flow.name,
      summary: flow.summary,
      requiredForCurrentPhase: true,
      priority: "must",
      sourceFieldRefs: [`brainstorm.domainModel.businessFlows[${index}].summary`],
      acceptanceRefs,
    });
  });

  const conceptSets = [
    { path: "deliveryConceptGlossary", value: brainstorm.conceptGrounding?.deliveryConceptGlossary },
    { path: "phaseConceptGrounding", value: brainstorm.conceptGrounding?.phaseConceptGrounding },
  ];
  for (const conceptSet of conceptSets) {
    conceptSet.value?.concepts.forEach((concept, index) => {
      const conceptInCurrentPhase =
        concept.phaseRelevance === "current" ||
        concept.scopeRefs.some((scopeRef) => currentScopeRefs.has(scopeRef)) ||
        concept.acceptanceRefs.some((acceptanceRef) => currentAcceptanceRefs.has(acceptanceRef));
      if (!conceptInCurrentPhase) {
        return;
      }
      addItem({
        kind: inferRequirementDetailKind(concept.explanation),
        title: concept.term,
        summary: concept.explanation,
        requiredForCurrentPhase: concept.phaseRelevance === "current",
        priority: concept.priority === "must_understand" ? "must" : concept.priority === "should_understand" ? "should" : "could",
        sourceFieldRefs: [`brainstorm.conceptGrounding.${conceptSet.path}.concepts[${index}].explanation`],
        scopeRefs: concept.scopeRefs.filter((scopeRef) => currentScopeRefs.has(scopeRef)),
        acceptanceRefs: concept.acceptanceRefs.filter((acceptanceRef) => currentAcceptanceRefs.has(acceptanceRef)),
        conceptRefs: [concept.conceptId],
      });
    });
  }

  const frontend = brainstorm.frontendExperience;
  frontend?.dataViews?.forEach((view, index) => {
    addItem({
      kind: "frontend_operation_path",
      title: view.name,
      summary: `${view.purpose} Selection: ${view.selectionMode}. Pagination required: ${view.paginationRequired}.`,
      requiredForCurrentPhase: frontend.required,
      priority: frontend.required ? "must" : "could",
      sourceFieldRefs: [`brainstorm.frontendExperience.dataViews[${index}]`],
      sourceRefs: view.sourceRefs,
      frontendRefs: [view.viewId],
    });
  });
  frontend?.actions?.forEach((action, index) => {
    addItem({
      kind: "frontend_operation_path",
      title: action.label,
      summary: `${action.label}. Entry: ${action.entryPoint}. Refresh: ${action.refreshPolicy}. Success feedback: ${action.successFeedback.join("; ")}. Blocking or error feedback: ${action.blockingOrErrorFeedback.join("; ")}.`,
      requiredForCurrentPhase: frontend.required,
      priority: frontend.required ? "must" : "could",
      sourceFieldRefs: [`brainstorm.frontendExperience.actions[${index}]`],
      sourceRefs: action.sourceRefs,
      frontendRefs: [action.actionId],
    });
  });
  frontend?.operationPaths?.forEach((operationPath, index) => {
    addItem({
      kind: "frontend_operation_path",
      title: operationPath.name,
      summary: operationPath.selectionSummary,
      requiredForCurrentPhase: frontend.required,
      priority: frontend.required ? "must" : "could",
      sourceFieldRefs: [`brainstorm.frontendExperience.operationPaths[${index}]`],
      sourceRefs: operationPath.sourceRefs,
      frontendRefs: [operationPath.pathId, ...operationPath.dataViewRefs, ...operationPath.actionRefs],
    });
  });

  brainstorm.scope.assumptions.forEach((assumption, index) => {
    addItem({
      kind: "assumption",
      title: assumption.id,
      summary: assumption.text,
      requiredForCurrentPhase: assumption.requiresConfirmation === false,
      priority: assumption.requiresConfirmation ? "should" : "could",
      sourceFieldRefs: [`brainstorm.scope.assumptions[${index}].text`],
      unresolvedNote: assumption.requiresConfirmation ? "Assumption still requires confirmation." : null,
    });
  });

  return {
    schemaVersion: "1.0",
    authority: "brainstorm_contract",
    sourceBrainstormContractRef: brainstormContractRef,
    items,
    extractionWarnings,
  };
}

function stableRequirementDetailId(
  kind: RequirementDetailsIndex["items"][number]["kind"],
  sourceFieldRefs: string[],
  summary: string,
): string {
  return `detail-${kind}-${createHash("sha1")
    .update(`${sourceFieldRefs.join("|")}:${summary}`)
    .digest("hex")
    .slice(0, 10)}`;
}

function stableRequirementDetailWarningId(seed: string, suffix: string): string {
  return `detail-warning-${createHash("sha1").update(`${seed}:${suffix}`).digest("hex").slice(0, 10)}`;
}

function inferRequirementDetailKind(
  value: string,
  fallback: RequirementDetailsIndex["items"][number]["kind"] = "business_scenario",
): RequirementDetailsIndex["items"][number]["kind"] {
  const text = value.toLowerCase();
  if (matchesAny(text, ["frontend", "ui", "page", "screen", "list", "query", "select", "refresh", "页面", "列表", "查询", "选择", "刷新", "反馈"])) {
    return "frontend_operation_path";
  }
  if (matchesAny(text, ["block", "blocking", "deny", "reject", "error", "invalid", "阻断", "拒绝", "失败", "无效", "原因"])) {
    return "blocking_rule";
  }
  if (matchesAny(text, ["validate", "validation", "required", "format", "校验", "必填", "格式", "规则"])) {
    return "validation_rule";
  }
  if (matchesAny(text, ["state", "status", "transition", "状态", "变更", "流转"])) {
    return "state_transition";
  }
  if (matchesAny(text, ["field", "input", "display", "relationship", "字段", "录入", "展示", "关系"])) {
    return "object_field_set";
  }
  if (matchesAny(text, ["create", "update", "approve", "cancel", "close", "submit", "operation", "action", "创建", "修改", "审批", "提交", "销户", "操作", "办理"])) {
    return "object_operation";
  }
  if (matchesAny(text, ["flow", "workflow", "process", "流程", "步骤"])) {
    return "business_flow";
  }
  return fallback;
}

function inferRequirementDetailImpactTags(
  value: string,
  kind: RequirementDetailsIndex["items"][number]["kind"],
): RequirementDetailsIndex["items"][number]["impactTags"] {
  const text = value.toLowerCase();
  const tags = new Set<RequirementDetailsIndex["items"][number]["impactTags"][number]>();
  if (kind === "scope_boundary" || kind === "deferred_or_excluded_boundary" || matchesAny(text, ["scope", "phase", "范围", "阶段", "边界"])) tags.add("scope");
  if (kind === "object_field_set" || matchesAny(text, ["field", "data", "entity", "model", "字段", "数据", "模型", "关系"])) tags.add("data_model");
  if (kind === "business_flow" || kind === "object_operation" || kind === "state_transition" || matchesAny(text, ["flow", "workflow", "process", "operation", "流程", "操作", "状态"])) tags.add("business_flow");
  if (kind === "frontend_operation_path" || matchesAny(text, ["frontend", "ui", "page", "list", "query", "页面", "列表", "查询", "反馈"])) tags.add("frontend");
  if (matchesAny(text, ["api", "interface", "request", "response", "http", "接口", "请求", "响应"])) tags.add("interface");
  if (kind === "acceptance_outcome" || matchesAny(text, ["acceptance", "verify", "success", "验收", "验证", "成功"])) tags.add("acceptance");
  if (matchesAny(text, ["runtime", "build", "start", "deploy", "运行", "构建", "启动", "部署"])) tags.add("runtime");
  return tags.size > 0 ? [...tags] : ["scope"];
}

function inferRequirementDetailLifecycleStage(value: string): RequirementDetailsIndex["items"][number]["lifecycleStage"] {
  const text = value.toLowerCase();
  if (matchesAny(text, ["create", "open", "apply", "submit", "创建", "开户", "申请", "提交", "新增"])) return "create";
  if (matchesAny(text, ["query", "search", "select", "list", "lookup", "查询", "搜索", "选择", "列表"])) return "query_select";
  if (matchesAny(text, ["view", "detail", "display", "查看", "详情", "展示"])) return "view";
  if (matchesAny(text, ["update", "edit", "change", "修改", "更新", "变更"])) return "update";
  if (matchesAny(text, ["approve", "review", "process", "审批", "审核", "处理"])) return "approve_or_process";
  if (matchesAny(text, ["state", "status", "transition", "状态", "流转"])) return "state_change";
  if (matchesAny(text, ["terminate", "cancel", "close", "delete", "撤销", "取消", "销户", "删除", "终止"])) return "terminate_or_cancel";
  if (matchesAny(text, ["block", "reject", "invalid", "error", "阻断", "拒绝", "失败", "异常"])) return "blocking_or_exception";
  return "not_applicable";
}

function inferRequirementDetailQuality(value: string): RequirementDetailsIndex["items"][number]["quality"] {
  const text = value.trim();
  if (text.length < 36) {
    return "thin";
  }
  const detailMarkers = [
    "field", "input", "precondition", "validation", "blocking", "reason", "state", "feedback", "refresh",
    "字段", "录入", "前置", "校验", "阻断", "原因", "状态", "反馈", "刷新",
  ].filter((marker) => text.toLowerCase().includes(marker)).length;
  if (text.length >= 160 || detailMarkers >= 3) {
    return "rich";
  }
  return "usable";
}

function inferRequirementDetailUnresolvedNote(value: string): string | null {
  const text = value.toLowerCase();
  return matchesAny(text, ["unclear", "unknown", "tbd", "to be confirmed", "未确认", "不明确", "待确认"])
    ? "Detail source contains unresolved wording."
    : null;
}

function matchesAny(value: string, needles: string[]): boolean {
  return needles.some((needle) => value.includes(needle));
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values.filter((value) => value.trim().length > 0))];
}

function truncateDetailTitle(value: string): string {
  return value.length > 96 ? `${value.slice(0, 93)}...` : value;
}

function summarizeBaseline(baseline: TechnicalBaseline): Record<string, unknown> {
  return baseline.stack;
}

function technicalBaselineSchemaShape(
  projectKind: "greenfield" | "existing_project" | "unknown",
  locator: DeliveryPhaseLocator,
  options: { hasPreviousTechnicalBaseline?: boolean } = {},
): Record<string, unknown> {
  const previousBaselineStatusRule = "confirmed only when the previous stable stack is unchanged; needs_user_confirmation when adding/replacing any technical baseline element or when repo signals conflict";
  const greenfieldStatusRule = "confirmed only after explicit user technical-baseline confirmation; do not submit before the final technology baseline is confirmed";
  return {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-001",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: projectKind === "greenfield"
      ? greenfieldStatusRule
      : options.hasPreviousTechnicalBaseline ? previousBaselineStatusRule : "confirmed",
    source: projectKind === "existing_project" ? "agent_inferred_from_repo_signals" : "agent_recommended_for_greenfield | user_specified",
    projectKind,
    scope: projectKind === "existing_project" ? "project" : "roadmap",
    stack: projectKind === "greenfield" ? {
      tracks: {
        web: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed Web client technology or No Web client", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
        app: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed App client technology or No App client", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
        backend: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed backend/service technology or No independent backend", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
        persistence: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed database/persistence technology or No persistence yet", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
        dataAccess: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed ORM/data access technology or No ORM", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
        externalServices: { status: "selected | not_needed | not_applicable | user_custom", selection: "confirmed external services or None", source: "agent_recommended_user_confirmed | user_adjusted | user_specified | not_applicable", rationale: "Why this track fits the confirmed requirement." },
      },
      derivedLater: ["testing", "build", "local run", "deployment preparation"],
    } : {
      runtime: "node",
      language: "typescript",
      packageManager: "npm",
      test: "project-appropriate test command",
    },
    constraints: [],
    evidence: [{ reason: "Why this baseline fits the request or repository." }],
    approval: {
      type: projectKind === "greenfield"
        ? "user_confirmed"
        : options.hasPreviousTechnicalBaseline ? "policy_auto_accept only when unchanged; none when needs_user_confirmation" : "user_confirmed",
      confirmedAt: "ISO timestamp required when type=user_confirmed",
      reason: projectKind === "greenfield"
        ? "Summarize the user's final technical-baseline confirmation, including whether they accepted the recommendation, adjusted tracks, or specified a custom stack."
        : options.hasPreviousTechnicalBaseline
        ? "Explain whether the previous baseline is unchanged or why the added/replaced/conflicting technical baseline needs user confirmation."
        : "Baseline follows confirmed request and repository signals.",
    },
    confidence: "medium",
    requiresUserConfirmation: projectKind === "greenfield"
      ? false
      : options.hasPreviousTechnicalBaseline
      ? "false only when unchanged; true when status=needs_user_confirmation"
      : projectKind === "existing_project",
    reasoningSummary: projectKind === "greenfield"
      ? ["Recommendation and user-confirmation rationale.", "Mention any user-adjusted or user-custom technology tracks."]
      : ["Short rationale."],
    alternatives: [{
      name: "Alternative baseline option name.",
      tradeoff: "Why this alternative was not chosen, including the engineering tradeoff.",
    }],
    createdAt: new Date(0).toISOString(),
    updatedAt: new Date(0).toISOString(),
  };
}

function buildArchitectureRepairRequest(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  candidate: ArchitectureArtifactContract | null,
  pgc: PlanningGenerationContract,
  issues: ArchitectureAcceptResult["issues"],
  requestId?: string,
): Record<string, unknown> | null {
  if (issues.length === 0) {
    return null;
  }
  const sectionOutputs = architectureSectionOutputs(projectRoot, locator, requestId ?? createId("arch-repair"));
  const targetSections = inferArchitectureRepairSections(issues);
  const targetSection = targetSections[0] ?? "coverage";
  const targetOutput = sectionOutputs.find((output) => output.section === targetSection) ?? sectionOutputs[0];
  const requirementDetailTransfer = requirementDetailTransferProjection(pgc);
  const planningContractPathRef = toProjectRelative(projectRoot, planningContractPath(projectRoot, pgc.planningContractId, locator));
  return {
    operation: "repair_architecture_section_candidate",
    protocolRef: "Step 5A ArchitectureSectionRepairRequest",
    inputs: {
      planningContractPath: planningContractPathRef,
      technicalBaselinePath: toProjectRelative(projectRoot, technicalBaselinePath(projectRoot, locator.deliveryId)),
    },
    contextProjection: {
      phaseId: locator.phaseId,
      planningContractId: pgc.planningContractId,
      requirementDetailTransfer,
    },
    fieldAccessHints: {
      requirementDetailTransfer: "Use .contextProjection.requirementDetailTransfer plus inputs.planningContractPath to preserve Brainstorm-confirmed current phase details while repairing AAC sections.",
      planningContractDetailSelectors: [
        ".phaseScope.included[].items",
        ".phaseScope.deferred[].items",
        ".phaseScope.excluded[].items",
        ".phaseScope.acceptanceCandidates[] | {id,statement,priority,sourceRefs,capabilityRefs}",
        ".planningInputs.businessFlows[].summary",
        ".contextRefs.phaseConceptGroundingRef",
        ".contextRefs.deliveryConceptGlossaryRef",
        ".contextRefs.confirmedFrontendExperienceRef",
        ".contextRefs.currentFrontendExperienceRef",
      ],
    },
    repairGoal: "Repair the affected Architecture section candidates so the assembled AAC passes loom structural validation while preserving current phase requirement details from requirementDetailTransfer.",
    mustNot: [
      "Do not change current phase scope.",
      "Do not change TechnicalBaseline.",
      "Do not include deferred or excluded scope.",
      "Do not write product code.",
      "Do not invent new user decisions.",
      "Do not return a whole ArchitectureArtifactContract replacement.",
    ],
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    sectionRewritePolicy: {
      dependencyOrder: ["foundation", "domain_contract", "behavior", "frontend_experience", "runtime_delivery", "coverage"],
      rewriteAffectedSectionAndDownstream: false,
      rule: "Rewrite only targetSection unless an issue explicitly points to an upstream section.",
    },
    targetSection,
    targetCandidateFile: targetOutput?.candidateFile ?? null,
    targetSchemaRef: targetOutput?.schemaRef ?? null,
    targetGenerationRules: targetOutput?.generationRules ?? [],
    issues: issues.map(compactContractIssue),
    issueSpecificRepairRules: architectureRepairRulesForIssues(issues),
    repairPolicy: {
      maxAutoRepairAttempts: 2,
      onExhausted: "report_to_user",
    },
    candidateId: candidate?.architectureArtifactContractId ?? null,
  };
}

function buildArchitectureRepairInstruction(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  candidate: ArchitectureArtifactContract | null,
  pgc: PlanningGenerationContract,
  issues: ArchitectureAcceptResult["issues"],
  requestId?: string,
): Record<string, unknown> | null {
  const repairRequest = buildArchitectureRepairRequest(projectRoot, locator, candidate, pgc, issues, requestId);
  if (!repairRequest) return null;
  return {
    mode: "repair_candidate",
    schema: "ArchitectureSections",
    ...artifactRepairPolicy(),
    ...repairRequest,
    repairSubmitRouting: repairSubmitRouting({
      kind: "candidate",
      submitCommandName: "architecture accept",
    }),
    instructions: [
      compactContextReadStep,
      "Repair only Architecture section candidate files.",
      "Do not modify project source code.",
      "Do not change Brainstorm scope, TechnicalBaseline, or PGC.",
      "Use contextProjection.requirementDetailTransfer and targetGenerationRules; preserve phaseScope items, acceptance sourceRefs/capabilityRefs, business flow summaries, concept refs, and frontend refs while repairing.",
      "Rewrite affected section files and any downstream section files required by the section dependency order.",
      "Run architecture accept again with the same request-id.",
      "After architecture accept succeeds, follow data.instruction immediately.",
    ],
    submitCommand: {
      name: "architecture accept",
      argv: [
        "architecture",
        "accept",
        "--delivery-id",
        locator.deliveryId,
        "--phase-id",
        locator.phaseId,
        ...(requestId ? ["--request-id", requestId] : []),
      ],
    },
  };
}

function architectureRepairRulesForIssues(issues: ArchitectureAcceptResult["issues"]): string[] {
  const rules: string[] = [];
  if (issues.some((issue) => issue.path === "/runtimeDelivery/basis/previousRuntimeDeliveryRef")) {
    rules.push(
      "For /runtimeDelivery/basis/previousRuntimeDeliveryRef, read the original ArchitectureSectionsGenerationRequest.sourceRefs.previousRuntimeDeliveryRef first.",
      "If sourceRefs.previousRuntimeDeliveryRef is present and runtimeDelivery.status is unchanged, set runtimeDelivery.basis.previousRuntimeDeliveryRef exactly to that value.",
      "If sourceRefs.previousRuntimeDeliveryRef is absent, do not wait for another source contract and do not keep runtimeDelivery.status=unchanged; rewrite runtime_delivery as modified or not_applicable according to the current phase facts and the request schema.",
    );
  }
  return rules;
}

function compactContractIssue(issue: ArchitectureAcceptResult["issues"][number]): Record<string, unknown> {
  return {
    code: issue.code,
    path: issue.path,
    message: issue.message,
    repairHint: architectureRepairHintForIssue(issue),
    ...(issue.schemaError ? { schemaError: issue.schemaError } : {}),
  };
}

function architectureRepairHintForIssue(issue: ArchitectureAcceptResult["issues"][number]): string | undefined {
  if (issue.path === "/runtimeDelivery/basis/previousRuntimeDeliveryRef") {
    return "Read request.sourceRefs.previousRuntimeDeliveryRef. If present, use it exactly for unchanged runtimeDelivery; if absent, do not keep status=unchanged and rewrite runtime_delivery as modified or not_applicable according to current phase facts.";
  }
  return issue.repairHint;
}

export function inferArchitectureRepairSections(issues: ArchitectureAcceptResult["issues"]): Array<"foundation" | "domain_contract" | "behavior" | "frontend_experience" | "runtime_delivery" | "coverage"> {
  const sections = new Set<"foundation" | "domain_contract" | "behavior" | "frontend_experience" | "runtime_delivery" | "coverage">();
  for (const issue of issues) {
    const pointer = issue.path;
    if (pointer.includes("/sections/foundation")) {
      sections.add("foundation");
    } else if (pointer.includes("/sections/domain_contract")) {
      sections.add("domain_contract");
    } else if (pointer.includes("/sections/behavior")) {
      sections.add("behavior");
    } else if (pointer.includes("/sections/frontend_experience")) {
      sections.add("frontend_experience");
    } else if (pointer.includes("/sections/runtime_delivery")) {
      sections.add("runtime_delivery");
    } else if (pointer.includes("/sections/coverage")) {
      sections.add("coverage");
    } else if (pointer.includes("/runtimeDelivery")) {
      sections.add("runtime_delivery");
    } else if (pointer.includes("/frontendExperience")) {
      sections.add("frontend_experience");
    } else if (pointer.includes("/acceptanceMatrix") || pointer.includes("/risksAndDecisions") || issue.code === "AAC_COVERAGE_TYPE_MISMATCH") {
      sections.add("coverage");
    } else if (pointer.includes("/userFlows") || pointer.includes("/stateMachines")) {
      sections.add("behavior");
    } else if (pointer.includes("/dataModel") || pointer.includes("/interfaces")) {
      sections.add("domain_contract");
    } else if (pointer.includes("/engineeringBoundary") || pointer.includes("/modules")) {
      sections.add("foundation");
    }
  }
  return [...sections];
}

function requireCandidateFile(candidateFile: string | undefined): string {
  if (!candidateFile?.trim()) {
    throw invalidArgument("architecture accept requires --request-id or --candidate-file.");
  }
  return candidateFile;
}

function allowedArchitectureRefsFromPgc(pgc: PlanningGenerationContract): Record<string, unknown> {
  return {
    scopeRefs: pgc.phaseScope.included.map((item) => item.scopeId),
    deferredScopeRefs: pgc.phaseScope.deferred.map((item) => item.scopeId),
    excludedScopeRefs: pgc.phaseScope.excluded.map((item) => item.scopeId),
    acceptanceRefs: pgc.phaseScope.acceptanceCandidates.map((item) => item.id),
  };
}

async function previousRuntimeDeliveryRef(projectRoot: string, locator: DeliveryPhaseLocator): Promise<string | null> {
  const index = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const activeIndex = index.phases.findIndex((phase) => phase.phaseId === locator.phaseId);
  const previousPhases = activeIndex >= 0 ? index.phases.slice(0, activeIndex) : index.phases;
  for (const phase of [...previousPhases].reverse()) {
    const architectureArtifact = phase.latestRefs?.architectureArtifact;
    if (phase.status === "completed" && typeof architectureArtifact === "string" && architectureArtifact.trim()) {
      return `${architectureArtifact}#/runtimeDelivery`;
    }
  }
  return null;
}

function architectureAllowedRefsUsage(): Record<string, unknown> {
  return {
    authority: "request.allowedRefs is the only allowed scope/acceptance id domain for AAC section refs.",
    exactOnly: true,
    fieldMappings: [
      {
        fields: ["scopeRefs[]"],
        allowedValuesFrom: "allowedRefs.scopeRefs",
        noMatchBehavior: "write [] or block; never invent a scope id",
      },
      {
        fields: ["acceptanceRefs[]", "acceptanceMatrix[].acceptanceId"],
        allowedValuesFrom: "allowedRefs.acceptanceRefs",
        noMatchBehavior: "write [] for optional acceptanceRefs or block coverage when a must acceptance cannot be represented; never invent AC ids",
      },
      {
        fields: ["deferredNotes[].deferredRef"],
        allowedValuesFrom: "allowedRefs.deferredScopeRefs",
        noMatchBehavior: "omit deferredNotes item when there is no allowed deferred scope ref",
      },
      {
        fields: ["excludedRef"],
        allowedValuesFrom: "allowedRefs.excludedScopeRefs",
        noMatchBehavior: "omit the optional excluded ref when there is no allowed excluded scope ref",
      },
    ],
    validatorBoundary: "CLI validates ids mechanically against allowedRefs. Agent owns semantic selection from the allowed sets.",
  };
}

function architectureAllowedRefGenerationRules(): string[] {
  return [
    "Read request.allowedRefs and outputContract.allowedRefsUsage before writing this section.",
    "Use only exact values from allowedRefs.scopeRefs for every scopeRefs[] value; never create new scope ids.",
    "Use only exact values from allowedRefs.acceptanceRefs for every acceptanceRefs[] value and coverage acceptanceId; never create new AC ids.",
    "Use only exact values from allowedRefs.deferredScopeRefs for deferredRef values and allowedRefs.excludedScopeRefs for excludedRef values.",
    "If an intended scope or acceptance meaning has no allowed ref, omit that optional ref or return blocked; do not approximate by inventing ids.",
  ];
}

function architectureSectionGenerationRules(rules: string[] = []): string[] {
  return [
    ...architectureAllowedRefGenerationRules(),
    "Use request.contextProjection.requirementDetailTransfer as the current phase requirement-detail transfer contract, and sourceRefs.planningContractRef as the full authority when more detail is needed.",
    "Do not collapse PGC phaseScope items, acceptance statement/sourceRefs/capabilityRefs, business flow summaries, concept refs, object field sets, operation rules, state changes, blocking reasons, or frontend refs into generic module labels.",
    ...rules,
  ];
}

function requirementDetailTransferProjection(pgc: PlanningGenerationContract): Record<string, unknown> {
  return {
    authority: "planning_generation_contract",
    purpose: "Mechanically carry Brainstorm-confirmed current phase requirement details into AAC generation without adding a parallel requirement model.",
    requirementDetails: compactRequirementDetailsIndex(pgc),
    requirementDetailsUsageRule: "When requirementDetails exists, treat requirementDetails.items as the canonical detail index after Brainstorm. Generate AAC artifacts that carry those details, then write coverage.detailCoverage using detailId plus artifact refs only; do not duplicate full detail summaries in coverage.",
    currentPhaseScope: {
      included: pgc.phaseScope.included.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
      deferred: pgc.phaseScope.deferred.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
      excluded: pgc.phaseScope.excluded.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
    },
    acceptanceDetails: pgc.phaseScope.acceptanceCandidates.map((item) => ({
      id: item.id,
      statement: item.statement,
      priority: item.priority,
      capabilityRefs: item.capabilityRefs ?? [],
      sourceRefs: item.sourceRefs ?? [],
    })),
    businessFlowDetails: pgc.planningInputs.businessFlows,
    userFacingLanguage: pgc.planningInputs.userFacingLanguage ?? null,
    userFacingLanguageUsageRule: "Apply only to user-visible product copy such as navigation labels, page text, form labels, action labels, result/status text, success messages, validation errors, and business-blocking feedback. Do not translate code identifiers, API paths, database fields, enum values, package names, framework terms, or internal artifact ids.",
    objectOperationDetailRules: {
      sourceFields: [
        "currentPhaseScope.included[].items",
        "acceptanceDetails[].statement",
        "businessFlowDetails[].summary",
        "conceptRefs.phaseConceptGroundingRef",
        "frontendExperienceDetails when UI applies",
      ],
      scopeCoverageRule: "Every currentPhaseScope.included item should remain traceable through AAC coverage: represented by domain_contract, behavior, frontend_experience, coverage, or an explicit unresolved/deferred reason. Do not preserve only the items that look easiest to implement.",
      preservationRule: "Treat Brainstorm-confirmed business objects, key field sets, object operations, operation inputs, preconditions, validation/blocking reasons, success state changes, and visible feedback as current-phase requirement details that AAC sections must preserve.",
      sectionMapping: {
        domain_contract: "Represent business objects as entities or reference projections; represent key field sets as entity fields, interface schemas, constraints, relationships, and data rules.",
        behavior: "Represent object operations as userFlows/stateMachines with preconditions, guards, blocking reasons, effects, success state changes, and feedback.",
        frontend_experience: "Represent object operation paths as data views, actions, operation paths, visible states, and refresh/readback expectations when UI applies.",
        coverage: "Map acceptance statements to the AAC artifacts that carry the object, field, operation, state, blocking, and feedback details.",
      },
      insufficientDetailRule: "If Brainstorm/PGC confirms a required scope item or object-operation detail but no AAC artifact can represent it, write blocked output for the affected section rather than silently omitting the detail.",
    },
    frontendExperienceDetails: {
      frontendExperience: pgc.planningInputs.frontendExperience ?? null,
      operationPathSelectors: [
        "planningContractRef.planningInputs.frontendExperience.dataViews",
        "planningContractRef.planningInputs.frontendExperience.actions",
        "planningContractRef.planningInputs.frontendExperience.operationPaths",
      ],
      usageRule: "Use these Brainstorm-confirmed frontend operation path details when generating AAC frontend_experience. If null, read frontendExperienceSource refs when present.",
    },
    conceptRefs: {
      deliveryConceptGlossaryRef: pgc.contextRefs?.deliveryConceptGlossaryRef ?? null,
      phaseConceptGroundingRef: pgc.contextRefs?.phaseConceptGroundingRef ?? null,
    },
    frontendRefs: {
      confirmedFrontendExperienceRef: pgc.contextRefs?.confirmedFrontendExperienceRef ?? null,
      currentFrontendExperienceRef: pgc.contextRefs?.currentFrontendExperienceRef ?? null,
    },
    consumeInSections: {
      domain_contract: "Represent fields, entities, relationships, constraints, request/response/error schemas, and interface rules from PGC scope items, acceptance details, business flow summaries, and objectOperationDetailRules.",
      behavior: "Represent object operations, flow steps, preconditions, validation/blocking rules, blocking reasons, outcomes, state changes, guards, effects, and visible feedback from PGC business flow and acceptance details.",
      frontend_experience: "Represent required input, display, feedback, navigation, target discovery/selection, action entry, refresh policy, and operation path expectations from PGC frontendExperienceDetails and frontend refs.",
      coverage: "Map each acceptance detail and confirmed scope item to current AAC artifacts and verification hints without dropping rule, field, state, unresolved-note, or source-ref context.",
      detailCoverage: "Map each requirementDetails.items[].detailId to the AAC artifacts that carry it, using coverage.content.detailCoverage.",
    },
  };
}

function compactRequirementDetailsIndex(pgc: PlanningGenerationContract): Record<string, unknown> | null {
  if (!pgc.requirementDetails) {
    return null;
  }
  return {
    schemaVersion: pgc.requirementDetails.schemaVersion,
    authority: pgc.requirementDetails.authority,
    sourceBrainstormContractRef: pgc.requirementDetails.sourceBrainstormContractRef,
    items: pgc.requirementDetails.items.map((item) => ({
      detailId: item.detailId,
      kind: item.kind,
      title: item.title,
      summary: item.summary,
      requiredForCurrentPhase: item.requiredForCurrentPhase,
      priority: item.priority,
      sourceRefs: item.sourceRefs,
      scopeRefs: item.scopeRefs,
      acceptanceRefs: item.acceptanceRefs,
      conceptRefs: item.conceptRefs,
      frontendRefs: item.frontendRefs,
      impactTags: item.impactTags,
      lifecycleStage: item.lifecycleStage,
      quality: item.quality,
      unresolvedNote: item.unresolvedNote,
    })),
    extractionWarningCount: pgc.requirementDetails.extractionWarnings.length,
    fullDetailSource: "sourceRefs.planningContractRef#/requirementDetails",
  };
}

function architectureEnumRefs(): Record<string, string[]> {
  return {
    section: ["foundation", "domain_contract", "behavior", "frontend_experience", "runtime_delivery", "coverage"],
    candidateStatus: ["ready", "blocked"],
    projectKind: ["greenfield", "existing_project", "unknown"],
    engineeringBoundaryStrategy: [
      "create_minimal_phase_structure",
      "follow_existing_structure",
      "extend_existing_modules",
      "unknown",
    ],
    entityType: ["internal", "external", "derived", "value_object"],
    implementationIntent: ["full", "reference_only", "read_only_projection", "external_dependency"],
    interfaceType: ["http_api", "service_method", "component", "cli_command", "event", "job", "external_adapter"],
    interfaceRole: ["read_model", "command", "readback", "component_binding", "external_contract", "unknown"],
    httpMethod: ["GET", "POST", "PUT", "PATCH", "DELETE"],
    userFlowKind: ["user_interaction", "service_flow", "cli_flow", "scheduled_flow", "system_flow"],
    userFlowEntryType: ["page", "api", "command", "event", "job", "manual", "unknown"],
    userFlowOutcomeType: ["success", "error", "partial", "async_pending"],
    acceptancePriority: ["must", "should", "could"],
    acceptanceCoverageStatus: ["covered", "partial", "not_applicable", "deferred", "uncovered"],
    acceptanceReasonCategory: [
      "candidate_incomplete",
      "reference_error",
      "scope_conflict",
      "baseline_conflict",
      "user_tradeoff",
      "upstream_invalid",
    ],
    acceptanceCoverageType: [
      "module",
      "data_entity",
      "data_constraint",
      "relationship",
      "interface",
      "user_flow",
      "state_machine",
      "state_rule",
      "decision",
      "risk",
    ],
    verificationHintKind: ["unit", "integration", "e2e", "manual", "static", "contract"],
    decisionType: ["architecture", "scope", "baseline", "delivery", "validation", "implementation"],
    decisionStatus: ["proposed", "accepted", "needs_user_decision", "superseded"],
    decisionCategory: [
      "scope_change",
      "baseline_change",
      "architecture_tradeoff",
      "acceptance_conflict",
      "defer_or_include",
    ],
    riskType: ["architecture", "implementation", "data", "integration", "delivery", "validation"],
    riskSeverity: ["low", "medium", "high", "blocking"],
    riskStatus: ["open", "mitigated", "accepted", "closed"],
    runtimeDeliveryStatus: [...runtimeDeliveryStatusSchema.options],
    runtimeDeliveryCodegenRequired: [...runtimeDeliveryCodegenRequiredSchema.options],
    runtimeDeliveryVerificationBoundary: [runtimeDeliveryVerificationBoundarySchema.value],
    runtimeDeliveryExpectedStatus: ["2xx_or_3xx"],
    runtimeSurfaceProbeType: ["http_path", "command", "import_check", "none"],
    handoffNextNode: ["task_plan", "architecture_artifact_repair", "needs_user_decision", "blocked"],
  };
}

function architectureSectionSchemaShape(
  locator: DeliveryPhaseLocator,
  requestId: string,
  section: "foundation" | "domain_contract" | "behavior" | "frontend_experience" | "runtime_delivery" | "coverage",
  content: Record<string, unknown>,
): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    requestId,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    section,
    status: "ready | blocked (section wrapper status; use ready when this section was generated successfully. Do not copy nested content status values such as runtimeDelivery.status=unchanged into this field.)",
    content,
    blockedReasons: {
      requiredWhen: "status=blocked",
      readyValue: [],
      itemShape: {
        code: "PGC_INSUFFICIENT",
        message: "Use only when status=blocked.",
        nextNode: {
          enum: ["planning_contract_create", "technical_baseline_request", "needs_user_decision", "blocked"],
          rule: "Choose exactly one enum value. Do not copy a pipe-joined list.",
        },
      },
    },
    createdAt: new Date(0).toISOString(),
  };
}

function foundationSectionContentShape(locator: DeliveryPhaseLocator): Record<string, unknown> {
  return {
    source: {
      planningGenerationContractId: "pgc-id-from-sourceRefs",
      technicalBaselineId: "technical-baseline-id-from-sourceRefs",
      brainstormContractId: "optional-brainstorm-contract-id",
      roadmapId: "roadmap-id-or-null",
      phaseId: locator.phaseId,
    },
    engineeringBoundary: {
      projectKind: "greenfield | existing_project | unknown",
      strategy: "create_minimal_phase_structure | follow_existing_structure | extend_existing_modules | unknown",
      applications: [{
        appId: "app-core",
        type: "node_service | web_app | cli | library | existing_app_type",
        root: ".",
      }],
      modules: [{
        moduleId: "module-id",
        appId: "app-core",
        paths: ["project-relative/path"],
        responsibility: "Current-phase responsibility owned by this engineering module.",
        layerMappings: [{
          layer: "domain | service | api | ui | persistence | verification",
          paths: ["project-relative/path/file-or-folder"],
          artifactIntent: "full | reference_only | read_only_projection | external_dependency",
        }],
      }],
      creationPolicy: {
        createOnlyCurrentPhasePaths: true,
        avoidFuturePhaseScaffolding: true,
      },
    },
    modules: [{
      moduleId: "module-id",
      name: "Module name",
      responsibility: "Current-phase module responsibility tied to concrete PGC scope items, acceptance details, business flows, frontend/runtime surfaces, or implementation boundaries.",
      dependsOn: ["other-module-id"],
      scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
      acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
    }],
  };
}

function foundationSectionGenerationRules(): string[] {
  return [
    "Use request.contextProjection.requirementDetailTransfer.currentPhaseScope.included[].items, acceptanceDetails, businessFlowDetails, conceptRefs, and frontendRefs to define module responsibilities and engineering boundaries.",
    "Every current-phase module responsibility should name the concrete business/technical object, action, workflow, field group, interface surface, frontend surface, runtime surface, or verification boundary it owns when such detail exists in PGC.",
    "Do not use generic module responsibilities such as core module, feature module, or UI module unless the same module also lists the concrete current-phase responsibility.",
    "Use engineeringBoundary.modules[].layerMappings to preserve where concrete domain, service, api, ui, persistence, runtime, or verification responsibilities should land for later TaskPlan artifactRefs.",
    "If a required current-phase detail cannot be assigned to any module boundary, write status=blocked with a concrete blockedReason instead of hiding the gap.",
  ];
}

function domainContractSectionContentShape(): Record<string, unknown> {
  return {
    dataModel: {
      entities: [{
        entityId: "entity-id",
        name: "Entity name",
        type: "internal | external | derived | value_object",
        implementationIntent: "full | reference_only | read_only_projection | external_dependency",
        moduleRefs: ["module-id-from-foundation"],
        scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
        fields: [fieldShapeExample("field-id", "fieldName", "string")],
        constraints: [{
          constraintId: "constraint-id",
          type: "business_rule | uniqueness | range | format | invariant",
          description: "Constraint description.",
        }],
      }],
      relationships: [{
        relationshipId: "relationship-id",
        type: "owns | references | derives_from | depends_on | associates_with",
        fromEntityRef: "entity-id",
        toEntityRef: "other-entity-id",
        moduleRefs: ["module-id-from-foundation"],
        scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
        description: "Relationship description.",
      }],
      constraints: [{
        constraintId: "global-constraint-id",
        type: "business_rule | invariant | validation",
        description: "Cross-entity constraint description.",
        entityRefs: ["entity-id"],
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      }],
    },
    interfaces: [{
      interfaceId: "interface-id",
      name: "interfaceName",
      type: "http_api | service_method | component | cli_command | event | job | external_adapter",
      role: "read_model | command | readback | component_binding | external_contract | unknown",
      moduleRefs: ["module-id-from-foundation"],
      entityRefs: ["entity-id"],
      scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
      acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      method: "GET | POST | PUT | PATCH | DELETE",
      path: "/optional/path/or-command-name",
      requestSchema: [fieldShapeExample("request-field-id", "requestField", "string")],
      responseSchema: [fieldShapeExample("response-field-id", "responseField", "string")],
      errorSchema: [fieldShapeExample("error-code", "errorCode", "string")],
    }],
  };
}

function domainContractSectionGenerationRules(): string[] {
  return [
    "This phase's AAC must be self-contained for later TaskPlan/TaskExecution refs.",
    "Use request.contextProjection.requirementDetailTransfer.currentPhaseScope.included[].items, acceptanceDetails, businessFlowDetails, and objectOperationDetailRules as the current phase domain-detail source.",
    "When PGC details name required fields, validation rules, blocking reasons, status values, source-grounded business constraints, or API data that frontend/backend tasks must share, represent them in content.dataModel.entities[].fields, entity/global constraints, interfaces requestSchema/responseSchema/errorSchema, and relationships as appropriate.",
    "When Brainstorm/PGC details name key field sets, preserve the fields as identity/input/display/relationship/status/result-feedback responsibilities in entities, constraints, relationships, interfaces, or assumptions. Do not leave them only as prose module notes.",
    "When Brainstorm/PGC details name operations on an object, make the domain artifacts sufficient for behavior and task planning: identify the entities, field schemas, constraints, and interface data needed by each operation.",
    "For user-facing workflows, design interfaces around the workflow/surface responsibilities rather than only module labels: read_model interfaces load/query/select target data, command interfaces submit user actions, and readback interfaces refresh the result or status after the action when needed.",
    "When frontendExperienceDetails or frontendExperience operation paths declare dataViews/actions/operationPaths, map them to interface roles where applicable: dataViews usually need read_model or component_binding, actions usually need command, and post-action refresh/status observation may need readback. If the phase is backend-only or UI is intentionally deferred, state that through interface roles and behavior instead of inventing frontend consumers.",
    "Each interface should expose the request, response, and error fields needed by the workflow step or operation path it supports. Do not collapse a list/query/readback protocol into a single vague service interface when the current phase needs user-visible data selection or refreshed state.",
    "Do not replace detailed PGC rules or field lists with generic phrases; carry the concrete details available in PGC into this section.",
    "If the current phase uses an entity that was created in an earlier phase or already exists in the repository, include it in content.dataModel.entities for this current section with implementationIntent=reference_only or read_only_projection unless this phase owns changing the entity shape.",
    "Do not reference an earlier phase AAC entity id from interfaces, relationships, constraints, behavior, or coverage unless that entity id is explicitly listed in this current domain_contract section.",
    "Use implementationIntent=full only for entities whose model or lifecycle is owned by the current phase scope.",
    "Relationships must use fromEntityRef/toEntityRef values from this current section's content.dataModel.entities[].entityId.",
    "Interfaces must use entityRefs values from this current section's content.dataModel.entities[].entityId.",
  ];
}

function fieldShapeExample(fieldId: string, name: string, type: string): Record<string, unknown> {
  return {
    fieldId,
    name,
    type,
    required: true,
    semanticType: "optional-semantic-type",
    enumValues: [],
  };
}

function behaviorSectionContentShape(): Record<string, unknown> {
  return {
    userFlows: [{
      flowId: "flow-id",
      name: "Flow name",
      kind: "user_interaction | service_flow | cli_flow | scheduled_flow | system_flow",
      moduleRefs: ["module-id-from-foundation"],
      interfaceRefs: ["interface-id-from-domain-contract"],
      entityRefs: ["entity-id-from-domain-contract"],
      scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
      acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      entry: {
        type: "page | api | command | event | job | manual | unknown",
        ref: "interface-or-route-ref-or-null",
        label: "Optional user-facing entry label.",
      },
      steps: [{
        stepId: "step-id",
        actor: "Optional actor",
        action: "What happens in this step.",
        systemResponse: "Optional system response.",
        interfaceRefs: ["interface-id-from-domain-contract"],
        stateMachineRefs: ["state-machine-id; required field, use [] when this step does not touch a state machine"],
      }],
      outcomes: [{
        type: "success | error | partial | async_pending",
        description: "Outcome description.",
        errorCode: "OPTIONAL_ERROR_CODE",
      }],
    }],
    stateMachines: [{
      stateMachineId: "state-machine-id",
      name: "State machine name",
      entityRef: "entity-id-or-null",
      entityRefs: ["entity-id-from-domain-contract"],
      moduleRefs: ["module-id-from-foundation"],
      scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
      acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      states: [{
        stateId: "state-id",
        name: "State name",
        terminal: false,
      }],
      initialState: "state-id",
      events: [{
        eventId: "event-id",
        name: "EventName",
      }],
      transitions: [{
        transitionId: "transition-id",
        from: "state-id",
        to: "next-state-id",
        event: "event-id",
        guards: ["Guard condition."],
        effects: ["Effect description."],
      }],
      rules: [{
        ruleId: "rule-id",
        description: "State rule description.",
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      }],
    }],
  };
}

function behaviorSectionGenerationRules(): string[] {
  return [
    "Use request.contextProjection.requirementDetailTransfer.businessFlowDetails, acceptanceDetails, and objectOperationDetailRules as the current phase behavior-detail source.",
    "For each applicable current phase object operation, represent trigger/action steps, operation inputs, preconditions, validation or blocking rules, blocking reasons, success outcomes, state changes, guards, effects, and user/system feedback in content.userFlows and content.stateMachines.",
    "If a confirmed operation has blocking reasons or state changes, do not model it only as a happy-path userFlow; include guards/effects/rules that TaskPlan can assign and Review can verify.",
    "If PGC says a flow is not applicable because the phase is non-domain technical work, express the technical workflow instead of fabricating business states.",
    "Use only artifact ids declared in the current request's accepted/created section candidates.",
    "Every userFlows[].entityRefs and stateMachines[].entityRefs value must come from the current domain_contract.content.dataModel.entities[].entityId.",
    "Every userFlows[].interfaceRefs value must come from the current domain_contract.content.interfaces[].interfaceId.",
    "If a flow needs an earlier phase entity that is missing from the current domain_contract section, repair domain_contract to add a reference_only or read_only_projection entity before referencing it here.",
  ];
}

function frontendExperienceSectionContentShape(): Record<string, unknown> {
  return {
    frontendExperience: {
      required: true,
      kind: "business_application | admin_console | dashboard | technical_demo | none",
      experienceLevel: "none | technical_demo | usable_internal_product | polished_product",
      sourceRefs: {
        brainstormFrontendExperienceRef: "frontendExperienceSource.confirmedFrontendExperienceRef or currentFrontendExperienceRef",
        repositoryContextRef: "frontendExperienceSource.repositoryContextRef only when present; omit when absent and never write null",
        technicalBaselineRef: "frontendExperienceSource.technicalBaselineRef",
      },
      sourceAuthority: [
        "Brainstorm frontendExperience is the user-confirmed product target.",
        "RepositoryContext and TechnicalBaseline provide implementation facts only.",
        "Do not downgrade a user-confirmed usable_internal_product target to technical_demo without a user decision ref.",
        "Do not invent a current UI implementation target when Brainstorm confirmed frontend required=false or experienceLevel=none.",
        "Use contextProjection.requirementDetailTransfer.userFacingLanguage for user-visible labels, navigation text, form text, success/error/business-blocking feedback, and visible state/result text. Keep technical identifiers conventional.",
      ],
      detection: {
        source: "agent_inferred_and_repo_detected | agent_inferred | repo_detected | not_applicable",
        signals: ["Requirement or repository signal that proves whether frontend is required."],
        confidence: "low | medium | high",
      },
      surfaces: [{
        surfaceId: "surface-id",
        name: "Surface name",
        purpose: "What user workflow this surface supports.",
        userRoleRefs: ["role-or-actor-ref"],
        workflowRefs: ["flow-id-from-behavior"],
        moduleRefs: ["module-id-from-foundation"],
      }],
      dataViews: [{
        viewId: "view-id",
        name: "Result list, detail, form, or dashboard name",
        purpose: "How the view helps the user find, inspect, or operate on the target object.",
        targetObject: "Business object users operate on, when applicable.",
        selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
        paginationRequired: true,
        defaultLoadsFirstPage: true,
        searchCriteria: [{
          criterionId: "criterion-id",
          label: "User-facing query condition grounded in confirmed fields.",
          fieldRef: "optional dataModel field/entity ref",
          reason: "Why this criterion is useful for the current operation path.",
          sourceRefs: ["brainstorm-or-pgc-source-ref"],
        }],
        criteriaUnclearNote: "Use only when confirmed fields are insufficient for advanced filters.",
        sourceRefs: ["brainstorm-or-pgc-source-ref"],
      }],
      actions: [{
        actionId: "action-id",
        label: "User-facing action label",
        targetObject: "Business object acted on, when applicable.",
        entryPoint: "result_row_action | detail_button | form_submit | bulk_action | inline_action | navigation_entry",
        inputFields: ["Confirmed input/display/pass-through field name or ref."],
        resultObservation: ["list_refresh", "detail_refresh", "inline_status_update", "response_message", "not_applicable"],
        refreshPolicy: "refresh_current_query | refresh_detail | update_inline_state | show_message_only | not_applicable",
        successFeedback: ["Visible success message, row/detail refresh, or status change."],
        blockingOrErrorFeedback: ["Visible business blocking reason, validation message, or error state."],
        sourceRefs: ["brainstorm-or-pgc-source-ref"],
      }],
      operationPaths: [{
        pathId: "path-id",
        name: "Operation path name",
        userGoal: "What the user is trying to complete.",
        surfaceRef: "surface-id",
        workflowRef: "flow-id-from-behavior",
        targetObject: "Business object users operate on, when applicable.",
        selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
        selectionSummary: "Natural-language path such as query list -> select record -> trigger action -> observe refreshed result.",
        dataViewRefs: ["view-id"],
        actionRefs: ["action-id"],
        requiredStates: ["idle", "loading", "success", "error", "empty", "business_blocking"],
        sourceRefs: ["brainstorm-or-pgc-source-ref"],
      }],
      navigation: {
        required: true,
        pattern: "tabs | sidebar | top_nav | segmented_control | none",
        items: [{
          label: "Navigation label",
          targetSurfaceRef: "surface-id",
        }],
      },
      interactionStates: ["idle", "loading", "success", "error", "empty", "business_blocking"],
      mustNot: [
        "Do not implement a usable_internal_product as one linear stack of naked forms.",
        "Do not use phase labels as product navigation.",
      ],
      notes: ["Frontend experience rationale for this phase."],
    },
  };
}

function runtimeDeliverySectionContentShape(locator: DeliveryPhaseLocator): Record<string, unknown> {
  return {
    runtimeDelivery: {
      status: "modified | unchanged | not_applicable",
      contractVersion: `${locator.phaseId}-v1`,
      runtimeKind: "technology-specific runtime kind chosen by Agent, e.g. node_express_serves_vite_static, python_fastapi, go_http_server, cli_tool, library",
      basis: {
        technicalBaselineRef: "sourceRefs.technicalBaselineRef; required authority for stack/package manager/framework/runtime model",
        repositoryContextRef: "optional sourceRefs.repositoryContextRef; omit or set null when no repository context exists",
        planningGenerationContractRef: "sourceRefs.planningContractRef; current phase scope authority",
        previousRuntimeDeliveryRef: "sourceRefs.previousRuntimeDeliveryRef when runtimeDelivery.status=unchanged; omit or set null only when no previous runtime delivery ref exists and status is not unchanged",
        reason: "Explain how TechnicalBaseline, RepositoryContext, and current phase scope shaped this contract. Required when status=unchanged or not_applicable.",
      },
      build: {
        command: "technology-appropriate build command chosen from TechnicalBaseline/repository facts, or omit when not_applicable",
        workingDirectory: ".",
        outputs: ["declared deliverable output path"],
        codeLevelExpectations: [
          "Technology-specific script/artifact expectations written by Agent.",
        ],
      },
      start: {
        command: "technology-appropriate start command chosen from TechnicalBaseline/repository facts, or omit when not_applicable",
        workingDirectory: ".",
        entry: "declared runtime entry or null",
        host: "0.0.0.0",
        port: 4173,
        portEnv: "PORT",
        codeLevelExpectations: [
          "Technology-specific entry/start behavior expectations.",
        ],
      },
      runtimeSurfaces: [{
        surfaceId: "preview-root",
        kind: "http | cli | library | worker | desktop | mobile | none",
        probe: {
          type: "http_path | command | import_check | none",
          target: "/ or command/module target or null",
          expected: "non-error response or expected command/import result",
        },
      }],
      deliveryMechanics: {
        staticAssets: {
          required: true,
          source: "technology-specific source root or null",
          output: "technology-specific output dir or null",
          servedBy: "technology-specific serving mechanism",
        },
        api: {
          required: true,
          entry: "technology-specific entry or null",
          basePath: "technology-specific base path or null",
          probePaths: ["technology-specific probe paths"],
        },
        codegen: {
          required: "yes | no | if_applicable",
          commands: ["technology-specific codegen commands"],
          codeLevelExpectations: [
            "Generated clients/types required for build are produced by declared scripts when applicable.",
          ],
        },
      },
      httpProbes: {
        previewPath: "/",
        healthPath: "Optional string. Omit when there is no separate health path; do not write null.",
        apiPaths: ["/api/example"],
        expectedStatus: "2xx_or_3xx",
      },
      frontend: {
        required: true,
        kind: "vite_react | next | static | none | other",
        buildCommand: "technology-appropriate frontend build command",
        sourceRoot: "frontend source root",
        outputDir: "frontend output dir",
        servedBy: "express_static | vite_preview | nginx_static | framework_server | not_applicable",
        servedByRef: "project-relative file/path that serves frontend output",
        codeLevelExpectations: ["Frontend artifact/serving expectations."],
      },
      api: {
        required: true,
        kind: "express | fastapi | spring_boot | go_http | none | other",
        buildCommand: "Optional non-null string. Omit when api.required=false or no separate API build exists; do not write null.",
        entry: "Optional non-null string. Omit when api.required=false or no API entry exists; do not write null.",
        basePath: "Optional non-null string. Omit when api.required=false or no API base path exists; do not write null.",
        probePaths: ["/api/example"],
        codeLevelExpectations: ["API route/probe expectations."],
      },
      environment: {
        required: ["required env var"],
        optional: ["optional env var"],
      },
      taskPlanningGuidance: {
        requireRuntimeDeliveryRequirementWhenTaskTouches: [
          "build_or_packaging",
          "runtime_entry",
          "serving_or_routing",
          "configuration_or_environment",
          "generated_artifacts",
          "runtime_surface",
        ],
        doNotRequireForTaskKinds: [
          "domain_only_validation",
          "copy_only_documentation",
          "pure_unit_test_additions",
        ],
        verificationBoundary: "code_level_only",
        doNotRequireCleanInstallOrContainerBuild: true,
      },
      risks: ["runtime delivery risks or assumptions"],
    },
    fieldPresenceMatrix: runtimeDeliveryFieldPresenceMatrix(),
    runtimeDeliveryRules: [
      ...runtimeDeliverySectionGenerationRules(),
    ],
  };
}

function runtimeDeliveryFieldPresenceMatrix(): Record<string, unknown> {
  return {
    whenStatusModified: {
      requiredObjects: [
        "basis",
        "build",
        "start",
        "runtimeSurfaces",
        "deliveryMechanics",
        "deliveryMechanics.staticAssets",
        "deliveryMechanics.api",
        "deliveryMechanics.codegen",
        "httpProbes",
        "frontend",
        "environment",
        "taskPlanningGuidance",
      ],
      requiredFields: [
        "basis.technicalBaselineRef",
        "basis.planningGenerationContractRef",
        "basis.reason",
        "build.command",
        "build.workingDirectory",
        "build.outputs",
        "build.codeLevelExpectations",
        "start.command",
        "start.workingDirectory",
        "start.host",
        "start.port",
        "start.portEnv",
        "start.codeLevelExpectations",
        "deliveryMechanics.staticAssets.required",
        "deliveryMechanics.api.required",
        "deliveryMechanics.codegen.required",
        "deliveryMechanics.codegen.commands",
        "deliveryMechanics.codegen.codeLevelExpectations",
        "httpProbes.previewPath",
        "httpProbes.apiPaths",
        "httpProbes.expectedStatus",
        "frontend.required",
        "frontend.kind",
        "frontend.codeLevelExpectations",
        "environment.required",
        "environment.optional",
        "taskPlanningGuidance.requireRuntimeDeliveryRequirementWhenTaskTouches",
        "taskPlanningGuidance.doNotRequireForTaskKinds",
        "taskPlanningGuidance.verificationBoundary",
        "taskPlanningGuidance.doNotRequireCleanInstallOrContainerBuild",
      ],
    },
    omitWhenNotApplicableNeverNull: [
      "httpProbes.healthPath",
      "api",
      "api.buildCommand",
      "api.entry",
      "api.basePath",
      "frontend.servedByRef",
      "frontend.buildCommand",
      "frontend.sourceRoot",
      "frontend.outputDir",
      "frontend.servedBy",
      "start.entry",
      "deliveryMechanics.staticAssets.source",
      "deliveryMechanics.staticAssets.output",
      "deliveryMechanics.staticAssets.servedBy",
    ],
    nullableOnlyWhereExplicitlyAllowed: [
      "basis.repositoryContextRef",
      "basis.previousRuntimeDeliveryRef",
      "deliveryMechanics.api.entry",
      "deliveryMechanics.api.basePath",
      "runtimeSurfaces[].probe.target",
    ],
    apiRules: [
      "Top-level runtimeDelivery.api is optional. Omit the entire object when no API contract is required.",
      "deliveryMechanics.api is required for status=modified and must always include required:boolean and probePaths:array; entry/basePath may be null only there.",
      "If deliveryMechanics.api.required=false, set deliveryMechanics.api.probePaths=[] and omit top-level runtimeDelivery.api.",
    ],
  };
}

function runtimeDeliverySectionGenerationRules(): string[] {
  return [
      "ArchitectureSectionCandidate.status is the wrapper status and must be ready when this runtime_delivery section is successfully generated; runtimeDelivery.status may independently be modified, unchanged, or not_applicable.",
      "Use sourceRefs.technicalBaselineRef as the authoritative technology stack input for runtimeDelivery.",
      "Do not choose a package manager, framework, runtime model, build tool, or start model that conflicts with TechnicalBaseline.",
      "Use sourceRefs.repositoryContextRef as repository-state evidence for existing files, scripts, runtime entries, and generated artifacts when present; omit runtimeDelivery.basis.repositoryContextRef or set it to null when absent.",
      "Use sourceRefs.planningContractRef as current phase scope authority.",
      "Use sourceRefs.previousRuntimeDeliveryRef as the only valid value for runtimeDelivery.basis.previousRuntimeDeliveryRef when runtimeDelivery.status=unchanged.",
      "If sourceRefs.previousRuntimeDeliveryRef is absent, do not write runtimeDelivery.status=unchanged; write modified when the current phase has a runtime shape, or blocked/not_applicable when appropriate.",
      "If RepositoryContext conflicts with TechnicalBaseline, record a risk/decision or return blocked; do not silently switch stacks.",
      "RuntimeDeliveryContract is a code/script/runtime-shape contract, not deploy execution proof.",
      "Do not require Docker, clean install, registry access, or browser-level proof in AAC.",
      "Do not create deploy files or modify application code while writing this section.",
      "Populate taskPlanningGuidance so TaskPlan can derive task-level runtimeDeliveryRequirement.",
      "Follow runtimeDelivery fieldPresenceMatrix exactly: required fields must be present for status=modified; omitWhenNotApplicableNeverNull fields must be omitted instead of set to null; nullableOnlyWhereExplicitlyAllowed fields are the only fields that may use null.",
      "Omit top-level runtimeDelivery.api entirely when no API contract is required. Do not create api.required=false as a top-level placeholder.",
      "For optional non-null string fields such as runtimeDelivery.httpProbes.healthPath and runtimeDelivery.api.buildCommand/entry/basePath, omit the field when it does not apply. Do not write null unless the fieldPresenceMatrix explicitly allows it.",
      "For nullable fields such as runtimeDelivery.deliveryMechanics.api.entry/basePath or basis.previousRuntimeDeliveryRef, null is allowed only when the fieldPresenceMatrix says nullable.",
    ];
}

function runtimeDeliverySectionEnumRefs(): Record<string, string[]> {
  return {
    runtimeDeliveryStatus: [...runtimeDeliveryStatusSchema.options],
    expectedStatus: ["2xx_or_3xx"],
    codegenRequired: [...runtimeDeliveryCodegenRequiredSchema.options],
    verificationBoundary: [runtimeDeliveryVerificationBoundarySchema.value],
    taskTouchCategory: [
      "build_or_packaging",
      "runtime_entry",
      "serving_or_routing",
      "configuration_or_environment",
      "generated_artifacts",
      "runtime_surface",
    ],
    runtimeSurfaceProbeType: ["http_path", "command", "import_check", "none"],
  };
}

function coverageSectionContentShape(): Record<string, unknown> {
  return {
    acceptanceMatrix: [{
      acceptanceId: "AC-ref-from-allowedRefs.acceptanceRefs",
      priority: "must | should | could",
      statement: "Acceptance statement copied or summarized from PGC.",
      coverageStatus: "covered | partial | not_applicable | deferred | uncovered",
      reason: "Required when coverage is not fully covered.",
      reasonCategory: "candidate_incomplete | reference_error | scope_conflict | baseline_conflict | user_tradeoff | upstream_invalid",
      coverage: [{
        type: "module | data_entity | data_constraint | relationship | interface | user_flow | state_machine | state_rule | decision | risk",
        refs: ["exact artifact id from the matching AAC section; do not prefix ids with field names such as constraintId:"],
        description: "How this artifact covers the acceptance.",
      }],
      coverageTypeRules: [
        "coverage[].type must match the real artifact kind for every ref.",
        "module refs must come from foundation.content.modules[].moduleId and use type=module.",
        "data_entity refs must come from domain_contract.content.dataModel.entities[].entityId and use type=data_entity.",
        "data_constraint refs must come from domain_contract entity/global constraintId values and use type=data_constraint.",
        "relationship refs must come from domain_contract.content.dataModel.relationships[].relationshipId and use type=relationship.",
        "interface refs must come from domain_contract.content.interfaces[].interfaceId and use type=interface.",
        "user_flow refs must come from behavior.content.userFlows[].flowId and use type=user_flow.",
        "state_machine refs must come from behavior.content.stateMachines[].stateMachineId and use type=state_machine.",
        "state_rule refs must come from behavior.content.stateMachines[].rules[].ruleId and use type=state_rule.",
        "decision refs must come from coverage.content.risksAndDecisions.decisions[].decisionId and use type=decision.",
        "risk refs must come from coverage.content.risksAndDecisions.risks[].riskId and use type=risk.",
      ],
      verificationHints: [{
        kind: "unit | integration | e2e | manual | static | contract",
        description: "How later TaskPlan/Review can verify this acceptance.",
      }],
    }],
    detailCoverage: [{
      detailId: "detail-id-from-request.contextProjection.requirementDetailTransfer.requirementDetails.items",
      coverageStatus: "covered | partial | not_applicable | uncovered",
      artifactRefs: {
        modules: ["module-id"],
        entities: ["entity-id"],
        fields: ["field-id"],
        constraints: ["constraint-id"],
        interfaces: ["interface-id"],
        userFlows: ["flow-id"],
        stateMachines: ["state-machine-id"],
        frontendDataViews: ["view-id"],
        frontendActions: ["action-id"],
        frontendOperationPaths: ["path-id"],
        acceptanceMatrix: ["AC-ref-from-allowedRefs.acceptanceRefs"],
      },
      reason: "Required when coverageStatus is partial, not_applicable, or uncovered. Omit for fully covered entries.",
    }],
    detailCoverageRules: [
      "detailCoverage[].detailId must come from request.contextProjection.requirementDetailTransfer.requirementDetails.items[].detailId.",
      "detailCoverage stores only detailId, coverageStatus, artifactRefs, and reason; do not copy full requirement detail summary into coverage.",
      "covered detailCoverage entries must reference at least one real AAC artifact or acceptanceMatrix entry.",
      "partial, not_applicable, and uncovered detailCoverage entries require a non-empty reason.",
    ],
    risksAndDecisions: {
      decisions: [{
        decisionId: "decision-id",
        type: "architecture | scope | baseline | delivery | validation | implementation",
        title: "Decision title",
        decision: "Decision text. Use null only when status=needs_user_decision.",
        rationale: "Why this decision is proposed or accepted.",
        scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
        status: "proposed | accepted | needs_user_decision | superseded",
        decisionCategory: "Optional. Required only when status=needs_user_decision. One of: scope_change | baseline_change | architecture_tradeoff | acceptance_conflict | defer_or_include.",
        decisionQuestion: "Optional. Required only when status=needs_user_decision. Omit this field for proposed/accepted/superseded decisions; do not set it to null.",
        options: [{
          optionId: "option-id",
          label: "Option label",
        }],
        allowFreeform: true,
        impact: {
          requiresScopeRevision: false,
          requiresBaselineRevision: false,
          requiresPgcRegeneration: false,
          requiresAacRegeneration: false,
        },
      }],
      risks: [{
        riskId: "risk-id",
        type: "architecture | implementation | data | integration | delivery | validation",
        title: "Risk title",
        description: "Risk description.",
        severity: "low | medium | high | blocking",
        mitigation: "Optional mitigation.",
        scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
        acceptanceRefs: ["AC-ref-from-allowedRefs.acceptanceRefs"],
        status: "open | mitigated | accepted | closed",
      }],
      assumptions: [{
        assumptionId: "assumption-id",
        statement: "Assumption statement.",
        scopeRefs: ["scope-ref-from-allowedRefs.scopeRefs"],
        entityRefs: ["entity-id-from-domain-contract"],
        status: "active | resolved | superseded",
      }],
      deferredNotes: [{
        deferredRef: "deferred-scope-ref-from-allowedRefs.deferredScopeRefs",
        reason: "Why this is deferred.",
        impactOnCurrentPhase: "How deferral affects current phase design.",
      }],
    },
    decisionFieldRules: [
      "For decisions where status is proposed, accepted, or superseded, omit decisionQuestion, options, allowFreeform, and impact unless they are meaningful non-null values.",
      "For decisions where status is needs_user_decision, decision must be null and decisionQuestion/options/allowFreeform/impact are required.",
      "Never write null for optional fields such as decisionQuestion, decisionCategory, options, allowFreeform, or impact; omit the field instead.",
    ],
    handoff: {
      readyForTaskPlan: true,
      blockingReasons: [],
      nextNode: "task_plan | architecture_artifact_repair | needs_user_decision | blocked",
    },
  };
}

function coverageSectionGenerationRules(): string[] {
  return [
    "Use request.contextProjection.requirementDetailTransfer.acceptanceDetails as the acceptance detail source for coverage.",
    "Use request.contextProjection.requirementDetailTransfer.requirementDetails.items as the canonical detail index for detailCoverage when present.",
    "For every current-phase requirementDetails item, write one detailCoverage entry that references the AAC artifacts carrying the detail or explains why the detail is partial, not_applicable, or uncovered.",
    "Do not copy full requirement detail text into detailCoverage; detailCoverage must use detailId plus artifactRefs only.",
    "For every acceptanceMatrix entry, preserve the PGC acceptance statement exactly and describe how current AAC artifacts cover the concrete rule, field, flow, state, blocking reason, or source-ref detail behind that acceptance.",
    "Verification hints should reflect the concrete requirement details carried from PGC, not only the high-level module label.",
    "Coverage refs must point to artifact ids declared in this current phase AAC, not directly to old phase AAC ids.",
    "If a must acceptance depends on an earlier phase entity, the current domain_contract section must first include that entity as reference_only or read_only_projection, then coverage may reference that current section entity id.",
    "Use coverage[].type=data_entity only for refs from current domain_contract.content.dataModel.entities[].entityId.",
    "Use coverage[].type=interface only for refs from current domain_contract.content.interfaces[].interfaceId.",
    "Use coverage[].type=user_flow only for refs from current behavior.content.userFlows[].flowId.",
  ];
}

function architectureSectionOutputs(
  root: string,
  locator: DeliveryPhaseLocator,
  requestId: string,
): ArchitectureSectionsGenerationRequest["outputContract"]["sectionOutputs"] {
  return [
    {
      section: "foundation",
      schemaRef: "architecture-section-foundation-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "foundation")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "foundation", foundationSectionContentShape(locator)),
      generationRules: architectureSectionGenerationRules(foundationSectionGenerationRules()),
    },
    {
      section: "domain_contract",
      schemaRef: "architecture-section-domain-contract-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "domain_contract")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "domain_contract", domainContractSectionContentShape()),
      generationRules: architectureSectionGenerationRules(domainContractSectionGenerationRules()),
    },
    {
      section: "behavior",
      schemaRef: "architecture-section-behavior-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "behavior")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "behavior", behaviorSectionContentShape()),
      generationRules: architectureSectionGenerationRules(behaviorSectionGenerationRules()),
    },
    {
      section: "frontend_experience",
      schemaRef: "architecture-section-frontend-experience-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "frontend_experience")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "frontend_experience", frontendExperienceSectionContentShape()),
      generationRules: architectureSectionGenerationRules([
        "Read request.frontendExperienceSource before writing this section.",
        "Use request.contextProjection.requirementDetailTransfer and PGC frontend refs to preserve current phase input, display, feedback, and workflow expectations confirmed in Brainstorm.",
        "When PGC details name user-facing fields, statuses, blocking feedback, required workflow surfaces, or frontend/backend interaction expectations, represent them in surfaces, navigation, interactionStates, notes, and mustNot as appropriate.",
        "When Brainstorm frontendExperience contains dataViews, actions, or operationPaths, preserve the current phase relevant entries in content.frontendExperience.dataViews/actions/operationPaths and connect them to userFlows where possible.",
        "For existing-object operation paths, prefer a paginated query/select view unless the confirmed frontend target says direct id entry, preselected context, or not applicable. Do not invent search criteria beyond confirmed fields.",
        "If frontendExperienceSource.confirmedFrontendExperienceRef or currentFrontendExperienceRef is present, include content.frontendExperience and set content.frontendExperience.sourceRefs.brainstormFrontendExperienceRef to that exact ref.",
        "If frontendExperienceSource.repositoryContextRef is null or absent, omit content.frontendExperience.sourceRefs.repositoryContextRef; do not write null for optional source refs.",
        "Use RepositoryContext and TechnicalBaseline only as implementation facts; do not downgrade the user-confirmed frontend target.",
        "If the user confirmed frontend required=false or experienceLevel=none, represent that target explicitly instead of inventing UI work.",
      ]),
    },
    {
      section: "runtime_delivery",
      schemaRef: "architecture-section-runtime-delivery-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "runtime_delivery")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "runtime_delivery", runtimeDeliverySectionContentShape(locator)),
      enumRefs: runtimeDeliverySectionEnumRefs(),
      generationRules: architectureSectionGenerationRules(runtimeDeliverySectionGenerationRules()),
    },
    {
      section: "coverage",
      schemaRef: "architecture-section-coverage-v1",
      candidateFile: toProjectRelative(root, architectureSectionCandidatePath(root, locator, requestId, "coverage")),
      schemaShape: architectureSectionSchemaShape(locator, requestId, "coverage", coverageSectionContentShape()),
      generationRules: architectureSectionGenerationRules(coverageSectionGenerationRules()),
    },
  ];
}

async function assembleArchitectureCandidateFromSections(
  root: string,
  locator: DeliveryPhaseLocator,
  requestId: string,
): Promise<{ value: unknown; issues: ArchitectureAcceptResult["issues"] }> {
  const requestFile = architectureRequestPath(root, requestId, locator);
  const request = architectureSectionsGenerationRequestSchema.parse(await hydrateRequestManifest(root, requestFile));
  const sectionOutputs = architectureRequestSectionOutputs(request);
  const sectionByName = new Map<string, ArchitectureSectionCandidate>();
  for (const output of sectionOutputs) {
    const candidateJson = await readJsonFile(resolveCliPath(root, output.candidateFile));
    const parsed = architectureSectionCandidateSchema.safeParse(candidateJson);
    if (!parsed.success) {
      return {
        value: null,
        issues: architectureSectionSchemaIssues(output.section, output.candidateFile, parsed.error),
      };
    }
    const candidate = parsed.data;
    if (candidate.requestId !== requestId || candidate.deliveryId !== locator.deliveryId || candidate.phaseId !== locator.phaseId || candidate.section !== output.section) {
      throw invalidArgument("Architecture section candidate does not match active request.", {
        requestId,
        section: output.section,
        candidateFile: output.candidateFile,
      });
    }
    if (candidate.status !== "ready") {
      throw invalidArgument("Architecture section candidate is blocked.", {
        requestId,
        section: output.section,
        blockedReasons: candidate.blockedReasons ?? [],
      });
    }
    sectionByName.set(output.section, candidate);
  }
  const foundation = sectionByName.get("foundation")?.content ?? {};
  const domain = sectionByName.get("domain_contract")?.content ?? {};
  const behavior = sectionByName.get("behavior")?.content ?? {};
  const frontend = sectionByName.get("frontend_experience")?.content ?? {};
  const runtime = sectionByName.get("runtime_delivery")?.content ?? {};
  const coverage = sectionByName.get("coverage")?.content ?? {};
  const now = new Date().toISOString();
  const assembled = {
    schemaVersion: "1.0",
    architectureArtifactContractId: createId("aac"),
    status: "ready",
    source: foundation.source,
    engineeringBoundary: foundation.engineeringBoundary,
    modules: foundation.modules,
    dataModel: domain.dataModel,
    interfaces: domain.interfaces,
    userFlows: behavior.userFlows ?? [],
    stateMachines: behavior.stateMachines ?? [],
    frontendExperience: frontend.frontendExperience,
    runtimeDelivery: runtime.runtimeDelivery,
    acceptanceMatrix: coverage.acceptanceMatrix,
    detailCoverage: coverage.detailCoverage ?? [],
    risksAndDecisions: coverage.risksAndDecisions,
    handoff: coverage.handoff,
    createdAt: now,
    updatedAt: now,
  };
  let version = 1;
  for (const output of sectionOutputs) {
    const candidate = sectionByName.get(output.section);
    if (candidate) {
      await writeJsonAtomic(architectureSectionVersionPath(root, locator, output.section, version), candidate);
    }
  }
  await writeJsonAtomic(architectureSessionPath(root, locator, requestId), {
    schemaVersion: "1.0",
    requestId,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    acceptedSections: sectionOutputs.map((output) => ({
      section: output.section,
      version,
      candidateFile: output.candidateFile,
    })),
    assembledArchitectureArtifactContractId: typeof assembled.architectureArtifactContractId === "string"
      ? assembled.architectureArtifactContractId
      : null,
    assembledAt: now,
  });
  return { value: assembled, issues: [] };
}

function architectureSectionSchemaIssues(
  section: ArchitectureSectionCandidate["section"],
  candidateFile: string,
  error: ZodError,
): ArchitectureAcceptResult["issues"] {
  return error.issues.map((zodIssue) => {
    const pathSuffix = zodIssue.path.map(String).join("/");
    const pointer = `/sections/${section}${pathSuffix ? `/${pathSuffix}` : ""}`;
    const base = issue("SCHEMA_INVALID", pointer);
    const allowedValues = "options" in zodIssue && Array.isArray(zodIssue.options)
      ? ` Allowed values: ${zodIssue.options.join(", ")}.`
      : "";
    return {
      ...base,
      message: `Architecture section candidate ${section} does not match its section schema at ${pointer}: ${zodIssue.message}.${allowedValues}`,
      repairHint: `Edit only ${candidateFile}. Follow the section schema exactly; when a field is an enum, write one allowed value, not a pipe-joined list.`,
    };
  });
}

async function latestId(filePath: string, key: string): Promise<string | null> {
  if (!(await pathExists(filePath))) {
    return null;
  }
  const json = await readJsonFile(filePath);
  if (typeof json === "object" && json !== null && key in json) {
    const value = (json as Record<string, unknown>)[key];
    return typeof value === "string" ? value : null;
  }
  return null;
}

function parseStored<T>(schema: { parse(value: unknown): T }, json: unknown, filePath: string): T {
  try {
    return schema.parse(json);
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("Stored loom contract does not match schema.", {
        file: filePath,
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

function resolveCliPath(projectRoot: string, filePath: string): string {
  return path.isAbsolute(filePath) ? filePath : fromProjectRelative(projectRoot, filePath);
}

async function listProjectFiles(projectRoot: string): Promise<string[]> {
  const files: string[] = [];
  async function walk(current: string): Promise<void> {
    let entries: import("node:fs").Dirent[];
    try {
      entries = await fs.readdir(current, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if ([".git", ".loom", "node_modules", "dist", "build"].includes(entry.name)) {
        continue;
      }
      const absolute = path.join(current, entry.name);
      const relative = toProjectRelative(projectRoot, absolute);
      if (entry.isDirectory()) {
        await walk(absolute);
      } else {
        files.push(relative);
      }
      if (files.length > 500) {
        return;
      }
    }
  }
  await walk(projectRoot);
  return files.sort();
}

async function readJsonObjectIfExists(filePath: string): Promise<Record<string, unknown> | null> {
  try {
    const raw = await fs.readFile(filePath, "utf8");
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

function extractScripts(packageJson: Record<string, unknown> | null): Record<string, string> {
  const scripts = packageJson?.scripts;
  if (typeof scripts !== "object" || scripts === null || Array.isArray(scripts)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(scripts).filter((entry): entry is [string, string] => typeof entry[1] === "string"),
  );
}

function detectPackageManagers(files: string[]): string[] {
  const managers: string[] = [];
  if (files.includes("pnpm-lock.yaml")) managers.push("pnpm");
  if (files.includes("package-lock.json")) managers.push("npm");
  if (files.includes("yarn.lock")) managers.push("yarn");
  if (files.includes("bun.lockb")) managers.push("bun");
  if (files.includes("pom.xml")) managers.push("maven");
  if (files.some((file) => file.endsWith("build.gradle"))) managers.push("gradle");
  if (files.includes("composer.json")) managers.push("composer");
  if (files.includes("Gemfile") || files.includes("Gemfile.lock")) managers.push("bundler");
  if (files.some((file) => file.endsWith(".csproj"))) managers.push("dotnet");
  return managers;
}

function detectLanguages(files: string[]): string[] {
  const languages = new Set<string>();
  for (const file of files) {
    if (/\.(ts|tsx)$/.test(file)) languages.add("TypeScript");
    if (/\.(js|jsx|mjs|cjs)$/.test(file)) languages.add("JavaScript");
    if (/\.java$/.test(file)) languages.add("Java");
    if (/\.py$/.test(file)) languages.add("Python");
    if (/\.go$/.test(file)) languages.add("Go");
    if (/\.rs$/.test(file)) languages.add("Rust");
    if (/\.php$/.test(file)) languages.add("PHP");
    if (/\.rb$/.test(file)) languages.add("Ruby");
    if (/\.cs$/.test(file)) languages.add("C#");
  }
  return [...languages];
}

function detectFrameworkHints(files: string[], packageJson: Record<string, unknown> | null): string[] {
  const text = JSON.stringify(packageJson ?? {});
  const hints = new Set<string>();
  for (const name of ["react", "vite", "vue", "next", "express", "fastify", "nestjs", "spring", "prisma", "laravel", "rails", "django", "fastapi", "aspnetcore"]) {
    if (text.toLowerCase().includes(name) || files.some((file) => file.toLowerCase().includes(name))) {
      hints.add(name);
    }
  }
  return [...hints];
}

function detectTestHints(files: string[], packageJson: Record<string, unknown> | null): string[] {
  const text = JSON.stringify(packageJson ?? {}).toLowerCase();
  const hints = new Set<string>();
  for (const name of ["vitest", "jest", "playwright", "cypress", "junit", "pytest"]) {
    if (text.includes(name) || files.some((file) => file.toLowerCase().includes(name))) {
      hints.add(name);
    }
  }
  return [...hints];
}

const MANIFEST_FILES = new Set([
  "package.json",
  "tsconfig.json",
  "vite.config.ts",
  "vite.config.js",
  "next.config.js",
  "pnpm-lock.yaml",
  "package-lock.json",
  "yarn.lock",
  "composer.json",
  "composer.lock",
  "Gemfile",
  "Gemfile.lock",
]);

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}
