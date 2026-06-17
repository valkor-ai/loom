import fs from "node:fs";
import path from "node:path";

const PENDING_TTL_MS = 30 * 60 * 1000;
const MAX_IDLE_PROMPTS_PER_STAGE = 3;
const IDLE_PROMPT_COOLDOWN_MS = 15 * 1000;

export const LoomPlugin = async ({ client, directory }) => {
  const pendingBySession = new Map();
  const guardedSessions = new Map();
  const idlePromptLocks = new Map();

  function remember(sessionID, pending, options = {}) {
    const previous = pendingBySession.get(sessionID);
    const keepPrevious = previous?.signature === pending.signature && options.resetAttempts !== true;
    const attemptsByStage = keepPrevious ? previous.attemptsByStage : {};
    const requiredReadFields = Array.isArray(pending.requiredReadFields)
      ? pending.requiredReadFields
      : keepPrevious
        ? previous.requiredReadFields ?? null
        : null;
    const requiredReadGroups = Array.isArray(pending.requiredReadGroups)
      ? pending.requiredReadGroups
      : keepPrevious
        ? previous.requiredReadGroups ?? null
        : null;
    const readFields = new Set([
      ...(keepPrevious ? previous.readFields ?? [] : []),
      ...(pending.readFields ?? []),
    ]);
    const exhaustionPromptedFor = new Set([
      ...(keepPrevious ? previous.exhaustionPromptedFor ?? [] : []),
      ...(pending.exhaustionPromptedFor ?? []),
    ]);
    pendingBySession.set(sessionID, {
      ...pending,
      readFields,
      requiredReadFields,
      requiredReadGroups,
      attemptsByStage,
      exhaustionPromptedFor,
      createdAt: Date.now(),
      lastPromptAt: keepPrevious ? previous.lastPromptAt : 0,
    });
    guardedSessions.set(sessionID, {
      projectRoot: pending.projectRoot,
      activatedAt: Date.now(),
    });
  }

  function clear(sessionID) {
    pendingBySession.delete(sessionID);
    guardedSessions.delete(sessionID);
    idlePromptLocks.delete(sessionID);
  }

  return {
    "command.execute.before": async (input) => {
      if (!isLoomCommand(input.command)) {
        return;
      }
      guardedSessions.set(input.sessionID, {
        projectRoot: directory,
        activatedAt: Date.now(),
      });
      const recovered = recoverPendingFromActiveOperation(directory);
      if (recovered) {
        remember(input.sessionID, recovered, { resetAttempts: true });
      }
    },

    "tool.execute.after": async (input, output) => {
      idlePromptLocks.delete(input.sessionID);
      const parsed = extractLoomEnvelope(output.output);
      if (!parsed || !isOpencodeLoomEnvelope(parsed)) {
        return;
      }

      const envelopeProjectRoot = projectRootFromEnvelope(parsed, directory);
      guardedSessions.set(input.sessionID, {
        projectRoot: envelopeProjectRoot,
        activatedAt: Date.now(),
      });

      const currentPending = pendingBySession.get(input.sessionID);
      if (parsed.command === "inspect" && currentPending) {
        const updated = pendingAfterInspect(currentPending, parsed);
        if (updated) {
          pendingBySession.set(input.sessionID, updated);
          decorateToolOutputWithContinuation(output, updated, "inspect");
        }
        return;
      }

      const pending = buildPendingContinuation(parsed, directory);
      if (!pending) {
        if (!shouldKeepPendingAfterNonRunnableEnvelope(parsed, pendingBySession.get(input.sessionID))) {
          clear(input.sessionID);
        }
        return;
      }

      remember(input.sessionID, pending, { resetAttempts: true });
      output.title = "Loom auto-runnable continuation required";
      output.metadata = {
        ...(output.metadata ?? {}),
        loomAutoRunnable: true,
        loomNextMode: pending.mode,
        loomNextCommand: pending.command ?? null,
        loomRequestRef: pending.requestRef ?? null,
      };
      output.output = `${toolResultBanner(pending)}\n\n${output.output}`;
    },

    event: async ({ event }) => {
      if (event.type !== "session.idle") {
        return;
      }
      const sessionID = event.properties.sessionID;
      let pending = pendingBySession.get(sessionID);
      if (!pending) {
        const guarded = guardedSessions.get(sessionID);
        pending = guarded ? recoverPendingFromActiveOperation(guarded.projectRoot) : null;
        if (!pending) {
          return;
        }
        remember(sessionID, pending);
        pending = pendingBySession.get(sessionID);
        if (!pending) {
          return;
        }
      }

      const now = Date.now();
      if (now - pending.createdAt > PENDING_TTL_MS) {
        const recovered = recoverPendingFromActiveOperation(pending.projectRoot);
        if (!recovered) {
          clear(sessionID);
          return;
        }
        remember(sessionID, recovered);
        pending = pendingBySession.get(sessionID);
        if (!pending) {
          return;
        }
      }

      let stage = stageForPending(pending);
      let attemptKey = attemptKeyForPending(pending, stage);
      let idlePromptKey = `${pending.signature}|${attemptKey}`;
      const locked = idlePromptLocks.get(sessionID);
      if (locked?.key === idlePromptKey && now - locked.at < IDLE_PROMPT_COOLDOWN_MS) {
        return;
      }
      const attempts = pending.attemptsByStage?.[attemptKey] ?? 0;
      if (attempts >= MAX_IDLE_PROMPTS_PER_STAGE) {
        const recovered = recoverPendingFromActiveOperation(pending.projectRoot);
        const exhaustionKey = `exhausted:${attemptKey}`;
        if (!recovered || !isPendingOutputMissing(recovered) || pending.exhaustionPromptedFor?.has(exhaustionKey)) {
          return;
        }
        pending = {
          ...pending,
          command: recovered.command ?? pending.command,
          requestRef: pending.requestRef ?? recovered.requestRef,
          resultFile: pending.resultFile ?? recovered.resultFile,
          candidateFile: pending.candidateFile ?? recovered.candidateFile,
          operationType: recovered.operationType ?? pending.operationType,
          exhaustionPromptedFor: new Set([...(pending.exhaustionPromptedFor ?? []), exhaustionKey]),
        };
        stage = "recover_after_exhausted_stage";
        attemptKey = `recover:${exhaustionKey}`;
        idlePromptKey = `${pending.signature}|${attemptKey}`;
      }

      pending.attemptsByStage = {
        ...(pending.attemptsByStage ?? {}),
        [attemptKey]: (pending.attemptsByStage?.[attemptKey] ?? 0) + 1,
      };
      pending.lastPromptAt = now;
      pendingBySession.set(sessionID, pending);
      idlePromptLocks.set(sessionID, { key: idlePromptKey, at: now });

      try {
        await client.session.promptAsync({
          path: { id: sessionID },
          query: { directory: pending.projectRoot ?? directory },
          body: {
            agent: "build",
            system: systemPromptForPending(pending, stage),
            parts: [
              {
                type: "text",
                metadata: { loomAutoContinue: true, signature: pending.signature, stage },
                text: idlePromptForPending(pending, stage),
              },
            ],
          },
        });
      } catch {
        idlePromptLocks.delete(sessionID);
        const recovered = recoverPendingFromActiveOperation(pending.projectRoot);
        if (recovered) {
          remember(sessionID, recovered);
        } else {
          clear(sessionID);
        }
      }
    },
  };
};

