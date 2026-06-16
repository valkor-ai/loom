import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import { invalidArgument, stateNotInitialized } from "../errors";
import {
  type OperationLease,
  type RouteDecision,
  routeDecisionSchema,
} from "../contracts";
import { pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import {
  getActiveLocator,
  loadDeliveryIndex,
  loadProjectStatus,
  upsertStatusDelivery,
} from "../state/delivery";
import {
  brainstormContractSchema,
  type DeliveryIndexPhase,
  type RouteAction,
} from "../schemas";
import {
  brainstormContractPath,
  continueDecisionLatestPath,
  deliveryIndexPath,
  operationLeasePath,
  reviewLatestPath,
  toProjectRelative,
} from "../state/paths";
import { closeOperationLease, markOperationLeaseStale, readOperationLease, refreshOperationLease, updateRouteState } from "./control";
import { controlledRuntimeProbeRules, getNextTask, materializeExecutionRepairTask, taskExecutionCompletionContinuityRequirement, verificationCommandSchedulingRules } from "./tasks";
import { artifactInstructionPolicy, brainstormAskUserInstructionPolicy, brainstormAskUserReadStep, compactContextReadStep, taskExecutionOutputPolicy } from "./output-policy";
import { autoRunInstruction, withAutoRunnableTransition } from "./routing-instructions";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "./request-manifest";
import { possibleRuntimeForegroundStall } from "./runtime-stall";
import { brainstormSessionAgentActionContract, normalizeAgentActionForFieldGroups } from "./agent-action";
import {
  architectureSingleSectionCompletionBarrier,
  architectureSingleSectionCompletionCondition,
  architectureSingleSectionRequiredSteps,
  architectureSingleSectionWriteTarget,
} from "./architecture-section-completion";
import { completedDeliveryUserMessage, type LoomCommandSurface } from "./user-guidance";

const execFileAsync = promisify(execFile);

const taskExecutionCompletionBarrierRules = [
  "A TaskExecutionRequest is not complete until TaskResult JSON exists at outputContract.resultFile and submitCommand has been run successfully.",
  "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
  "If implementation or verification cannot be completed inside the current task boundary, still write a failed or blocked TaskResult and run submitCommand so loom can route repair or user decision.",
  "Only stop before submitCommand when requestRef cannot be read, submitCommand returns a non-repairable failure, or the returned instruction is user-gated.",
];

export type ContinueDeliveryInput = {
  projectRoot: string;
  commandSurface?: LoomCommandSurface;
};

export async function continueDelivery(input: ContinueDeliveryInput): Promise<RouteDecision> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const commandSurface = input.commandSurface ?? "@loom";
  const completedDecision = await completedDeliveryDecision(root, commandSurface);
  if (completedDecision) {
    return completedDecision;
  }
  const locator = await getActiveLocator(root);
  const delivery = await loadDeliveryIndex(root, locator.deliveryId);
  const phase = delivery.phases.find((item) => item.phaseId === locator.phaseId);
  if (!phase) {
    throw invalidArgument("Active phase does not exist.", locator);
  }
  const lease = await readActiveLease(root, locator.deliveryId);
  const now = new Date().toISOString();
  if (lease && phase.nextAction && isUserGatedActionType(phase.nextAction.type)) {
    return routeDecisionFromAction({
      root,
      delivery,
      phase,
      locator,
      action: phase.nextAction,
      now,
      staleLeaseRecovered: false,
      commandSurface,
    });
  }
  if (lease) {
    const recoverable = await recoverableGenerationInstruction(root, lease);
    if (recoverable) {
      const fresh = new Date(lease.expiresAt).getTime() > Date.now();
      const activeOperation = fresh
        ? lease
        : await refreshOperationLease(root, locator.deliveryId, "continue_resume_expired_recoverable_operation") ?? lease;
      const decision = routeDecisionSchema.parse({
        schemaVersion: "1.0",
        decisionId: createId("continue"),
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        status: "ready",
        source: {
          command: "continue",
          type: "active_operation_lease",
          ref: toProjectRelative(root, operationLeasePath(root, locator.deliveryId)),
          validated: true,
        },
        nextAction: {
          type: recoverable.nextActionType,
          targetNode: recoverable.nextActionType,
          reason: "RECOVER_INCOMPLETE_GENERATION_REQUEST",
        },
        instruction: recoverable.instruction,
        materialization: {
          attempted: false,
          status: "already_exists",
          requestRef: recoverable.requestRef,
          candidateFile: recoverable.candidateFile,
          leaseRef: toProjectRelative(root, operationLeasePath(root, locator.deliveryId)),
        },
        concurrency: {
          leaseChecked: true,
          blockedByFreshLease: false,
          staleLeaseRecovered: !fresh,
          activeOperation,
          recoverableActiveOperation: true,
          recoveryReason: recoverable.recoveryReason,
        },
        ...(recoverable.possibleRuntimeForegroundStall
          ? { possibleRuntimeForegroundStall: recoverable.possibleRuntimeForegroundStall }
          : {}),
        createdAt: now,
      });
      await writeJsonAtomic(continueDecisionLatestPath(root, locator), decision);
      return decision;
    }
  }
  if (lease && new Date(lease.expiresAt).getTime() > Date.now()) {
    const decision = routeDecisionSchema.parse({
      schemaVersion: "1.0",
      decisionId: createId("continue"),
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      status: "blocked",
      source: {
        command: "continue",
        type: "active_operation_lease",
        ref: toProjectRelative(root, operationLeasePath(root, locator.deliveryId)),
        validated: true,
      },
      nextAction: {
        type: "blocked",
        targetNode: "active_operation",
        reason: "ACTIVE_OPERATION_RUNNING",
      },
      instruction: {
        mode: "report_blocked",
        userMessage: "当前还有 loom 操作正在执行，暂时不能启动新的下一步。",
      },
      materialization: {
        attempted: false,
        status: "skipped_blocked",
        requestRef: null,
        candidateFile: null,
        leaseRef: toProjectRelative(root, operationLeasePath(root, locator.deliveryId)),
      },
      concurrency: {
        leaseChecked: true,
        blockedByFreshLease: true,
        staleLeaseRecovered: false,
        activeOperation: lease,
      },
      createdAt: now,
    });
    await writeJsonAtomic(continueDecisionLatestPath(root, locator), decision);
    return decision;
  }
  if (lease) {
    await markOperationLeaseStale(root, locator.deliveryId, "continue_recovered_expired_lease");
  }
  if (phase.nextAction?.type === "continue_to_next_phase") {
    const activation = await activateNextPhase(root, delivery, locator.phaseId, phase.nextAction.targetPhaseId);
    if (!activation) {
      const refreshedDelivery = await loadDeliveryIndex(root, locator.deliveryId);
      const refreshedPhase = refreshedDelivery.phases.find((item) => item.phaseId === locator.phaseId) ?? phase;
      return routeDecisionFromAction({
        root,
        delivery: refreshedDelivery,
        phase: refreshedPhase,
        locator,
        action: refreshedPhase.nextAction ?? {
          type: "done",
          source: "continue",
          deliveryId: locator.deliveryId,
          phaseId: locator.phaseId,
          reason: "ROADMAP_COMPLETED",
        },
        now,
        staleLeaseRecovered: Boolean(lease),
        commandSurface,
      });
    }
    const refreshedDelivery = await loadDeliveryIndex(root, activation.deliveryId);
    const refreshedPhase = refreshedDelivery.phases.find((item) => item.phaseId === activation.phaseId);
    if (!refreshedPhase) {
      throw invalidArgument("Activated phase does not exist.", activation);
    }
    return routeDecisionFromAction({
      root,
      delivery: refreshedDelivery,
      phase: refreshedPhase,
      locator: activation,
      action: refreshedPhase.nextAction ?? defaultNextActionForActivatedPhase(refreshedPhase, activation),
      now,
      staleLeaseRecovered: Boolean(lease),
      transition: {
        type: "phase_activated",
        fromPhaseId: locator.phaseId,
        toPhaseId: activation.phaseId,
        reason: "REVIEW_APPROVED_NEXT_PHASE",
      },
      commandSurface,
    });
  }

  const nextAction = phase.nextAction ?? {
    type: "done",
    source: "delivery_index",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    reason: delivery.status === "completed" ? "DELIVERY_COMPLETED" : "NO_NEXT_ACTION",
  };
  return routeDecisionFromAction({
    root,
    delivery,
    phase,
    locator,
    action: nextAction,
    now,
    staleLeaseRecovered: Boolean(lease),
    commandSurface,
  });
}

async function completedDeliveryDecision(root: string, commandSurface: LoomCommandSurface): Promise<RouteDecision | null> {
  const status = await loadProjectStatus(root);
  if (status.activeDeliveryId || status.phase !== "completed" || !status.lastCompletedDeliveryId) {
    return null;
  }
  const delivery = await loadDeliveryIndex(root, status.lastCompletedDeliveryId);
  const phaseId = delivery.activePhaseId;
  const phase = delivery.phases.find((item) => item.phaseId === phaseId);
  if (!phase) {
    throw invalidArgument("Last completed delivery active phase does not exist.", {
      deliveryId: delivery.deliveryId,
      phaseId,
    });
  }
  const now = new Date().toISOString();
  const decision = routeDecisionSchema.parse({
    schemaVersion: "1.0",
    decisionId: createId("continue"),
    deliveryId: delivery.deliveryId,
    phaseId,
    status: "done",
    source: {
      command: "continue",
      type: "completed_delivery",
      ref: toProjectRelative(root, deliveryIndexPath(root, delivery.deliveryId)),
      validated: true,
    },
    nextAction: {
      type: "done",
      targetNode: "done",
      reason: "DELIVERY_ALREADY_COMPLETED",
    },
    instruction: {
      mode: "report_done",
      userMessage: completedDeliveryUserMessage(commandSurface),
    },
    materialization: {
      attempted: false,
      status: "not_applicable",
      requestRef: null,
      candidateFile: null,
      leaseRef: null,
    },
    concurrency: {
      leaseChecked: false,
      blockedByFreshLease: false,
      staleLeaseRecovered: false,
      activeOperation: null,
    },
    createdAt: now,
  });
  const locator = { deliveryId: delivery.deliveryId, phaseId };
  await writeJsonAtomic(continueDecisionLatestPath(root, locator), decision);
  return decision;
}