function decorateToolOutputWithContinuation(output, pending, source) {
  const stage = stageForPending(pending);
  output.title = "Loom auto-runnable continuation required";
  output.metadata = {
    ...(output.metadata ?? {}),
    loomAutoRunnable: true,
    loomContinuationSource: source,
    loomNextMode: pending.mode,
    loomNextStage: stage,
    loomNextCommand: pending.command ?? null,
    loomRequestRef: pending.requestRef ?? null,
  };
  output.output = `${toolResultBanner(pending)}\n\n${inlinePromptForPending(pending, stage)}\n\n${output.output}`;
}

function isLoomCommand(command) {
  return command === "loom" || command === "loom-deploy";
}

function projectRootFromEnvelope(envelope, adapterDirectory) {
  const instruction = envelope.instruction ?? envelope.data?.instruction;
  const actionRequired = envelope.actionRequired;
  return envelope.projectRoot
    ?? instruction?.commandInvocation?.projectRoot
    ?? actionRequired?.commandInvocation?.projectRoot
    ?? instruction?.submitCommand?.commandInvocation?.projectRoot
    ?? actionRequired?.submitCommand?.commandInvocation?.projectRoot
    ?? adapterDirectory;
}

function extractLoomEnvelope(text) {
  if (typeof text !== "string") {
    return null;
  }
  const trimmed = text.trim();
  const first = trimmed.indexOf("{");
  const last = trimmed.lastIndexOf("}");
  if (first < 0 || last <= first) {
    return null;
  }
  try {
    return JSON.parse(trimmed.slice(first, last + 1));
  } catch {
    return null;
  }
}

function isOpencodeLoomEnvelope(value) {
  return (
    value &&
    typeof value === "object" &&
    value.agentProfile?.id === "opencode" &&
    typeof value.command === "string"
  );
}

function buildPendingContinuation(envelope, adapterDirectory) {
  const instruction = envelope.instruction ?? envelope.data?.instruction;
  const actionRequired = envelope.actionRequired;
  const mode = instruction?.mode ?? actionRequired?.mode;
  if (hasUserDecisionBoundary(envelope)) {
    return null;
  }
  const autoRunnable =
    actionRequired?.autoContinue === true ||
    actionRequired?.mustRunImmediately === true ||
    instruction?.autoContinue === true ||
    instruction?.mustRunImmediately === true ||
    instruction?.mustStartImmediately === true;

  if (!autoRunnable || isStopMode(mode)) {
    return null;
  }

  const projectRoot = projectRootFromEnvelope(envelope, adapterDirectory);
  const resultFile = firstString(
    instruction?.resultFile,
    actionRequired?.resultFile,
    instruction?.completionBarrier?.resultFile,
    actionRequired?.completionBarrier?.resultFile,
  );
  const candidateFile = firstString(
    instruction?.candidateFile,
    actionRequired?.candidateFile,
    instruction?.completionBarrier?.candidateFile,
    actionRequired?.completionBarrier?.candidateFile,
  );
  const targetCandidateFile = firstString(
    instruction?.targetCandidateFile,
    actionRequired?.targetCandidateFile,
    instruction?.completionBarrier?.targetCandidateFile,
    actionRequired?.completionBarrier?.targetCandidateFile,
  );
  const replacements = { resultFile, candidateFile, targetCandidateFile };
  const command = buildNextCommand(mode, instruction, actionRequired, projectRoot, replacements);
  const submitCommand = buildCommandFromCandidates(
    [
      instruction?.submitCommand,
      actionRequired?.submitCommand,
      instruction?.expectedResponse?.submitCommand,
      actionRequired?.expectedResponse?.submitCommand,
      instruction?.completionBarrier?.submitCommand,
      actionRequired?.completionBarrier?.submitCommand,
    ],
    projectRoot,
    replacements,
  );
  const followUpCommand = buildCommandFromCandidates(
    [
      instruction?.completionBarrier?.followUpCommand,
      actionRequired?.completionBarrier?.followUpCommand,
    ],
    projectRoot,
    replacements,
  );
  const requestRef = instruction?.requestRef ?? actionRequired?.requestRef ?? null;
  const signature = [
    envelope.command,
    envelope.deliveryId ?? envelope.data?.deliveryId ?? "",
    envelope.phaseId ?? envelope.data?.phaseId ?? "",
    mode ?? "",
    command ?? "",
    requestRef ?? "",
    resultFile ?? "",
    candidateFile ?? "",
    targetCandidateFile ?? "",
    submitCommand ?? "",
  ].join("|");

  return {
    mode,
    command,
    submitCommand,
    followUpCommand,
    requestRef,
    resultFile,
    candidateFile,
    targetCandidateFile,
    projectRoot,
    operationType: null,
    signature,
    readFields: new Set(),
    requiredReadFields: null,
    requiredReadGroups: null,
    attemptsByStage: {},
    exhaustionPromptedFor: new Set(),
  };
}

function shouldKeepPendingAfterNonRunnableEnvelope(envelope, pending) {
  if (!pending) {
    return false;
  }
  if (hasUserDecisionBoundary(envelope)) {
    return false;
  }
  const instruction = envelope.instruction ?? envelope.data?.instruction;
  const actionRequired = envelope.actionRequired;
  const mode = instruction?.mode ?? actionRequired?.mode;
  if (isStopMode(mode)) {
    return false;
  }

  const command = String(envelope.command ?? "");
  return new Set(["inspect", "status"]).has(command);
}

function pendingAfterInspect(pending, envelope) {
  const inspected = inspectedFieldsFromEnvelope(envelope);
  const failedFields = inspectErrorRequestedFieldsFromEnvelope(envelope);
  const requestRef = typeof envelope.data?.requestRef === "string" ? envelope.data.requestRef : null;
  if (inspected.fields.length === 0 && failedFields.length === 0) {
    return pending;
  }
  if (pending.requestRef && requestRef && pending.requestRef !== requestRef) {
    return pending;
  }
  const beforeStage = stageForPending(pending);
  const beforeAttemptKey = attemptKeyForPending(pending, beforeStage);
  const readFields = new Set([...(pending.readFields ?? []), ...inspected.fields, ...failedFields]);
  const requestReadPlanValue = inspected.values.get("requestReadPlan");
  const agentActionValue = inspected.values.get("agentAction");
  const requestReadPlan = inspected.fields.includes("requestReadPlan")
    ? requiredReadPlanFromRequestReadPlan(requestReadPlanValue)
    : null;
  const agentActionReadPlan = inspected.fields.includes("agentAction")
    ? requiredReadPlanFromAgentAction(agentActionValue)
    : null;
  const readPlan = requestReadPlan ?? agentActionReadPlan;
  const requiredReadFields = readPlan?.fields ?? pending.requiredReadFields ?? null;
  const requiredReadGroups = readPlan?.groups ?? pending.requiredReadGroups ?? null;
  const updated = {
    ...pending,
    readFields,
    requiredReadFields,
    requiredReadGroups,
  };
  const afterStage = stageForPending(updated);
  const afterAttemptKey = attemptKeyForPending(updated, afterStage);
  if (afterStage !== beforeStage || afterAttemptKey !== beforeAttemptKey) {
    updated.attemptsByStage = {
      ...(pending.attemptsByStage ?? {}),
      [afterAttemptKey]: 0,
    };
    updated.lastPromptAt = 0;
  }
  return updated;
}

function inspectedFieldsFromEnvelope(envelope) {
  const fields = [];
  const values = new Map();
  const data = envelope?.data;
  if (!data || typeof data !== "object") {
    return { fields, values };
  }
  if (typeof data.field === "string" && data.field.length > 0) {
    fields.push(data.field);
    values.set(data.field, data.value);
  }
  if (data.fields && typeof data.fields === "object" && !Array.isArray(data.fields)) {
    for (const [field, entry] of Object.entries(data.fields)) {
      if (typeof field !== "string" || field.length === 0 || !entry || typeof entry !== "object") {
        continue;
      }
      fields.push(field);
      values.set(field, entry.value);
    }
  }
  return { fields: uniqueFieldNames(fields), values };
}

function inspectErrorRequestedFieldsFromEnvelope(envelope) {
  if (envelope?.ok !== false || envelope?.command !== "inspect") {
    return [];
  }
  const requested = envelope?.error?.details?.inspectRecovery?.requestedFields
    ?? envelope?.error?.details?.requestedFields;
  return Array.isArray(requested)
    ? uniqueFieldNames(requested.filter((field) => typeof field === "string" && field.length > 0))
    : [];
}

function isStopMode(mode) {
  return new Set(["ask_user", "manual_review", "needs_user_decision", "report_blocked", "report_done"]).has(mode);
}