async function routeDecisionFromAction(input: {
  root: string;
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>;
  phase: Awaited<ReturnType<typeof loadDeliveryIndex>>["phases"][number];
  locator: { deliveryId: string; phaseId: string };
  action: NonNullable<Awaited<ReturnType<typeof loadDeliveryIndex>>["phases"][number]["nextAction"]>;
  now: string;
  staleLeaseRecovered?: boolean;
  transition?: Record<string, unknown>;
  commandSurface?: LoomCommandSurface;
}): Promise<RouteDecision> {
  const { root, delivery, locator, action: nextAction, now } = input;
  if (nextAction.type === "continue_execution") {
    const nextTask = await getNextTask({
      projectRoot: root,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
    });
    if (nextTask.hasTask && nextTask.instruction) {
      await updateRouteState({
        projectRoot: root,
        locator,
        deliveryStatus: "executing",
        phaseStatus: "executing",
        latestRefs: {},
        nextAction: {
          ...nextAction,
          ref: nextTask.executionRequestPath,
          refs: {
            ...(nextAction.refs ?? {}),
            taskId: nextTask.task?.taskId ?? null,
            groupId: nextTask.executionRequest?.groupId ?? nextTask.task?.groupId ?? null,
            taskPlanRunId: nextTask.executionRequest?.taskPlanRunId ?? null,
            executionRequestRef: nextTask.executionRequestPath,
            resultFile: typeof nextTask.executionRequest?.resultFile === "string" ? nextTask.executionRequest.resultFile : null,
            activeOperationType: "task_execution",
          },
        },
      });
      const decision = routeDecisionSchema.parse({
        schemaVersion: "1.0",
        decisionId: createId("continue"),
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        status: "ready",
        source: {
          command: "continue",
          type: nextAction.source ?? "task_plan_run",
          ref: nextAction.ref ?? nextTask.executionRequestPath,
          validated: true,
        },
        nextAction: {
          type: nextAction.type,
          targetNode: nextAction.targetNode ?? "task_execution",
          reason: nextAction.reason ?? "READY_TASK_AVAILABLE",
          refs: {
            ...(nextAction.refs ?? {}),
            taskId: nextTask.task?.taskId ?? null,
            executionRequestRef: nextTask.executionRequestPath,
          },
        },
        instruction: withAutoRunnableTransition({
          ...nextTask.instruction,
          routingRule: "loom continue has already created the next TaskExecutionRequest. Execute this request now; do not run next-task or loom continue before submitting its TaskResult.",
          userMessage: "Next TaskExecutionRequest is already created; execute it now.",
        }, {
          sourceCommand: "continue",
          sourceSummary: "loom continue created the next TaskExecutionRequest.",
          primaryAction: "execute_materialized_next_task",
          mustStartImmediately: true,
        }),
        materialization: {
          attempted: true,
          status: "created",
          requestRef: nextTask.executionRequestPath,
          candidateFile: typeof nextTask.executionRequest?.resultFile === "string" ? nextTask.executionRequest.resultFile : null,
          leaseRef: toProjectRelative(root, operationLeasePath(root, locator.deliveryId)),
        },
        concurrency: {
          leaseChecked: true,
          blockedByFreshLease: false,
          staleLeaseRecovered: input.staleLeaseRecovered ?? false,
          activeOperation: null,
        },
        ...(input.transition ? { transition: input.transition } : {}),
        createdAt: now,
      });
      await writeJsonAtomic(continueDecisionLatestPath(root, locator), decision);
      return decision;
    }
  }
  const baseInstruction = await instructionFor(root, delivery, locator.deliveryId, locator.phaseId, nextAction.type, input.commandSurface);
  const instruction = await instructionWithPhaseTransitionAdvisories(root, input.transition, nextAction.type, baseInstruction);
  const decision = routeDecisionSchema.parse({
    schemaVersion: "1.0",
    decisionId: createId("continue"),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: statusForAction(nextAction.type),
    source: {
      command: "continue",
      type: nextAction.source ?? "delivery_index",
      ref: nextAction.ref ?? null,
      validated: true,
    },
    nextAction: {
      type: nextAction.type,
      targetNode: nextAction.targetNode ?? targetNodeFor(nextAction.type),
      reason: nextAction.reason ?? "NEXT_ACTION_FROM_DELIVERY_INDEX",
      refs: nextAction.refs,
    },
    instruction,
    materialization: {
      attempted: false,
      status: ["needs_user_decision", "manual_review"].includes(nextAction.type) ? "skipped_user_gated" : "not_applicable",
      requestRef: null,
      candidateFile: null,
      leaseRef: null,
    },
    concurrency: {
      leaseChecked: true,
      blockedByFreshLease: false,
      staleLeaseRecovered: input.staleLeaseRecovered ?? false,
      activeOperation: null,
    },
    ...(input.transition ? { transition: input.transition } : {}),
    createdAt: now,
  });
  await writeJsonAtomic(continueDecisionLatestPath(root, locator), decision);
  return decision;
}