function hasUserDecisionBoundary(envelope) {
  const instruction = envelope?.instruction ?? envelope?.data?.instruction;
  const actionRequired = envelope?.actionRequired;
  const mode = instruction?.mode ?? actionRequired?.mode;
  if (isStopMode(mode)) {
    return true;
  }
  const nextActionType = firstString(
    instruction?.nextAction?.type,
    actionRequired?.nextAction?.type,
    envelope?.data?.nextAction?.type,
    envelope?.data?.instruction?.nextAction?.type,
  );
  if (nextActionType === "needs_user_decision" || nextActionType === "manual_review") {
    return true;
  }
  return issuesFromEnvelope(envelope).some((issue) => issue?.repairability === "requires_user_decision");
}

function issuesFromEnvelope(envelope) {
  const candidates = [
    envelope?.issues,
    envelope?.data?.issues,
    envelope?.instruction?.issues,
    envelope?.data?.instruction?.issues,
    envelope?.repairInstruction?.issues,
    envelope?.data?.repairInstruction?.issues,
  ];
  return candidates.flatMap((value) => Array.isArray(value) ? value : []);
}

function buildNextCommand(mode, instruction, actionRequired, projectRoot, replacements = {}) {
  if (mode !== "run_cli") {
    return null;
  }

  return buildCommandFromCandidates(
    [
      instruction?.command,
      actionRequired?.command,
      instruction,
      actionRequired,
    ],
    instruction?.commandInvocation?.projectRoot ?? actionRequired?.commandInvocation?.projectRoot ?? projectRoot,
    replacements,
  );
}

function buildCommandFromCandidates(candidates, projectRoot, replacements = {}) {
  for (const candidate of candidates) {
    const command = buildCommand(candidate, projectRoot, replacements);
    if (command) {
      return command;
    }
  }
  return null;
}

function buildCommand(commandLike, projectRoot, replacements = {}) {
  if (!commandLike || typeof commandLike !== "object") {
    return null;
  }
  const invocation =
    commandLike.commandInvocation ??
    commandLike.command?.commandInvocation;
  if (invocation?.argvWithProjectRoot) {
    return shellCommand(applyArgReplacements(invocation.argvWithProjectRoot, replacements));
  }

  const argv = commandLike.argv ?? commandLike.command?.argv;
  if (Array.isArray(argv)) {
    const withProjectRoot = projectRoot && !argv.includes("--project-root")
      ? [...argv, "--project-root", projectRoot]
      : argv;
    return shellCommand(applyArgReplacements(withProjectRoot, replacements));
  }

  return null;
}

function applyArgReplacements(argv, replacements) {
  return argv.map((value) => {
    const text = String(value);
    if (text === "{resultFile}" && replacements.resultFile) return replacements.resultFile;
    if (text === "{candidateFile}" && replacements.candidateFile) return replacements.candidateFile;
    if (text === "{targetCandidateFile}" && replacements.targetCandidateFile) return replacements.targetCandidateFile;
    return text;
  });
}

function toolResultBanner(pending) {
  const firstCommand = firstToolCommandForPending(pending);
  return [
    "LOOM_AUTORUN_REQUIRED",
    `mode: ${pending.mode ?? "unknown"}`,
    pending.operationType ? `operationType: ${pending.operationType}` : null,
    firstCommand ? `firstToolCommand: ${firstCommand}` : null,
    pending.command ? `nextCommand: ${pending.command}` : null,
    pending.requestRef ? `requestRef: ${pending.requestRef}` : null,
    pending.resultFile ? `resultFile: ${pending.resultFile}` : null,
    pending.candidateFile ? `candidateFile: ${pending.candidateFile}` : null,
    pending.targetCandidateFile ? `targetCandidateFile: ${pending.targetCandidateFile}` : null,
    pending.submitCommand ? `submitCommand: ${pending.submitCommand}` : null,
    "Do not answer with a recap. The next assistant action must follow this loom instruction.",
  ].filter(Boolean).join("\n");
}

function systemPromptForPending(pending, stage) {
  return [
    "You are inside a Loom auto-runnable transition.",
    "A plain text acknowledgment such as \"continuing\" is an invalid response.",
    "Your next assistant action must be a tool call or file operation that follows the Loom instruction.",
    "Do not stop, recap, ask whether to continue, create a separate plan, or wait for user approval.",
    pending.resultFile && !fileExists(pending.projectRoot, pending.resultFile)
      ? `The required result file is still missing: ${pending.resultFile}.`
      : null,
    pending.candidateFile && !fileExists(pending.projectRoot, pending.candidateFile)
      ? `The required candidate file is still missing: ${pending.candidateFile}.`
      : null,
    `Current recovery stage: ${stage}.`,
    "If the task cannot be completed, write the required failed or blocked Loom result artifact and submit it through the returned submit command.",
  ].filter(Boolean).join("\n");
}