async function instructionWithPhaseTransitionAdvisories(
  projectRoot: string,
  transition: Record<string, unknown> | undefined,
  actionType: string,
  instruction: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  if (
    transition?.type !== "phase_activated" ||
    (actionType !== "brainstorm_clarification" && actionType !== "brainstorm_confirmation") ||
    instruction.mode !== "ask_user" ||
    !(await isGitRepo(projectRoot))
  ) {
    return instruction;
  }
  const fromPhaseId = typeof transition.fromPhaseId === "string" ? transition.fromPhaseId : "previous phase";
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

async function readActiveLease(projectRoot: string, deliveryId: string): Promise<OperationLease | null> {
  const lease = await readOperationLease(projectRoot, deliveryId);
  if (!lease) {
    return null;
  }
  return lease.status === "active" ? lease : null;
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

async function instructionFor(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  deliveryId: string,
  phaseId: string,
  actionType: string,
  commandSurface: LoomCommandSurface | undefined,
): Promise<Record<string, unknown>> {
  const commandByAction: Record<string, string[]> = {
    technical_baseline_request: ["technical-baseline", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    repository_context_request: ["repository-context", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    planning_contract_create: ["planning-contract", "create", "--delivery-id", deliveryId, "--phase-id", phaseId],
    architecture_artifact_contract: ["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    taskplan_generation: ["task-plan", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    continue_execution: ["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId],
    review: ["review", "--delivery-id", deliveryId, "--phase-id", phaseId],
    execution_repair: ["repair", "request", "--type", "execution", "--delivery-id", deliveryId, "--phase-id", phaseId],
    task_result_repair: ["repair", "request", "--type", "task-result", "--delivery-id", deliveryId, "--phase-id", phaseId],
    taskplan_repair: ["repair", "request", "--type", "taskplan", "--delivery-id", deliveryId, "--phase-id", phaseId],
    architecture_artifact_repair: ["repair", "request", "--type", "architecture", "--delivery-id", deliveryId, "--phase-id", phaseId],
  };
  if (actionType === "brainstorm_clarification" || actionType === "brainstorm_confirmation") {
    const brainstormRequest = await latestBrainstormRequestForPhase(projectRoot, delivery, phaseId);
    return {
      mode: "ask_user",
      ...brainstormAskUserInstructionPolicy(),
      requestRef: brainstormRequest?.requestRef ?? null,
      userMessage: actionType === "brainstorm_clarification"
        ? "当前需求需要由 Agent 继续澄清；只有 final_summary 被用户明确确认后，才写入并提交 BrainstormCandidate。"
        : "当前阶段范围需要由 Agent 总结并等待用户确认；确认 phase_scope 后继续后续 Brainstorm 块，只有 final_summary 被确认后才提交 BrainstormCandidate。",
      expectedResponse: {
        kind: "brainstorm_progressive_clarification",
        rule: "Agent manages the conversation. Before presenting confirmation or writing BrainstormCandidate, read requestRef through agentAction.read.fieldGroups inspect commands. For phase_scope, concept_grounding, and frontend_experience confirmations, continue to the next Brainstorm block in chat. Read the candidate write contract, write BrainstormCandidate, and run submitCommand only after the dedicated final_summary block is explicitly confirmed.",
        requestReadRule: brainstormAskUserReadStep,
        currentTurnAnswerRule: {
          consumeCurrentUserMessage: true,
          meaning: "If the same user message that invoked @loom also contains a clear answer, option selection, or confirmation, treat it as the user response for this gate.",
          explicitConfirmationExamples: [
            "按当前路线图确认",
            "确认，按上述范围继续",
            "确认按此范围提交",
            "confirm",
          ],
          doNotAskAgainWhenCurrentMessageIsExplicit: true,
          ifAmbiguousAskUser: true,
        },
        requestRef: brainstormRequest?.requestRef ?? null,
      },
    };
  }
  if (actionType === "needs_user_decision" || actionType === "manual_review") {
    const manualReview = actionType === "manual_review"
      ? await latestManualReviewInstruction(projectRoot, { deliveryId, phaseId })
      : null;
    return {
      mode: "ask_user",
      userMessage: actionType === "manual_review"
        ? manualReviewUserMessage(manualReview)
        : "当前需要用户做一个产品或技术决策。",
      ...(manualReview
        ? {
            requestRef: manualReview.requestRef,
            candidateFile: manualReview.candidateFile,
            submitCommand: manualReview.submitCommand,
            acceptedShortReplies: manualReview.acceptedShortReplies,
            instruction: manualReview.instruction,
          }
        : {}),
    };
  }
  if (actionType === "done") {
    return {
      mode: "report_done",
      userMessage: completedDeliveryUserMessage(commandSurface),
    };
  }
  if (actionType === "continue_to_next_phase") {
    const currentIndex = delivery.phases.findIndex((phase) => phase.phaseId === phaseId);
    const nextPhase = currentIndex >= 0 ? delivery.phases[currentIndex + 1] : undefined;
    if (!nextPhase) {
      return {
        mode: "report_done",
        userMessage: completedDeliveryUserMessage(commandSurface),
      };
    }
    return withAutoRunnableTransition({
      mode: "run_cli",
      command: {
        name: "continue_to_next_phase",
        argv: ["continue"],
      },
      userMessage: `当前阶段已完成。下一步会激活 ${nextPhase.name} 并继续需求确认或规划。`,
      nextPhase: {
        phaseId: nextPhase.phaseId,
        name: nextPhase.name,
      },
    }, {
      sourceCommand: "continue",
      sourceSummary: `Current phase completed and ${nextPhase.name} is ready to activate.`,
      primaryAction: "continue_to_next_phase",
    });
  }
  return withAutoRunnableTransition({
    mode: "run_cli",
    routingRule: "Run this command now. After it finishes, follow the command response instruction or run loom continue again. Do not stop to ask the user unless the next response is user-gated, blocked, done, or failed.",
    command: {
      name: actionType,
      argv: commandByAction[actionType] ?? ["continue"],
    },
    userMessage: "可以继续执行下一步。",
  }, {
    sourceCommand: "continue",
    sourceSummary: "A route action is ready.",
    primaryAction: "run_next_loom_command",
  });
}

function manualReviewUserMessage(manualReview: Awaited<ReturnType<typeof latestManualReviewInstruction>>): string {
  const instruction = manualReview?.instruction;
  if (
    instruction &&
    typeof instruction === "object" &&
    !Array.isArray(instruction) &&
    typeof (instruction as { message?: unknown }).message === "string"
  ) {
    const message = (instruction as { message: string }).message;
    const userPrompt = typeof (instruction as { userPrompt?: unknown }).userPrompt === "string"
      ? (instruction as { userPrompt: string }).userPrompt
      : null;
    const replies = Array.isArray((instruction as { acceptedShortReplies?: unknown }).acceptedShortReplies)
      ? (instruction as { acceptedShortReplies: Array<unknown> }).acceptedShortReplies
          .filter((item): item is Record<string, unknown> => isRecord(item))
          .map((item) => {
            const text = typeof item.text === "string" ? item.text : null;
            const effect = typeof item.effect === "string" ? item.effect : null;
            return text && effect ? `${text}：${effect}` : text;
          })
          .filter((item): item is string => Boolean(item))
      : [];
    return [
      message,
      userPrompt,
      replies.length > 0 ? `可回复选项：${replies.join("；")}` : null,
    ].filter((item): item is string => Boolean(item)).join("\n");
  }
  return "当前需要人工 review 结论。";
}

async function recoverableGenerationInstruction(
  projectRoot: string,
  lease: OperationLease,
): Promise<{
  nextActionType: string;
  requestRef: string | null;
  candidateFile: string | null;
  recoveryReason: string;
  instruction: Record<string, unknown>;
  possibleRuntimeForegroundStall?: Record<string, unknown>;
} | null> {
  const requestRef = typeof lease.refs.requestRef === "string" ? lease.refs.requestRef : null;
  if (!requestRef) {
    return null;
  }
  const requestPath = path.join(projectRoot, requestRef);
  if (!(await pathExists(requestPath))) {
    return null;
  }
  const request = await hydrateRequestManifest(projectRoot, requestPath);
  if (lease.operationType === "architecture_generation") {
    const outputContract = typeof (request as { outputContract?: unknown }).outputContract === "object" && (request as { outputContract?: unknown }).outputContract !== null
      ? (request as { outputContract: { sectionOutputs?: unknown } }).outputContract
      : null;
    const sectionOutputs = Array.isArray(outputContract?.sectionOutputs)
      ? outputContract.sectionOutputs as Array<{ section?: unknown; schemaRef?: unknown; candidateFile?: unknown; schemaShape?: unknown; enumRefs?: unknown; generationRules?: unknown }>
      : Array.isArray(lease.refs.sectionOutputs)
        ? lease.refs.sectionOutputs as Array<{ section?: unknown; schemaRef?: unknown; candidateFile?: unknown; schemaShape?: unknown; enumRefs?: unknown; generationRules?: unknown }>
        : [];
    const progress = await architectureSectionProgress(projectRoot, sectionOutputs);
    const allSectionsExist = progress.total > 0 && progress.missing.length === 0;
    if (allSectionsExist) {
      return {
        nextActionType: "architecture_artifact_contract",
        requestRef,
        candidateFile: null,
        recoveryReason: "architecture section candidate files are present but not submitted",
        instruction: submitExistingCandidateInstruction({
          candidateKind: "ArchitectureSections",
          requestRef,
          agentAction: agentActionFromRequest(request),
          submitCommand: (request as { submitCommand?: unknown }).submitCommand ?? null,
          existingOutputs: {
            progressSignal: "candidate_files",
            sections: progress.files,
          },
          mustNotRunCommandsBeforeSubmit: ["architecture request", "task-plan request", "technical-baseline request", "repository-context request", "loom continue"],
          userMessage: "Architecture section candidate files already exist. Submit the existing files with the request submitCommand before continuing.",
        }),
      };
    }
    const targetSection = progress.missing[0] ?? null;
    const progressTargetOutput = progress.files.find((file) => file.section === targetSection) ?? null;
    const contractTargetOutput = sectionOutputs.find((file) => file.section === targetSection) ?? null;
    const targetOutput = contractTargetOutput
      ? { ...contractTargetOutput, candidateFile: progressTargetOutput?.candidateFile ?? contractTargetOutput.candidateFile }
      : progressTargetOutput;
    const targetFile = typeof targetOutput?.candidateFile === "string" ? targetOutput.candidateFile : null;
    const currentTarget = architectureSingleSectionWriteTarget(targetOutput);
    const requestWithCurrentTarget = await syncRequestAgentActionCurrentTarget(projectRoot, requestRef, request, currentTarget);
    return {
      nextActionType: "architecture_artifact_contract",
      requestRef,
      candidateFile: targetFile,
      recoveryReason: progress.completed.length > 0
        ? "architecture section generation is partially complete"
        : "architecture section candidate files are not present",
      instruction: withAutoRunnableTransition({
        mode: "generate_candidate",
        ...artifactInstructionPolicy(),
        candidateKind: "ArchitectureSections",
        requestRef,
        completionBarrier: architectureSingleSectionCompletionBarrier(targetFile),
        outputSummary: {
          progressSignal: "candidate_files",
          sections: progress.files,
          completedSections: progress.completed,
          missingSections: progress.missing,
        },
        agentAction: agentActionFromRequest(requestWithCurrentTarget),
        sectionGenerationMode: "single_section",
        targetSection,
        targetCandidateFile: targetFile,
        blockedOutput: (request as { blockedOutput?: unknown }).blockedOutput ?? null,
        submitCommand: (request as { submitCommand?: unknown }).submitCommand ?? null,
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: ["architecture request", "task-plan request", "technical-baseline request", "repository-context request"],
        generationSteps: [
          "Read requestRef.",
          "Use agentAction.write.currentTarget.schemaShape, currentTarget.enumRefs, allowedRefs, fieldAccessHints, and generationProtocol as the current section contract.",
          "Do not probe guessed jq paths; if a lookup returns null, use fieldAccessHints and agentAction.write.sectionOutputs.",
          "Write only targetSection to targetCandidateFile unless that file already exists and is complete.",
          "After targetCandidateFile exists, immediately run loom continue as the next action so the CLI can scan file progress and return the next missing section or submit_existing_candidate.",
          "Do not send a progress summary or ask whether to continue between writing targetCandidateFile and running loom continue.",
          "Do not run submitCommand until loom continue returns submit_existing_candidate or all section files exist.",
        ],
        routingRule: "Resume this active generation request by generating only targetSection. The request already exists at requestRef; do not create a new request. Run loom continue immediately after the target file is written; writing one section is not a stop condition.",
        userMessage: targetSection
          ? `Architecture generation request is active. Generate only the missing ${targetSection} section, then run loom continue.`
          : "Architecture generation request is active but no missing section could be selected. Run loom status and inspect the request.",
      }, {
        sourceCommand: "continue",
        sourceSummary: "Recovered an active ArchitectureSections generation request.",
        primaryAction: "generate_missing_architecture_section",
        completionCondition: architectureSingleSectionCompletionCondition,
        requiredSteps: architectureSingleSectionRequiredSteps(),
      }),
    };
  }
  if (lease.operationType === "taskplan_generation") {
    const outputContract = (request as { outputContract?: { outlineFile?: unknown } }).outputContract;
    const outlineFile = typeof outputContract?.outlineFile === "string" ? outputContract.outlineFile : null;
    const progress = await taskPlanGroupedProgress(projectRoot, request);
    if (progress.complete) {
      return {
        nextActionType: "taskplan_generation",
        requestRef,
        candidateFile: outlineFile,
        recoveryReason: "taskplan grouped output files are present but not submitted",
        instruction: submitExistingCandidateInstruction({
          candidateKind: "TaskPlanGroupedOutputs",
          requestRef,
          candidateFile: outlineFile,
          agentAction: agentActionFromRequest(request),
          submitCommand: submitCommandFromRequest(request),
          existingOutputs: {
            ...taskPlanOutputSummary((request as { outputContract?: unknown }).outputContract),
            progress,
          },
          mustNotRunCommandsBeforeSubmit: ["task-plan request", "architecture request", "technical-baseline request", "repository-context request", "loom continue"],
          userMessage: "TaskPlan grouped output files already exist. Submit the existing outputs with the request submitCommand before continuing.",
        }),
      };
    }
    return {
      nextActionType: "taskplan_generation",
      requestRef,
      candidateFile: outlineFile,
      recoveryReason: progress.outline.status === "written"
        ? "taskplan grouped output generation is partially complete"
        : "taskplan grouped output files are not present",
      instruction: withAutoRunnableTransition({
        mode: "generate_candidate",
        ...artifactInstructionPolicy(),
        candidateKind: "TaskPlanGroupedOutputs",
        requestRef,
        outputSummary: {
          ...taskPlanOutputSummary((request as { outputContract?: unknown }).outputContract),
          progressSignal: "candidate_files",
          progress,
        },
        agentAction: agentActionFromRequest(request),
        blockedOutput: (request as { blockedOutput?: unknown }).blockedOutput ?? null,
        submitCommand: submitCommandFromRequest(request),
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: ["task-plan request", "architecture request", "technical-baseline request", "repository-context request", "loom continue"],
        generationSteps: [
          "Read requestRef.",
          "This TaskPlanGenerationRequest already exists; do not summarize request creation or ask whether to continue before generating the grouped outputs.",
          "Use the request's outputContract, schemaShape, enumRefs, and allowedRefs.",
          "If outlineFile is missing, write outlineFile first. If outlineFile exists, keep it and write only the missing group candidate files listed in outputSummary.progress.missingGroups unless an existing file is invalid or incomplete.",
          "Run the request's submitCommand after grouped outputs exist.",
          "Follow the submit command response instruction or run loom continue after submit succeeds.",
        ],
        routingRule: "Resume this active TaskPlanGenerationRequest. The request already exists at requestRef; do not create a new request and do not stop after request creation. Generate the missing grouped TaskPlan files and submit them before any continue call.",
        userMessage: "TaskPlan generation request is active but incomplete. Resume by generating grouped TaskPlan candidate files and submitting them.",
      }, {
        sourceCommand: "continue",
        sourceSummary: "Recovered an active TaskPlanGenerationRequest.",
        primaryAction: "generate_taskplan_grouped_outputs_and_submit",
      }),
    };
  }
  if (lease.operationType === "technical_baseline_generation" || lease.operationType === "repository_context_generation") {
    const candidateFile = typeof lease.refs.candidateFile === "string"
      ? lease.refs.candidateFile
      : candidateFileFromRequest(request);
    if (candidateFile && await pathExists(path.join(projectRoot, candidateFile))) {
      const isRepositoryContext = lease.operationType === "repository_context_generation";
      return {
        nextActionType: isRepositoryContext ? "repository_context_request" : "technical_baseline_request",
        requestRef,
        candidateFile,
        recoveryReason: `${lease.operationType} candidate is present but not submitted`,
        instruction: submitExistingCandidateInstruction({
          candidateKind: isRepositoryContext ? "RepositoryContext" : "TechnicalBaseline",
          requestRef,
          candidateFile,
          agentAction: agentActionFromRequest(request),
          submitCommand: submitCommandFromRequest(request),
          mustNotRunCommandsBeforeSubmit: [
            isRepositoryContext ? "repository-context request" : "technical-baseline request",
            "architecture request",
            "task-plan request",
            "loom continue",
          ],
          userMessage: "Candidate JSON already exists. Submit the existing candidate with the request submitCommand before continuing.",
        }),
      };
    }
    const isRepositoryContext = lease.operationType === "repository_context_generation";
    return {
      nextActionType: isRepositoryContext ? "repository_context_request" : "technical_baseline_request",
      requestRef,
      candidateFile,
      recoveryReason: `${lease.operationType} candidate is not present`,
      instruction: withAutoRunnableTransition({
        mode: "generate_candidate",
        ...artifactInstructionPolicy(),
        candidateKind: isRepositoryContext ? "RepositoryContext" : "TechnicalBaseline",
        requestRef,
        candidateFile,
        agentAction: agentActionFromRequest(request),
        blockedOutput: (request as { blockedOutput?: unknown }).blockedOutput ?? null,
        submitCommand: submitCommandFromRequest(request),
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: [
          isRepositoryContext ? "repository-context request" : "technical-baseline request",
          "architecture request",
          "task-plan request",
          "loom continue",
        ],
        generationSteps: [
          "Read requestRef.",
          "Use the request's output/outputContract schemaShape, enumRefs, and generationProtocol.",
          "Write the requested candidateFile.",
          "Run the request's submitCommand after candidateFile exists.",
          "Follow the submit command response instruction or run loom continue after submit succeeds.",
        ],
        routingRule: "Resume this active generation request. The request already exists at requestRef; do not create a new request. Generate the missing candidate JSON and submit it before any continue call.",
        userMessage: "Generation request is active but incomplete. Resume by generating the candidate JSON and submitting it.",
      }, {
        sourceCommand: "continue",
        sourceSummary: `Recovered an active ${lease.operationType} request.`,
        primaryAction: "generate_candidate_and_submit",
      }),
    };
  }
  if (lease.operationType === "task_execution") {
    const resultFile = typeof lease.refs.resultFile === "string" ? lease.refs.resultFile : resultFileFromRequest(request);
    if (resultFile && await pathExists(path.join(projectRoot, resultFile))) {
      return {
        nextActionType: "continue_execution",
        requestRef,
        candidateFile: resultFile,
        recoveryReason: "task execution result is present but not submitted",
        instruction: submitExistingCandidateInstruction({
          candidateKind: "TaskResult",
          requestRef,
          candidateFile: resultFile,
          agentAction: agentActionFromRequest(request),
          submitCommand: submitCommandFromRequest(request),
          mustNotRunCommandsBeforeSubmit: ["next-task", "review", "repair request", "loom continue"],
          userMessage: "TaskResult file already exists. Submit it with record-result before continuing.",
        }),
      };
    }
    const materialized = await getNextTask({
      projectRoot,
      deliveryId: lease.deliveryId,
      phaseId: lease.phaseId,
    });
    if (materialized.hasTask && materialized.instruction) {
      const runtimeStall = await possibleRuntimeForegroundStall({
        projectRoot,
        lease,
        request,
        resultFile: typeof materialized.executionRequest?.resultFile === "string"
          ? materialized.executionRequest.resultFile
          : resultFile,
      });
      return {
        nextActionType: "continue_execution",
        requestRef: materialized.executionRequestPath ?? requestRef,
        candidateFile: typeof materialized.executionRequest?.resultFile === "string"
          ? materialized.executionRequest.resultFile
          : resultFile,
        recoveryReason: "task execution request is active and result is not present",
        instruction: withAutoRunnableTransition({
          ...materialized.instruction,
          ...(runtimeStall ? { possibleRuntimeForegroundStall: runtimeStall } : {}),
          recovery: true,
          requestAlreadyExists: true,
          routingRule: runtimeStall
            ? "Resume this active task execution request. The request already exists at requestRef. Do not create another task request, do not restart planning, and do not wait for a ready runtime/server process to exit naturally. Finish the probe/cleanup decision, write TaskResult, and submit it before any continue call."
            : "Resume this active task execution request. The request already exists at requestRef; do not create another task request and do not stop with a progress summary. Finish the task result and submit it before any continue call.",
          userMessage: runtimeStall
            ? "当前不是权限问题。任务结果尚未提交，可能停在不会自动退出的本地服务/预览进程上。继续当前任务，完成验证、记录 cleanup 状态并提交 TaskResult。"
            : "Task execution request is active but incomplete. Resume by writing and submitting the TaskResult; do not stop with an interim progress summary.",
        }, {
          sourceCommand: "continue",
          sourceSummary: "Recovered an active TaskExecutionRequest.",
          primaryAction: "resume_current_task",
          mustStartImmediately: true,
        }),
        ...(runtimeStall ? { possibleRuntimeForegroundStall: runtimeStall } : {}),
      };
    }
    const runtimeStall = await possibleRuntimeForegroundStall({ projectRoot, lease, request, resultFile });
    return {
      nextActionType: "continue_execution",
      requestRef,
      candidateFile: resultFile,
      recoveryReason: "task execution request is active and result is not present",
      instruction: withAutoRunnableTransition({
        mode: "execute_task",
        ...taskExecutionOutputPolicy(),
        requestRef,
        resultFile,
        agentAction: agentActionFromRequest(request),
        submitCommand: submitCommandFromRequest(request),
        completionBarrier: {
          resultFile,
          submitCommand: submitCommandFromRequest(request),
          rules: taskExecutionCompletionBarrierRules,
        },
        runtimeCommandGuard: runtimeCommandGuardFromExecutionRequest(request),
        ...(runtimeStall ? { possibleRuntimeForegroundStall: runtimeStall } : {}),
        completionContinuityRequirement: taskExecutionCompletionContinuityRequirement(),
        task: taskSummaryFromExecutionRequest(request),
        candidateKind: "TaskResult",
        candidateFile: resultFile,
        blockedOutput: (request as { blockedOutput?: unknown }).blockedOutput ?? null,
        stopAfterCommand: false,
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: ["next-task", "review", "repair request", "loom continue"],
        primaryAction: {
          action: "resume_current_task",
          requestRef,
          resultFile,
          submitCommand: submitCommandFromRequest(request),
          rule: runtimeStall
            ? "Finish this active TaskExecutionRequest now. If a ready runtime/server is waiting in the foreground, probe it, stop only task-owned runtime when safe, record cleanup state, and submit TaskResult. Do not replace execution with a recovery prompt."
            : "Finish this active TaskExecutionRequest now. Do not replace execution with a recovery prompt.",
        },
        completionCondition: {
          completeWhen: "TaskResult exists at resultFile and submitCommand has succeeded.",
          afterSubmit: "Follow returned data.instruction immediately when auto-runnable.",
          stopOnlyWhen: [
            "request cannot be read",
            "submitCommand returns non-repairable failure",
            "returned instruction is user-gated, blocked, done, manual_review, or needs_user_decision",
          ],
        },
        mustNotDuringPrimaryAction: [
          "Do not replace this active task with a recovery prompt while tool calls are still available.",
          "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
          "Do not run any routing command before submitting this TaskResult.",
          "Do not let any agent-chosen verification method prevent TaskResult and submitCommand closeout.",
          ...verificationCommandSchedulingRules.slice(0, 4),
          ...controlledRuntimeProbeRules,
        ],
        executionSteps: [
          "Read requestRef.",
          compactContextReadStep,
          "Execute or finish the task described by the existing TaskExecutionRequest.",
          "Treat completionBarrier as mandatory: progress-only summaries are not completion, and the task stays running until resultFile exists and submitCommand succeeds.",
          "Keep chat output compact: do not paste source diffs, large patches, full source files, or TaskResult JSON.",
          "Avoid apply_patch or any chat-visible patch workflow for source edits when it would paste a large source patch into chat; use quiet file editing and report changed paths instead.",
          "Run appropriate verification for the task, but obey verificationCommandSchedulingRules: write-producing verification commands must be run one at a time, each in its own completed tool call.",
          "Do not run long-lived runtime/server commands as foreground verification commands.",
          "If you start a temporary runtime/probe server for verification, stop only that task-owned runtime before writing TaskResult and record runtimeDeliveryEvidence.runtimeProbeCleanup when runtimeDeliveryEvidence applies.",
          "If a runtime/server command is already running in the foreground and has shown a ready URL, listening port, or health-ready signal, do not wait for it to exit. Use that ready target for verification, stop only task-owned runtime when safe, then submit TaskResult.",
          "Record executionContinuity in TaskResult. If any agent-owned long-running work, browser session, interactive tool, server, watcher, or worker may still be unreleased, do not claim pure completed; use completed_with_notes with notes unless an independent failure or blocked condition remains.",
          "Write the requested TaskResult file even when the task ends failed or blocked.",
          "If verification remains failed after allowed self-repair, write a failed or blocked TaskResult and run submitCommand; do not stop in chat to ask the user whether to continue.",
          "Run the request's submitCommand after resultFile exists.",
          "Follow the submit command response instruction or run loom continue after submit succeeds.",
        ],
        verificationCommandSchedulingRules,
        stopConditions: [
          "request cannot be read",
          "task returns blocked",
          "task returns failed after allowed self-repair",
          "submitCommand fails and does not return a repairInstruction",
          "returned instruction is ask_user, report_blocked, report_done, manual_review, or needs_user_decision",
        ],
        routingRule: "Resume this active task execution request. The request already exists at requestRef; do not create another task request and do not stop with a progress summary. Finish the task result and submit it before any continue call.",
        userMessage: runtimeStall
          ? "当前不是权限问题。任务结果尚未提交，可能停在不会自动退出的本地服务/预览进程上。继续当前任务，完成验证、记录 cleanup 状态并提交 TaskResult。"
          : "Task execution request is active but incomplete. Resume by writing and submitting the TaskResult; do not stop with an interim progress summary.",
      }, {
        sourceCommand: "continue",
        sourceSummary: "Recovered an active TaskExecutionRequest.",
        primaryAction: "resume_current_task",
        mustStartImmediately: true,
      }),
      ...(runtimeStall ? { possibleRuntimeForegroundStall: runtimeStall } : {}),
    };
  }
  if (lease.operationType === "execution_repair") {
    const targetTaskIds = targetTaskIdsFromRepairRequest(request);
    if (targetTaskIds.length === 0) {
      return null;
    }
    await closeOperationLease({
      projectRoot,
      locator: { deliveryId: lease.deliveryId, phaseId: lease.phaseId },
      operationType: "execution_repair",
      reason: "execution_repair_recovered_to_task_execution",
    });
    const materialized = await materializeExecutionRepairTask({
      projectRoot,
      locator: { deliveryId: lease.deliveryId, phaseId: lease.phaseId },
      repairRequestId: repairRequestIdFromRequest(request),
      targetTaskIds,
    });
    if (!materialized.hasTask || !materialized.instruction) {
      return null;
    }
    return {
      nextActionType: "continue_execution",
      requestRef: materialized.executionRequestPath,
      candidateFile: typeof materialized.executionRequest?.resultFile === "string"
        ? materialized.executionRequest.resultFile
        : null,
      recoveryReason: "execution repair request reopened target task",
      instruction: withAutoRunnableTransition({
        ...materialized.instruction,
        routingRule: "Execution repair has reopened the target task and created a TaskExecutionRequest. Execute it now; do not run repair request or loom continue before submitting its TaskResult.",
        userMessage: "Execution repair is ready. Execute the reopened task request now.",
      }, {
        sourceCommand: "continue",
        sourceSummary: "Execution repair recovery reopened the target task and created a TaskExecutionRequest.",
        primaryAction: "execute_reopened_task_request",
        mustStartImmediately: true,
      }),
    };
  }
  if (
    lease.operationType === "task_result_repair" ||
    lease.operationType === "taskplan_repair" ||
    lease.operationType === "architecture_artifact_repair"
  ) {
    const candidateFile = typeof lease.refs.candidateFile === "string"
      ? lease.refs.candidateFile
      : candidateFileFromRequest(request);
    const submitCommand = repairSubmitCommandFromRequest(request, candidateFile);
    if (candidateFile && await pathExists(path.join(projectRoot, candidateFile))) {
      return {
        nextActionType: lease.operationType,
        requestRef,
        candidateFile,
        recoveryReason: `${lease.operationType} candidate is present but not submitted`,
        instruction: submitExistingCandidateInstruction({
          candidateKind: repairCandidateKindForOperation(lease.operationType),
          requestRef,
          candidateFile,
          agentAction: agentActionFromRequest(request),
          submitCommand,
          mustNotRunCommandsBeforeSubmit: ["repair request", "review", "next-task", "architecture request", "task-plan request", "loom continue"],
          userMessage: "Repair candidate already exists. Submit it with the repair request submitCommand before continuing.",
        }),
      };
    }
    return {
      nextActionType: lease.operationType,
      requestRef,
      candidateFile,
      recoveryReason: `${lease.operationType} request is active and candidate is not present`,
      instruction: withAutoRunnableTransition({
        mode: "generate_candidate",
        ...artifactInstructionPolicy(),
        candidateKind: repairCandidateKindForOperation(lease.operationType),
        requestRef,
        candidateFile,
        agentAction: agentActionFromRequest(request),
        outputContract: (request as { outputContract?: unknown }).outputContract ?? null,
        blockedOutput: null,
        submitCommand,
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: ["repair request", "review", "next-task", "architecture request", "task-plan request", "loom continue"],
        generationSteps: [
          "Read requestRef.",
          compactContextReadStep,
          "Use repairRules, enumRefs, outputContract, and resumePolicy from the RepairRequest.",
          "Write only the requested repair candidate/result file(s); do not modify unrelated project files.",
          "If submitCommand.argv is empty, stop and report that the original generation request id/ref is missing; do not guess requestId.",
          "Run submitCommand after the repair candidate/result exists.",
          "Follow the submit command response instruction immediately when it is auto-runnable.",
        ],
        routingRule: "Resume this active RepairRequest. The request already exists at requestRef; do not create another repair request. Generate and submit the repair candidate before any continue call.",
        userMessage: "Repair request is active but incomplete. Resume by writing and submitting the repair candidate.",
      }, {
        sourceCommand: "continue",
        sourceSummary: `Recovered an active ${lease.operationType} request.`,
        primaryAction: "generate_repair_artifact_and_submit",
      }),
    };
  }
  if (lease.operationType === "review_generation") {
    const resultFile = typeof lease.refs.resultFile === "string" ? lease.refs.resultFile : resultFileFromRequest(request);
    if (resultFile && await pathExists(path.join(projectRoot, resultFile))) {
      return {
        nextActionType: "review",
        requestRef,
        candidateFile: resultFile,
        recoveryReason: "review result is present but not submitted",
        instruction: submitExistingCandidateInstruction({
          candidateKind: "ReviewResult",
          requestRef,
          candidateFile: resultFile,
          agentAction: agentActionFromRequest(request),
          submitCommand: submitCommandFromRequest(request),
          mustNotRunCommandsBeforeSubmit: ["review", "repair request", "loom continue"],
          userMessage: "ReviewResult file already exists. Submit it with review accept before continuing.",
        }),
      };
    }
    return {
      nextActionType: "review",
      requestRef,
      candidateFile: resultFile,
      recoveryReason: "review request is active and result is not present",
      instruction: withAutoRunnableTransition({
        mode: "generate_candidate",
        ...artifactInstructionPolicy(),
        candidateKind: "ReviewResult",
        requestRef,
        candidateFile: resultFile,
        agentAction: agentActionFromRequest(request),
        blockedOutput: null,
        submitCommand: submitCommandFromRequest(request),
        recovery: true,
        requestAlreadyExists: true,
        mustNotRunCommandsBeforeSubmit: ["review", "repair request", "loom continue"],
        generationSteps: [
          "Read requestRef.",
          "Use the ReviewRequest outputContract schemaShape, validatorRules, allowedRefs, and routingRules.",
          "Write the requested ReviewResult file.",
          "Run the request's submitCommand after resultFile exists.",
          "Follow the submit command response instruction or run loom continue after submit succeeds.",
        ],
        routingRule: "Resume this active review request. The request already exists at requestRef; do not create another review request. Generate and submit ReviewResult before any continue call.",
        userMessage: "Review request is active but incomplete. Resume by writing and submitting the ReviewResult.",
      }, {
        sourceCommand: "continue",
        sourceSummary: "Recovered an active ReviewRequest.",
        primaryAction: "generate_review_result_and_submit",
      }),
    };
  }
  return null;
}

function repairCandidateKindForOperation(operationType: OperationLease["operationType"]): string {
  if (operationType === "task_result_repair") {
    return "TaskResult";
  }
  if (operationType === "taskplan_repair") {
    return "TaskPlanGroupedReplacement";
  }
  if (operationType === "architecture_artifact_repair") {
    return "ArchitectureSectionReplacement";
  }
  return "RepairCandidate";
}

async function architectureSectionProgress(
  projectRoot: string,
  outputs: Array<{ section?: unknown; candidateFile?: unknown }>,
): Promise<{
  total: number;
  completed: string[];
  missing: string[];
  files: Array<{ section: string | null; candidateFile: string | null; status: "written" | "missing"; updatedAt: string | null }>;
}> {
  const files = [];
  const completed = [];
  const missing = [];
  for (const output of outputs) {
    const section = typeof output.section === "string" ? output.section : null;
    const candidateFile = typeof output.candidateFile === "string" ? output.candidateFile : null;
    const stat = candidateFile ? await fileStatOrNull(path.join(projectRoot, candidateFile)) : null;
    const status: "written" | "missing" = stat ? "written" : "missing";
    if (section) {
      if (stat) {
        completed.push(section);
      } else {
        missing.push(section);
      }
    }
    files.push({
      section,
      candidateFile,
      status,
      updatedAt: stat?.mtime.toISOString() ?? null,
    });
  }
  return {
    total: outputs.length,
    completed,
    missing,
    files,
  };
}

async function taskPlanGroupedProgress(projectRoot: string, request: unknown): Promise<{
  progressSignal: "candidate_files";
  complete: boolean;
  outline: { candidateFile: string | null; status: "written" | "missing" | "invalid"; updatedAt: string | null };
  groupTotal: number;
  completedGroupCount: number;
  completedGroups: string[];
  missingGroups: string[];
  groups: Array<{ groupId: string; candidateFile: string | null; status: "written" | "missing"; updatedAt: string | null }>;
  recommendedAction: "write_outline" | "repair_outline" | "write_missing_groups" | "submit_accept";
  summary: string;
}> {
  if (typeof request !== "object" || request === null) {
    return {
      progressSignal: "candidate_files",
      complete: false,
      outline: { candidateFile: null, status: "missing", updatedAt: null },
      groupTotal: 0,
      completedGroupCount: 0,
      completedGroups: [],
      missingGroups: [],
      groups: [],
      recommendedAction: "write_outline",
      summary: "TaskPlan outline is missing. Generate outline first, then generate group files.",
    };
  }
  const outputContract = (request as { outputContract?: { outlineFile?: unknown } }).outputContract;
  const outlineFile = typeof outputContract?.outlineFile === "string" ? outputContract.outlineFile : null;
  const outlineStat = outlineFile ? await fileStatOrNull(path.join(projectRoot, outlineFile)) : null;
  if (!outlineFile || !outlineStat) {
    return {
      progressSignal: "candidate_files",
      complete: false,
      outline: { candidateFile: outlineFile, status: "missing", updatedAt: null },
      groupTotal: 0,
      completedGroupCount: 0,
      completedGroups: [],
      missingGroups: [],
      groups: [],
      recommendedAction: "write_outline",
      summary: "TaskPlan outline is missing. Generate outline first, then generate group files.",
    };
  }
  let outline: unknown;
  try {
    outline = await readJsonFile(path.join(projectRoot, outlineFile));
  } catch {
    return {
      progressSignal: "candidate_files",
      complete: false,
      outline: { candidateFile: outlineFile, status: "invalid", updatedAt: outlineStat.mtime.toISOString() },
      groupTotal: 0,
      completedGroupCount: 0,
      completedGroups: [],
      missingGroups: [],
      groups: [],
      recommendedAction: "repair_outline",
      summary: "TaskPlan outline exists but cannot be read as JSON. Repair outline before generating groups.",
    };
  }
  const outlineGroups = typeof outline === "object" && outline !== null && Array.isArray((outline as { groups?: unknown }).groups)
    ? (outline as { groups: Array<{ groupId?: unknown }> }).groups
    : [];
  const groups = [];
  const completedGroups = [];
  const missingGroups = [];
  for (const group of outlineGroups) {
    if (typeof group.groupId !== "string") {
      continue;
    }
    const candidateFile = taskPlanGroupFileFromRequest(request, group.groupId);
    const stat = candidateFile ? await fileStatOrNull(path.join(projectRoot, candidateFile)) : null;
    const status: "written" | "missing" = stat ? "written" : "missing";
    if (stat) {
      completedGroups.push(group.groupId);
    } else {
      missingGroups.push(group.groupId);
    }
    groups.push({
      groupId: group.groupId,
      candidateFile,
      status,
      updatedAt: stat?.mtime.toISOString() ?? null,
    });
  }
  return {
    progressSignal: "candidate_files",
    complete: outlineGroups.length > 0 && missingGroups.length === 0,
    outline: { candidateFile: outlineFile, status: "written", updatedAt: outlineStat.mtime.toISOString() },
    groupTotal: outlineGroups.length,
    completedGroupCount: completedGroups.length,
    completedGroups,
    missingGroups,
    groups,
    recommendedAction: outlineGroups.length > 0 && missingGroups.length === 0 ? "submit_accept" : "write_missing_groups",
    summary: outlineGroups.length > 0 && missingGroups.length === 0
      ? `TaskPlan outline exists and all ${outlineGroups.length} group file(s) are written. Submit task-plan accept.`
      : `TaskPlan outline exists and ${completedGroups.length}/${outlineGroups.length} group file(s) are written. Generate missing groups: ${missingGroups.join(", ") || "unknown"}.`,
  };
}

async function fileStatOrNull(file: string): Promise<import("node:fs").Stats | null> {
  try {
    return await fs.stat(file);
  } catch {
    return null;
  }
}

function taskPlanGroupFileFromRequest(request: unknown, groupId: string): string | null {
  if (typeof request !== "object" || request === null) {
    return null;
  }
  const pattern = (request as { outputContract?: { groupFilePattern?: unknown } }).outputContract?.groupFilePattern;
  return typeof pattern === "string" ? pattern.replace("{groupId}", groupId) : null;
}

function candidateFileFromRequest(request: unknown): string | null {
  if (typeof request !== "object" || request === null) {
    return null;
  }
  const output = (request as { output?: { candidateFile?: unknown }; outputContract?: { candidateFile?: unknown } });
  if (typeof output.output?.candidateFile === "string") {
    return output.output.candidateFile;
  }
  if (typeof output.outputContract?.candidateFile === "string") {
    return output.outputContract.candidateFile;
  }
  return null;
}

function resultFileFromRequest(request: unknown): string | null {
  if (typeof request !== "object" || request === null) {
    return null;
  }
  const output = (request as { resultFile?: unknown; outputContract?: { resultFile?: unknown } });
  if (typeof output.resultFile === "string") {
    return output.resultFile;
  }
  if (typeof output.outputContract?.resultFile === "string") {
    return output.outputContract.resultFile;
  }
  return null;
}

function taskSummaryFromExecutionRequest(request: unknown): Record<string, unknown> {
  const typed = request as {
    source?: { taskId?: unknown; groupId?: unknown };
    task?: {
      taskId?: unknown;
      groupId?: unknown;
      title?: unknown;
      taskKind?: unknown;
      acceptanceRefs?: unknown;
    };
  };
  return {
    taskId: typeof typed.source?.taskId === "string"
      ? typed.source.taskId
      : typeof typed.task?.taskId === "string"
        ? typed.task.taskId
        : null,
    groupId: typeof typed.source?.groupId === "string"
      ? typed.source.groupId
      : typeof typed.task?.groupId === "string"
        ? typed.task.groupId
        : null,
    title: typeof typed.task?.title === "string" ? typed.task.title : null,
    taskKind: typeof typed.task?.taskKind === "string" ? typed.task.taskKind : null,
    acceptanceRefs: Array.isArray(typed.task?.acceptanceRefs)
      ? typed.task.acceptanceRefs.filter((ref): ref is string => typeof ref === "string" && ref.length > 0)
      : [],
  };
}

function targetTaskIdsFromRepairRequest(request: unknown): string[] {
  if (typeof request !== "object" || request === null) {
    return [];
  }
  const inputs = (request as { inputs?: { targetTaskIds?: unknown } }).inputs;
  return Array.isArray(inputs?.targetTaskIds)
    ? inputs.targetTaskIds.filter((taskId): taskId is string => typeof taskId === "string" && taskId.length > 0)
    : [];
}

function repairRequestIdFromRequest(request: unknown): string {
  if (typeof request === "object" && request !== null) {
    const repairRequestId = (request as { repairRequestId?: unknown }).repairRequestId;
    if (typeof repairRequestId === "string" && repairRequestId.length > 0) {
      return repairRequestId;
    }
  }
  return "unknown-repair-request";
}

function submitExistingCandidateInstruction(input: {
  candidateKind: string;
  requestRef: string;
  candidateFile?: string | null;
  agentAction?: unknown;
  submitCommand: unknown;
  existingOutputs?: unknown;
  mustNotRunCommandsBeforeSubmit: string[];
  userMessage: string;
}): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "submit_existing_candidate",
    ...artifactInstructionPolicy(),
    candidateKind: input.candidateKind,
    requestRef: input.requestRef,
    candidateFile: input.candidateFile ?? null,
    agentAction: input.agentAction ?? null,
    existingOutputs: input.existingOutputs ?? null,
    submitCommand: input.submitCommand,
    recovery: true,
    requestAlreadyExists: true,
    candidateAlreadyExists: true,
    mustNotRunCommandsBeforeSubmit: input.mustNotRunCommandsBeforeSubmit,
    generationSteps: [
      "Read requestRef.",
      compactContextReadStep,
      "Do not regenerate or overwrite existing candidate files unless they are invalid or incomplete.",
      "Run the request's submitCommand for the existing candidate/result files.",
      "If submit fails with repairInstruction, repair the same files and resubmit.",
      "Follow the submit command response instruction or run loom continue after submit succeeds.",
    ],
    routingRule: "Resume this active request by submitting the existing candidate/result files. Do not create a new request and do not run loom continue before submit succeeds.",
    userMessage: input.userMessage,
  }, {
    sourceCommand: "continue",
    sourceSummary: "Recovered existing candidate/result files that are ready to submit.",
    primaryAction: "submit_existing_candidate",
  });
}

function agentActionFromRequest(request: unknown): unknown {
  if (!isRecord(request)) {
    return null;
  }
  return request.agentAction ?? null;
}

function runtimeCommandGuardFromExecutionRequest(request: unknown): Record<string, unknown> | undefined {
  if (!isRecord(request)) {
    return undefined;
  }
  const task = request.task;
  if (!isRecord(task)) {
    return undefined;
  }
  const executionRules = request.executionRules;
  const runtimeRules = isRecord(executionRules) && isRecord(executionRules.runtimeDeliveryExecutionRules)
    ? executionRules.runtimeDeliveryExecutionRules
    : null;
  const guardRules = Array.isArray(runtimeRules?.runtimeCommandGuardRules)
    ? runtimeRules.runtimeCommandGuardRules.filter((rule): rule is string => typeof rule === "string" && rule.length > 0)
    : controlledRuntimeProbeRules;
  return {
    appliesWhen: "task.runtimeDeliveryRequirement is present or the task starts a temporary runtime/server/probe process",
    rules: guardRules.length > 0 ? guardRules : controlledRuntimeProbeRules,
  };
}

function submitCommandFromRequest(request: unknown): unknown {
  if (typeof request !== "object" || request === null) {
    return null;
  }
  const typed = request as {
    submitCommand?: unknown;
    output?: { submitCommand?: unknown };
    outputContract?: { submitCommand?: unknown };
  };
  return typed.submitCommand ?? typed.output?.submitCommand ?? typed.outputContract?.submitCommand ?? null;
}

function repairSubmitCommandFromRequest(request: unknown, candidateFile: string | null): unknown {
  if (typeof request !== "object" || request === null) {
    return null;
  }
  const typed = request as {
    repairRequestId?: unknown;
    outputContract?: { submitCommand?: unknown };
  };
  const submitCommand = typed.outputContract?.submitCommand;
  if (!isCommand(submitCommand)) {
    return submitCommand ?? null;
  }
  const repairRequestId = typeof typed.repairRequestId === "string" ? typed.repairRequestId : "{repairRequestId}";
  return {
    ...submitCommand,
    argv: submitCommand.argv.map((part) => {
      if (part === "{candidateFile}") {
        return candidateFile ?? part;
      }
      if (part === "{repairRequestId}") {
        return repairRequestId;
      }
      return part;
    }),
  };
}

function isCommand(value: unknown): value is { name: string; argv: string[] } {
  return typeof value === "object" &&
    value !== null &&
    typeof (value as { name?: unknown }).name === "string" &&
    Array.isArray((value as { argv?: unknown }).argv) &&
    (value as { argv: unknown[] }).argv.every((part) => typeof part === "string");
}

function taskPlanOutputSummary(outputContract: unknown): Record<string, unknown> | null {
  if (typeof outputContract !== "object" || outputContract === null) {
    return null;
  }
  const typed = outputContract as {
    outlineFile?: unknown;
    groupFilePattern?: unknown;
  };
  return {
    outlineFile: typeof typed.outlineFile === "string" ? typed.outlineFile : null,
    groupFilePattern: typeof typed.groupFilePattern === "string" ? typed.groupFilePattern : null,
  };
}

async function latestBrainstormRequestForPhase(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  phaseId: string,
): Promise<{
  requestId: string;
  brainstormRunId: string;
  requestRef: string;
  candidateFile: string;
  submitCommand: Record<string, unknown>;
} | null> {
  const phase = delivery.phases.find((item) => item.phaseId === phaseId);
  const requestRef = phase?.latestRefs.brainstormRequest;
  if (!requestRef) {
    return null;
  }

  const requestPath = path.join(projectRoot, requestRef);
  let request = await pathExists(requestPath)
    ? await hydrateRequestManifest(projectRoot, requestPath)
    : null;
  const requestId = phase?.latestRefs.brainstormRequestId
    ?? stringField(request, "requestId");
  const brainstormRunId = phase?.latestRefs.brainstormRunId
    ?? stringField(request, "brainstormRunId")
    ?? stringField(request, "runId");
  const candidateFile = phase?.latestRefs.brainstormCandidateFile
    ?? candidateFileFromRequest(request);

  if (!requestId || !brainstormRunId || !candidateFile) {
    return null;
  }

  const requestSubmitCommand = submitCommandFromRequest(request);
  const submitCommand = isRecord(requestSubmitCommand)
    ? requestSubmitCommand as { name: string; argv: string[] }
    : {
        name: "brainstorm accept",
        argv: [
          "brainstorm",
          "accept",
          "--delivery-id",
          delivery.deliveryId,
          "--phase-id",
          phaseId,
          "--request-id",
          requestId,
          "--run-id",
          brainstormRunId,
          "--candidate-file",
          candidateFile,
        ],
      };

  if (isRecord(request) && request.requestType === "brainstorm_session" && !isRecord(request.agentAction)) {
    const blockedCandidateFile = isRecord(request.blockedOutput) && typeof request.blockedOutput.candidateFile === "string"
      ? request.blockedOutput.candidateFile
      : null;
    const repairedRequest: Record<string, unknown> = {
      ...request,
      agentAction: brainstormSessionAgentActionContract({
        candidateFile,
        blockedFile: blockedCandidateFile,
        submitCommand,
      }),
    };
    await writeRequestManifestAtomic(projectRoot, requestPath, repairedRequest);
    request = await hydrateRequestManifest(projectRoot, requestPath);
  }

  return {
    requestId,
    brainstormRunId,
    requestRef,
    candidateFile,
    submitCommand,
  };
}

async function readJsonIfPresent(filePath: string): Promise<unknown | null> {
  if (!(await pathExists(filePath))) {
    return null;
  }
  return readJsonFile(filePath);
}

function stringField(value: unknown, key: string): string | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  return typeof value[key] === "string" ? value[key] : undefined;
}

async function syncRequestAgentActionCurrentTarget(
  projectRoot: string,
  requestRef: string,
  hydratedRequest: unknown,
  currentTarget: Record<string, unknown>,
): Promise<unknown> {
  const updatedRequest = withAgentActionCurrentTarget(hydratedRequest, currentTarget);
  const requestFile = path.join(projectRoot, requestRef);
  const rawRequest = await readJsonIfPresent(requestFile);
  if (!isRecord(rawRequest)) {
    return updatedRequest;
  }

  const agentActionRef = requestManifestRef(rawRequest, "agentAction");
  if (agentActionRef) {
    const agentActionFile = path.join(projectRoot, agentActionRef);
    const agentAction = await readJsonIfPresent(agentActionFile);
    if (isRecord(agentAction)) {
      await writeJsonAtomic(agentActionFile, withAgentActionCurrentTarget({ agentAction }, currentTarget).agentAction);
    }
    return updatedRequest;
  }

  if (isRecord(rawRequest.agentAction)) {
    await writeJsonAtomic(requestFile, withAgentActionCurrentTarget(rawRequest, currentTarget));
  }
  return updatedRequest;
}

function requestManifestRef(request: Record<string, unknown>, key: string): string | null {
  const manifest = request.requestManifest;
  const refs = isRecord(manifest) ? manifest.refs : null;
  const entry = isRecord(refs) ? refs[key] : null;
  return isRecord(entry) && typeof entry.ref === "string" ? entry.ref : null;
}