function idlePromptForPending(pending, stage) {
  const firstCommand = firstToolCommandForPending(pending);
  const common = [
    "[loom adapter auto-continue]",
    "The previous loom command returned an auto-runnable instruction, but opencode became idle before completing it.",
    "Do not answer with text only. The next assistant step must be a tool call or file operation.",
    "Do not recap, do not ask whether to continue, and do not create a separate plan or todo.",
    firstCommand ? `Run this first tool command now if it matches the current stage:\n${firstCommand}` : null,
  ];
  if (pending.operationType && pending.mode === "recover_active_operation") {
    return [
      ...common,
      `Loom still has an active ${pending.operationType} operation.`,
      pending.requestRef ? `activeRequestRef: ${pending.requestRef}` : null,
      pending.resultFile ? `requiredResultFile: ${pending.resultFile}` : null,
      pending.candidateFile ? `requiredCandidateFile: ${pending.candidateFile}` : null,
      `Run the exact recovery command now:\n${shellCommand(["continue", "--project-root", pending.projectRoot])}`,
      "Then immediately follow the returned instruction until its candidate/result file is written and its submit command succeeds, or until Loom returns a real user gate, done state, blocked state, or non-repairable failure.",
    ].filter(Boolean).join("\n\n");
  }
  if (stage === "recover_after_exhausted_stage") {
    return [
      ...common,
      "The previous auto-continue stage was prompted repeatedly without reaching the required Loom output barrier.",
      pending.operationType ? `Active operation: ${pending.operationType}` : null,
      pending.requestRef ? `activeRequestRef: ${pending.requestRef}` : null,
      pending.resultFile ? `requiredResultFile: ${pending.resultFile}` : null,
      pending.candidateFile ? `requiredCandidateFile: ${pending.candidateFile}` : null,
      `Run the active operation recovery command now:\n${shellCommand(["continue", "--project-root", pending.projectRoot])}`,
      "Then follow the returned instruction. If implementation cannot be completed, write the required failed or blocked Loom artifact and submit it. Do not silently stop.",
    ].filter(Boolean).join("\n\n");
  }
  if (pending.mode === "run_cli") {
    return [
      ...common,
      `Run this exact CLI transition now:\n${pending.command ?? shellCommand(["continue", "--project-root", pending.projectRoot])}`,
      "After it returns, immediately follow any returned auto-runnable instruction. A progress summary is not a valid next action.",
    ].join("\n\n");
  }
  if (pending.mode === "execute_task") {
    const submitLine = pending.submitCommand ? `After writing the result file, run:\n${pending.submitCommand}` : "After writing the result file, run the submitCommand from the request.";
    if (stage === "read_request_read_plan" || stage === "read_agent_action" || stage === "read_required_fields") {
      const nextReadField = nextRequiredReadFieldForPending(pending) ?? "requestReadPlan";
      return [
        ...common,
        stage === "read_request_read_plan"
          ? `Read the TaskExecutionRequest requestReadPlan first so the required field groups are known: ${pending.requestRef ?? "instruction.requestRef"}.`
          : stage === "read_agent_action"
          ? `Compatibility fallback: read the TaskExecutionRequest agentAction because requestReadPlan was unavailable: ${pending.requestRef ?? "instruction.requestRef"}.`
          : `Read the next required TaskExecutionRequest field now: ${nextReadField}.`,
        pending.requiredReadFields?.length
          ? `Required fields remaining: ${missingRequiredReadFields(pending).join(", ") || "none"}.`
          : null,
        firstCommand ? `Run:\n${firstCommand}` : null,
        "Do not start implementation from a partially read TaskExecutionRequest unless inspect is unavailable and you are using the documented requestManifest fallback.",
      ].filter(Boolean).join("\n\n");
    }
    return [
      ...common,
      `The TaskExecutionRequest required fields already inspected in this session are: ${[...(pending.readFields ?? [])].join(", ") || "none"}. Do not stop after reading them.`,
      pending.requestRef ? `TaskExecutionRequest ref: ${pending.requestRef}` : null,
      pending.resultFile ? `Required TaskResult file: ${pending.resultFile}` : null,
      "Now modify/verify the project as required by the current task. If completion is impossible, write a failed or blocked TaskResult instead of stopping.",
      submitLine,
      "Then follow the returned Loom instruction immediately.",
    ].filter(Boolean).join("\n\n");
  }
  if (pending.mode === "generate_candidate") {
    const target = pending.targetCandidateFile ?? pending.candidateFile ?? "the candidate/result files named by the request output contract";
    if (stage === "read_request_read_plan" || stage === "read_agent_action" || stage === "read_required_fields") {
      const nextReadField = nextRequiredReadFieldForPending(pending) ?? "requestReadPlan";
      return [
        ...common,
        stage === "read_request_read_plan"
          ? `Read the generation request requestReadPlan first so the required field groups are known: ${pending.requestRef ?? "instruction.requestRef"}.`
          : stage === "read_agent_action"
          ? `Compatibility fallback: read the generation request agentAction because requestReadPlan was unavailable: ${pending.requestRef ?? "instruction.requestRef"}.`
          : `Read the next required generation request field now: ${nextReadField}.`,
        pending.requiredReadFields?.length
          ? `Required fields remaining: ${missingRequiredReadFields(pending).join(", ") || "none"}.`
          : null,
        firstCommand ? `Run:\n${firstCommand}` : null,
        "Only generate/write the candidate after the required request fields have been read, or after using the documented requestManifest fallback because inspect is unavailable.",
      ].filter(Boolean).join("\n\n");
    }
    return [
      ...common,
      `The generation request required fields already inspected in this session are: ${[...(pending.readFields ?? [])].join(", ") || "none"}.`,
      `Generate/write ${target}.`,
      pending.followUpCommand ? `Then run the follow-up command:\n${pending.followUpCommand}` : null,
      pending.submitCommand ? `Then run the submit command:\n${pending.submitCommand}` : "Then run the submitCommand from the instruction/request.",
      "Do not summarize generated sections/groups as a stopping point; follow the returned instruction immediately.",
    ].filter(Boolean).join("\n\n");
  }
  if (pending.mode === "submit_existing_candidate") {
    if (stage === "read_request_read_plan" || stage === "read_agent_action" || stage === "read_required_fields") {
      const nextReadField = nextRequiredReadFieldForPending(pending) ?? "requestReadPlan";
      return [
        ...common,
        stage === "read_request_read_plan"
          ? `Read the submit request requestReadPlan first so the required field groups are known: ${pending.requestRef ?? "instruction.requestRef"}.`
          : stage === "read_agent_action"
          ? `Compatibility fallback: read the submit request agentAction because requestReadPlan was unavailable: ${pending.requestRef ?? "instruction.requestRef"}.`
          : `Read the next required submit request field now: ${nextReadField}.`,
        pending.requiredReadFields?.length
          ? `Required fields remaining: ${missingRequiredReadFields(pending).join(", ") || "none"}.`
          : null,
        firstCommand ? `Run:\n${firstCommand}` : null,
        "Only submit the existing candidate/result after the required request fields have been read, or after using the documented requestManifest fallback because inspect is unavailable.",
      ].filter(Boolean).join("\n\n");
    }
    return [
      ...common,
      pending.requestRef ? `Use requestRef when needed: ${pending.requestRef}` : null,
      "Verify the existing candidate/result files named by the instruction still exist.",
      pending.submitCommand ? `Run the submit command now:\n${pending.submitCommand}` : "Run the submitCommand from the instruction now.",
      "Then follow the returned Loom instruction immediately.",
    ].filter(Boolean).join("\n\n");
  }
  if (pending.mode === "repair_candidate" || pending.mode === "repair_result_contract") {
    if (stage === "read_request_read_plan" || stage === "read_agent_action" || stage === "read_required_fields") {
      const nextReadField = nextRequiredReadFieldForPending(pending) ?? "requestReadPlan";
      return [
        ...common,
        stage === "read_request_read_plan"
          ? `Read the repair request requestReadPlan first so the required field groups are known: ${pending.requestRef ?? "instruction.requestRef"}.`
          : stage === "read_agent_action"
          ? `Compatibility fallback: read the repair request agentAction because requestReadPlan was unavailable: ${pending.requestRef ?? "instruction.requestRef"}.`
          : `Read the next required repair request field now: ${nextReadField}.`,
        pending.requiredReadFields?.length
          ? `Required fields remaining: ${missingRequiredReadFields(pending).join(", ") || "none"}.`
          : null,
        firstCommand ? `Run:\n${firstCommand}` : null,
        "Only repair and resubmit the referenced artifact after the required request fields have been read, or after using the documented requestManifest fallback because inspect is unavailable.",
      ].filter(Boolean).join("\n\n");
    }
    return [
      ...common,
      pending.requestRef ? `Read the repair request/ref: ${pending.requestRef}` : null,
      "Repair only the referenced Loom candidate/result artifact. Do not start a new request and do not run continue before resubmitting the repaired artifact.",
      pending.submitCommand ? `Run the submit command after repair:\n${pending.submitCommand}` : "Run the submitCommand from the repair instruction after repair.",
      "Then follow the returned Loom instruction immediately.",
    ].filter(Boolean).join("\n\n");
  }
  return [
    ...common,
    pending.requestRef ? `requestRef: ${pending.requestRef}` : null,
    pending.command ? `Run:\n${pending.command}` : `Run recovery command:\n${shellCommand(["continue", "--project-root", pending.projectRoot])}`,
    "Follow the returned instruction immediately until a real user gate, done state, blocked state, or non-repairable failure.",
  ].filter(Boolean).join("\n\n");
}

function inlinePromptForPending(pending, stage) {
  return idlePromptForPending(pending, stage)
    .replace("[loom adapter auto-continue]", "LOOM_NEXT_ACTION")
    .replace(
      "The previous loom command returned an auto-runnable instruction, but opencode became idle before completing it.",
      "The latest loom tool output updated the auto-runnable instruction state.",
    );
}

function firstToolCommandForPending(pending) {
  if (pending.mode === "run_cli" && pending.command) {
    return pending.command;
  }
  if (pending.mode === "recover_active_operation") {
    return shellCommand(["continue", "--project-root", pending.projectRoot]);
  }
  if (!pending.requestRef) {
    return null;
  }
  if (requiresRequestReadPlan(pending.mode)) {
    const groupCommand = nextRequiredReadGroupCommandForPending(pending);
    if (groupCommand) {
      return groupCommand;
    }
    const field = nextRequiredReadFieldForPending(pending);
    if (!field) {
      return null;
    }
    return shellCommand(["inspect", "--request", pending.requestRef, "--field", field, "--project-root", pending.projectRoot]);
  }
  return null;
}

function stageForPending(pending) {
  if (pending.mode === "run_cli") {
    return "run_cli";
  }
  if (pending.mode === "execute_task") {
    if (pending.resultFile && fileExists(pending.projectRoot, pending.resultFile)) {
      return "submit_result";
    }
    if (shouldReadRequestReadPlan(pending)) {
      return "read_request_read_plan";
    }
    if (shouldReadAgentAction(pending)) {
      return "read_agent_action";
    }
    if (missingRequiredReadFields(pending).length > 0) {
      return "read_required_fields";
    }
    return "execute_task";
  }
  if (pending.mode === "generate_candidate") {
    const outputFile = pending.targetCandidateFile ?? pending.candidateFile;
    if (outputFile && fileExists(pending.projectRoot, outputFile)) {
      return pending.followUpCommand ? "run_follow_up" : "submit_candidate";
    }
    if (shouldReadRequestReadPlan(pending)) {
      return "read_request_read_plan";
    }
    if (shouldReadAgentAction(pending)) {
      return "read_agent_action";
    }
    if (missingRequiredReadFields(pending).length > 0) {
      return "read_required_fields";
    }
    return "generate_candidate";
  }
  if (pending.mode === "submit_existing_candidate") {
    if (shouldReadRequestReadPlan(pending)) {
      return "read_request_read_plan";
    }
    if (shouldReadAgentAction(pending)) {
      return "read_agent_action";
    }
    if (missingRequiredReadFields(pending).length > 0) {
      return "read_required_fields";
    }
    return "submit_candidate";
  }
  if (pending.mode === "repair_candidate" || pending.mode === "repair_result_contract") {
    if (shouldReadRequestReadPlan(pending)) {
      return "read_request_read_plan";
    }
    if (shouldReadAgentAction(pending)) {
      return "read_agent_action";
    }
    if (missingRequiredReadFields(pending).length > 0) {
      return "read_required_fields";
    }
    return "repair_and_submit";
  }
  if (pending.mode === "recover_active_operation") {
    return `recover_${pending.operationType ?? "operation"}`;
  }
  return "follow_instruction";
}