function withAgentActionCurrentTarget(value: unknown, currentTarget: Record<string, unknown>): Record<string, unknown> {
  const request = isRecord(value) ? { ...value } : {};
  const normalizedAgentAction = normalizeAgentActionForFieldGroups(request.agentAction);
  const agentAction = isRecord(normalizedAgentAction) ? { ...normalizedAgentAction } : {};
  const isGenerateSections = agentAction.actionKind === "generate_sections";
  const read = isRecord(agentAction.read) ? { ...agentAction.read } : {};
  const currentTargetField = "agentAction.write.currentTarget";
  const required = isGenerateSections
    ? withoutArchitectureSectionRedundantFields(read.required)
    : Array.isArray(read.required) ? [...read.required] : [];
  if (!required.includes(currentTargetField)) {
    required.unshift(currentTargetField);
  }
  const fieldGroups = isGenerateSections
    ? compactArchitectureSectionFieldGroups(read.fieldGroups)
    : Array.isArray(read.fieldGroups) ? [...read.fieldGroups] : [];
  if (!fieldGroups.some((group) => isRecord(group) && Array.isArray(group.fields) && group.fields.includes(currentTargetField))) {
    fieldGroups.unshift(agentActionCurrentTargetReadGroup());
  }
  const optional = isGenerateSections
    ? withoutArchitectureSectionRedundantFields(read.optional)
    : read.optional;
  const write = isRecord(agentAction.write) ? { ...agentAction.write } : {};
  write.currentTarget = currentTarget;
  read.required = required;
  if (Array.isArray(optional)) {
    read.optional = optional;
  }
  read.fieldGroups = fieldGroups;
  delete read.fields;
  agentAction.read = read;
  agentAction.write = write;
  if (isGenerateSections) {
    const schema = isRecord(agentAction.schema) ? { ...agentAction.schema } : {};
    schema.shapeLocation = "agentAction.write.currentTarget.schemaShape";
    schema.enumLocation = "agentAction.write.currentTarget.enumRefs";
    agentAction.schema = schema;
  }
  request.agentAction = normalizeAgentActionForFieldGroups(agentAction);
  return request;
}