function recoverPendingFromActiveOperation(projectRoot) {
  const status = readJson(path.join(projectRoot, ".loom", "status.json"));
  if (isUserGatedStatus(status)) {
    return null;
  }
  const deliveryId = status?.activeDeliveryId;
  if (!deliveryId) {
    return null;
  }
  const lease = readJson(path.join(projectRoot, ".loom", "deliveries", deliveryId, "operations", "active-lease.json"));
  if (!lease || lease.status !== "active" || !lease.operationType) {
    return null;
  }
  const refs = lease.refs && typeof lease.refs === "object" ? lease.refs : {};
  const resultFile = firstString(refs.resultFile, refs.candidateFile);
  const candidateFile = firstString(refs.candidateFile);
  const requestRef = firstString(refs.requestRef, refs.executionRequestRef, refs.reviewRequestRef);
  const signature = [
    "recovered-active-operation",
    deliveryId,
    lease.phaseId ?? "",
    lease.operationType,
    requestRef ?? "",
    resultFile ?? "",
    candidateFile ?? "",
  ].join("|");
  return {
    mode: "recover_active_operation",
    command: shellCommand(["continue", "--project-root", projectRoot]),
    submitCommand: null,
    followUpCommand: null,
    requestRef,
    resultFile,
    candidateFile,
    targetCandidateFile: null,
    projectRoot,
    operationType: lease.operationType,
    signature,
    readFields: new Set(),
    requiredReadFields: null,
    requiredReadGroups: null,
    attemptsByStage: {},
    exhaustionPromptedFor: new Set(),
  };
}

function isUserGatedStatus(status) {
  const actionType = firstString(status?.effectiveNextAction?.type, status?.nextAction?.type, status?.nextAction);
  return actionType === "needs_user_decision" || actionType === "manual_review" || String(actionType || "").startsWith("brainstorm_");
}

function requiresRequestReadPlan(mode) {
  return new Set([
    "execute_task",
    "generate_candidate",
    "submit_existing_candidate",
    "repair_candidate",
    "repair_result_contract",
  ]).has(mode);
}

function shouldReadRequestReadPlan(pending) {
  return Boolean(
    requiresRequestReadPlan(pending.mode) &&
    pending.requestRef &&
    !pending.readFields?.has("requestReadPlan") &&
    !Array.isArray(pending.requiredReadFields),
  );
}

function shouldReadAgentAction(pending) {
  return Boolean(
    requiresRequestReadPlan(pending.mode) &&
    pending.requestRef &&
    pending.readFields?.has("requestReadPlan") &&
    !pending.readFields?.has("agentAction") &&
    !Array.isArray(pending.requiredReadFields),
  );
}

function missingRequiredReadFields(pending) {
  if (!Array.isArray(pending.requiredReadFields)) {
    return [];
  }
  return pending.requiredReadFields.filter((field) => !pending.readFields?.has(field));
}

function nextRequiredReadFieldForPending(pending) {
  if (shouldReadRequestReadPlan(pending)) {
    return "requestReadPlan";
  }
  if (shouldReadAgentAction(pending)) {
    return "agentAction";
  }
  return missingRequiredReadFields(pending)[0] ?? null;
}

function nextRequiredReadGroupCommandForPending(pending) {
  if (!Array.isArray(pending.requiredReadGroups)) {
    return null;
  }
  for (const group of pending.requiredReadGroups) {
    const fields = Array.isArray(group.fields) ? group.fields : [];
    if (!fields.some((field) => !pending.readFields?.has(field))) {
      continue;
    }
    const argv = Array.isArray(group.readCommand?.argv)
      ? group.readCommand.argv.map((value) => String(value) === "{requestRef}" ? pending.requestRef : String(value))
      : ["inspect", "--request", pending.requestRef, "--field", fields.join(",")];
    const withProjectRoot = argv.includes("--project-root")
      ? argv
      : [...argv, "--project-root", pending.projectRoot];
    return shellCommand(withProjectRoot);
  }
  return null;
}

function requiredReadPlanFromRequestReadPlan(value) {
  if (!value || typeof value !== "object" || !Array.isArray(value.groups)) {
    return null;
  }
  const groups = requiredReadGroupsFromRequestReadPlan(value);
  if (groups.length > 0) {
    return {
      fields: dedupeCoveredFieldPaths(groups.flatMap((group) => group.fields)),
      groups,
    };
  }
  return { fields: [], groups: [] };
}

function requiredReadPlanFromAgentAction(value) {
  const read = value?.read;
  if (!read || typeof read !== "object") {
    return null;
  }
  const groups = requiredReadGroupsFromAgentAction(read);
  if (groups.length > 0) {
    return {
      fields: dedupeCoveredFieldPaths(groups.flatMap((group) => group.fields)),
      groups,
    };
  }
  const legacyFields = requiredLegacyReadFields(read);
  if (legacyFields.length > 0) {
    return { fields: legacyFields, groups: null };
  }
  const labels = Array.isArray(read.required)
    ? read.required.map(normalizeReadLabel).filter((field) => field && field !== "this request" && field !== "referencedArtifactReadGuide")
    : [];
  const fields = dedupeCoveredFieldPaths(labels);
  return fields.length > 0 ? { fields, groups: null } : { fields: [], groups: [] };
}

function requiredReadGroupsFromRequestReadPlan(readPlan) {
  if (!Array.isArray(readPlan.groups)) {
    return [];
  }
  const groups = [];
  for (const group of readPlan.groups) {
    if (!group || typeof group !== "object" || group.required === false || !Array.isArray(group.fields)) {
      continue;
    }
    const fields = dedupeCoveredFieldPaths(group.fields.filter((field) => typeof field === "string" && field.length > 0));
    if (fields.length === 0) {
      continue;
    }
    const argv = Array.isArray(group.readCommand?.argv)
      ? group.readCommand.argv.map((value) => String(value))
      : ["inspect", "--request", "{requestRef}", "--field", fields.join(",")];
    groups.push({
      fields,
      readCommand: {
        name: "inspect",
        argv,
      },
    });
  }
  return groups;
}

function requiredReadGroupsFromAgentAction(read) {
  if (!Array.isArray(read.fieldGroups)) {
    return [];
  }
  const groups = [];
  for (const group of read.fieldGroups) {
    if (!group || typeof group !== "object" || group.required === false || !Array.isArray(group.fields)) {
      continue;
    }
    const fields = dedupeCoveredFieldPaths(group.fields.filter((field) => typeof field === "string" && field.length > 0));
    if (fields.length === 0) {
      continue;
    }
    groups.push({
      fields,
      readCommand: {
        name: "inspect",
        argv: ["inspect", "--request", "{requestRef}", "--field", fields.join(",")],
      },
    });
  }
  return groups;
}

function requiredLegacyReadFields(read) {
  const fields = read.fields;
  if (!Array.isArray(fields)) {
    return [];
  }
  const required = [];
  for (const entry of fields) {
    if (!entry || typeof entry.field !== "string" || entry.required === false) {
      continue;
    }
    required.push(entry.field);
  }
  return dedupeCoveredFieldPaths(required);
}

function normalizeReadLabel(value) {
  return String(value)
    .replace(/\s+when\s+.+$/i, "")
    .replace(/\s+if\s+.+$/i, "")
    .trim();
}

function attemptKeyForPending(pending, stage = stageForPending(pending)) {
  const readFields = [...(pending.readFields ?? [])].sort().join(",");
  const requiredReadFields = Array.isArray(pending.requiredReadFields)
    ? pending.requiredReadFields.slice().sort().join(",")
    : "unknown";
  const resultState = pending.resultFile
    ? fileExists(pending.projectRoot, pending.resultFile) ? "result:exists" : "result:missing"
    : "result:none";
  const candidateState = pending.candidateFile
    ? fileExists(pending.projectRoot, pending.candidateFile) ? "candidate:exists" : "candidate:missing"
    : "candidate:none";
  const targetState = pending.targetCandidateFile
    ? fileExists(pending.projectRoot, pending.targetCandidateFile) ? "target:exists" : "target:missing"
    : "target:none";
  return [stage, `read:${readFields}`, `required:${requiredReadFields}`, resultState, candidateState, targetState].join("|");
}

function isPendingOutputMissing(pending) {
  return Boolean(
    (pending.resultFile && !fileExists(pending.projectRoot, pending.resultFile)) ||
    (pending.candidateFile && !fileExists(pending.projectRoot, pending.candidateFile)) ||
    (pending.targetCandidateFile && !fileExists(pending.projectRoot, pending.targetCandidateFile)),
  );
}

function fileExists(projectRoot, file) {
  if (!file || typeof file !== "string") {
    return false;
  }
  return fs.existsSync(path.isAbsolute(file) ? file : path.join(projectRoot, file));
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

function firstString(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.length > 0) {
      return value;
    }
  }
  return null;
}

function uniqueFieldNames(fields) {
  return [...new Set(fields.map((field) => String(field).trim()).filter((field) => field.length > 0))];
}

function dedupeCoveredFieldPaths(fields) {
  const unique = uniqueFieldNames(fields);
  return unique.filter((field) => !unique.some((candidate) => candidate !== field && field.startsWith(`${candidate}.`)));
}

function shellCommand(argv) {
  return `LOOM_AGENT_PROFILE=opencode LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" ${argv.map(shellQuote).join(" ")}`;
}

function shellQuote(value) {
  const text = String(value);
  if (/^[A-Za-z0-9_./:=@%+-]+$/.test(text)) {
    return text;
  }
  return `'${text.replace(/'/g, "'\\''")}'`;
}