function withoutArchitectureSectionRedundantFields(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string =>
    typeof item === "string" && !isArchitectureSingleSectionRedundantField(item)
  );
}

function compactArchitectureSectionFieldGroups(value: unknown): unknown[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const fields = Array.isArray(group.fields)
        ? group.fields.filter((field): field is string =>
          typeof field === "string" && !isArchitectureSingleSectionRedundantField(field)
        )
        : [];
      return {
        ...group,
        fields,
        readCommand: {
          name: "inspect",
          argv: ["inspect", "--request", "{requestRef}", "--field", [...new Set(fields)].join(",")],
        },
      };
    })
    .filter((group) => group.fields.length > 0);
}

function isArchitectureSingleSectionRedundantField(label: string): boolean {
  const normalized = normalizeReadLabel(label);
  return normalized === "outputContract.sectionOutputs" || normalized === "enumRefs";
}

function normalizeReadLabel(label: string): string {
  return label
    .replace(/\s+when\s+.+$/i, "")
    .replace(/\s+if\s+.+$/i, "")
    .trim();
}

function agentActionCurrentTargetReadGroup(): Record<string, unknown> {
  const field = "agentAction.write.currentTarget";
  return {
    groupId: "agent_action_current_target",
    required: true,
    purpose: "Current active write target selected by the returned instruction.",
    whenToRead: "Before writing an architecture section candidate or deciding whether this step can stop.",
    fields: [field],
    readCommand: {
      name: "inspect",
      argv: ["inspect", "--request", "{requestRef}", "--field", field],
    },
    fallbackRule: "If this grouped inspect read fails, read requestManifest.refs.agentAction.ref and select .write.currentTarget. Do not print full .loom artifacts.",
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function latestManualReviewInstruction(projectRoot: string, locator: { deliveryId: string; phaseId: string }): Promise<{
  requestRef: string;
  candidateFile: string | null;
  submitCommand: unknown;
  acceptedShortReplies: unknown;
  instruction: unknown;
} | null> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  if (!(await pathExists(latestPath))) {
    return null;
  }
  const latest = await readJsonFile(latestPath) as { latestManualReviewRequestRef?: unknown };
  const requestRef = typeof latest.latestManualReviewRequestRef === "string"
    ? latest.latestManualReviewRequestRef
    : null;
  if (!requestRef) {
    return null;
  }
  const requestPath = path.join(projectRoot, requestRef.split("/").join(path.sep));
  if (!(await pathExists(requestPath))) {
    return {
      requestRef,
      candidateFile: null,
      submitCommand: null,
      acceptedShortReplies: null,
      instruction: null,
    };
  }
  const request = await readJsonFile(requestPath) as {
    instruction?: { acceptedShortReplies?: unknown };
    resolutionContract?: { candidateFile?: unknown; submitCommand?: unknown };
  };
  return {
    requestRef,
    candidateFile: typeof request.resolutionContract?.candidateFile === "string"
      ? request.resolutionContract.candidateFile
      : null,
    submitCommand: request.resolutionContract?.submitCommand ?? null,
    acceptedShortReplies: request.instruction?.acceptedShortReplies ?? null,
    instruction: request.instruction ?? null,
  };
}

function statusForAction(actionType: string): RouteDecision["status"] {
  if (actionType === "done") {
    return "done";
  }
  if (isUserGatedActionType(actionType)) {
    return "waiting_user";
  }
  return "ready";
}

function isUserGatedActionType(actionType: string): boolean {
  return actionType === "needs_user_decision" || actionType === "manual_review" || actionType.startsWith("brainstorm_");
}

function targetNodeFor(actionType: string): string {
  return actionType;
}

async function requireInitialized(projectRoot: string): Promise<void> {
  if (!(await pathExists(path.join(projectRoot, ".loom", "config.json")))) {
    throw stateNotInitialized(projectRoot);
  }
}

async function activateNextPhase(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  currentPhaseId: string,
  targetPhaseId?: string,
): Promise<{ deliveryId: string; phaseId: string } | null> {
  const plan = await preflightPhaseActivation(projectRoot, delivery, currentPhaseId, targetPhaseId);
  await commitPhaseActivation(projectRoot, plan);
  return plan.nextPhase ? { deliveryId: plan.delivery.deliveryId, phaseId: plan.nextPhase.phaseId } : null;
}

type PhaseActivationPlan = {
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>;
  currentPhase: DeliveryIndexPhase;
  nextPhase: DeliveryIndexPhase | null;
  now: string;
};

async function preflightPhaseActivation(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  currentPhaseId: string,
  targetPhaseId?: string,
): Promise<PhaseActivationPlan> {
  const currentIndex = delivery.phases.findIndex((phase) => phase.phaseId === currentPhaseId);
  if (currentIndex < 0) {
    throw invalidArgument("Current phase does not exist in DeliveryRun.", {
      deliveryId: delivery.deliveryId,
      phaseId: currentPhaseId,
    });
  }
  const nextPhase = await resolveNextPhaseForActivation(
    projectRoot,
    delivery,
    delivery.phases[currentIndex],
    targetPhaseId,
  );
  return {
    delivery,
    currentPhase: delivery.phases[currentIndex],
    nextPhase,
    now: new Date().toISOString(),
  };
}

async function resolveNextPhaseForActivation(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  currentPhase: DeliveryIndexPhase,
  targetPhaseId?: string,
): Promise<DeliveryIndexPhase | null> {
  if (targetPhaseId) {
    const existing = delivery.phases.find((phase) => phase.phaseId === targetPhaseId);
    if (existing) return existing;
  }
  return nextPhaseFromPreview(projectRoot, delivery, currentPhase, targetPhaseId);
}

async function commitPhaseActivation(projectRoot: string, plan: PhaseActivationPlan): Promise<void> {
  const index = structuredClone(plan.delivery);
  const currentPhase = index.phases.find((phase) => phase.phaseId === plan.currentPhase.phaseId);
  if (!currentPhase) {
    throw invalidArgument("Current phase does not exist in DeliveryRun.", {
      deliveryId: index.deliveryId,
      phaseId: plan.currentPhase.phaseId,
    });
  }
  currentPhase.status = "completed";
  currentPhase.nextAction = plan.nextPhase
    ? null
    : {
        type: "done",
        source: "continue",
        deliveryId: index.deliveryId,
        phaseId: currentPhase.phaseId,
        reason: "ROADMAP_COMPLETED",
      };

  if (!plan.nextPhase) {
    index.status = "completed";
  } else {
    let nextPhase = index.phases.find((phase) => phase.phaseId === plan.nextPhase?.phaseId);
    if (!nextPhase) {
      nextPhase = structuredClone(plan.nextPhase);
      index.phases.push(nextPhase);
    }
    nextPhase.status = "pending";
    nextPhase.nextAction = {
      type: "repository_context_request",
      source: "continue_to_next_phase",
      deliveryId: index.deliveryId,
      phaseId: nextPhase.phaseId,
      ref: nextPhase.latestRefs.brainstormContract ?? null,
      reason: "NEXT_PHASE_REPOSITORY_CONTEXT_REQUIRED_BEFORE_BRAINSTORM",
      targetNode: "repository_context_request",
      refs: {
        nextPhasePreview: nextPhase.nextAction?.refs?.nextPhasePreview ?? null,
      },
    };
    index.activePhaseId = nextPhase.phaseId;
    index.status = "planning";
  }

  index.updatedAt = plan.now;
  await writeJsonAtomic(deliveryIndexPath(projectRoot, index.deliveryId), index);
  await upsertStatusFromIndex(projectRoot, index);
}

async function nextPhaseFromPreview(
  projectRoot: string,
  delivery: Awaited<ReturnType<typeof loadDeliveryIndex>>,
  currentPhase: DeliveryIndexPhase,
  targetPhaseId?: string,
): Promise<DeliveryIndexPhase | null> {
  const file = brainstormContractPath(projectRoot, delivery.deliveryId);
  if (!(await pathExists(file))) return null;
  const contract = brainstormContractSchema.parse(await readJsonFile(file));
  const preview = contract.phasePlan.nextPhasePreview;
  if (preview.kind === "none") return null;
  if (targetPhaseId && preview.suggestedPhaseId !== targetPhaseId) return null;
  const existing = delivery.phases.find((phase) => phase.phaseId === preview.suggestedPhaseId);
  if (existing) return existing;
  return {
    phaseId: preview.suggestedPhaseId,
    name: preview.title,
    status: "pending",
    latestRefs: {
      brainstormContract: toProjectRelative(projectRoot, file),
    },
    nextAction: {
      type: "repository_context_request",
      source: "next_phase_preview",
      deliveryId: delivery.deliveryId,
      phaseId: preview.suggestedPhaseId,
      ref: toProjectRelative(projectRoot, file),
      reason: "NEXT_PHASE_REPOSITORY_CONTEXT_REQUIRED_BEFORE_BRAINSTORM",
      targetNode: "repository_context_request",
      refs: {
        fromPhaseId: currentPhase.phaseId,
        nextPhasePreview: preview,
      },
    },
  };
}

async function upsertStatusFromIndex(projectRoot: string, index: Awaited<ReturnType<typeof loadDeliveryIndex>>): Promise<void> {
  const status = await loadProjectStatus(projectRoot);
  if (status.activeDeliveryId && status.activeDeliveryId !== index.deliveryId) {
    throw invalidArgument("Cannot activate phase for a non-active delivery.", {
      activeDeliveryId: status.activeDeliveryId,
      deliveryId: index.deliveryId,
    });
  }
  await upsertStatusDelivery(projectRoot, index);
}

function defaultNextActionForActivatedPhase(phase: DeliveryIndexPhase, locator: { deliveryId: string; phaseId: string }): RouteAction {
  return {
    type: "repository_context_request",
    source: "continue_to_next_phase",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    ref: phase.latestRefs.brainstormContract ?? null,
    reason: "NEXT_PHASE_REPOSITORY_CONTEXT_REQUIRED_BEFORE_BRAINSTORM",
    targetNode: "repository_context_request",
    refs: {
      brainstormContract: phase.latestRefs.brainstormContract ?? null,
    },
  };
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}
