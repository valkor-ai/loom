#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const BLOCKED_PLAN_TOOLS = new Set([
  "EnterPlanMode",
  "ExitPlanMode",
]);

const USER_GATED_ACTIONS = new Set([
  "brainstorm_clarification",
  "brainstorm_confirmation",
  "manual_review",
  "needs_user_decision",
]);

const AUTO_RUNNABLE_OPERATION_TYPES = new Set([
  "technical_baseline_generation",
  "repository_context_generation",
  "architecture_generation",
  "taskplan_generation",
  "task_execution",
  "review_generation",
  "execution_repair",
  "task_result_repair",
  "taskplan_repair",
  "architecture_artifact_repair",
]);

main().catch((error) => {
  writeJson({
    systemMessage: `Loom workflow guard skipped because of an internal error: ${error.message}`,
  });
});

async function main() {
  const input = readHookInput();
  const cwd = input.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const eventName = input.hook_event_name || "";
  const toolName = input.tool_name || "";
  const sessionId = input.session_id || "";
  const status = readStatus(cwd);

  if (eventName === "UserPromptSubmit") {
    updateSessionStateFromPrompt(sessionId, cwd, status, input.user_prompt || "");
    return;
  }

  if (eventName === "PreToolUse" && toolName === "Bash") {
    if (isLoomBashCommand(input.tool_input?.command)) {
      writeSessionState(sessionId, cwd, true, "loom_cli_command");
    }
    return;
  }

  if (!isLoomWorkflowContext(cwd, sessionId)) {
    return;
  }

  if (eventName === "PreToolUse" && BLOCKED_PLAN_TOOLS.has(toolName)) {
    writePreToolUseDeny([
      "Loom workflow is active in this Claude session.",
      `Do not use Claude Code ${toolName} or Plan Mode for Loom work because Plan Mode creates a separate user-approval gate.`,
      "Use Claude internal task/todo/subagent tools as implementation aids if useful, but keep Loom as the workflow authority. Follow the latest Loom CLI JSON instruction instead. For execute_task, finish the current TaskExecutionRequest, write the required TaskResult, and run its submitCommand. If the task is truly blocked, write a failed or blocked TaskResult and submit it so Loom can route repair.",
    ].join(" "));
    return;
  }

  if (eventName === "Stop") {
    const recoverableOperation = getRecoverableLoomOperation(status, cwd);
    if (!recoverableOperation) {
      return;
    }

    writeJson({
      decision: "block",
      reason: [
        `Loom still has an active ${recoverableOperation.label} operation that has not reached its completion barrier.`,
        recoverableOperation.detail,
        "Do not stop, recap, enter Claude Plan Mode, or wait for user approval.",
        `Run: LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" continue --project-root ${shellQuote(cwd)}`,
        recoverableOperation.afterContinue,
      ].join(" "),
      systemMessage: "Loom continuation guard blocked an incomplete stop.",
    });
  }
}

function writePreToolUseDeny(message) {
  writeJson({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: message,
    },
    systemMessage: message,
  });
}

function readHookInput() {
  let raw = "";
  try {
    raw = fs.readFileSync(0, "utf8").trim();
  } catch {
    return {};
  }
  if (!raw) {
    return {};
  }
  try {
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

function isLoomWorkflowContext(cwd, sessionId) {
  const status = readStatus(cwd);
  if (!isActiveLoomStatus(status)) {
    return false;
  }
  return readSessionState(sessionId, cwd)?.active === true;
}

function readStatus(cwd) {
  const statusPath = path.join(cwd, ".loom", "status.json");
  try {
    return JSON.parse(fs.readFileSync(statusPath, "utf8"));
  } catch {
    return null;
  }
}

function isActiveLoomStatus(status) {
  if (!status || !status.activeDeliveryId) {
    return false;
  }
  const active = Array.isArray(status.deliveries)
    ? status.deliveries.find((delivery) => delivery.deliveryId === status.activeDeliveryId)
    : null;
  if (!active) {
    return true;
  }
  return !["completed", "complete", "done", "cancelled", "canceled", "failed"].includes(String(active.status || "").toLowerCase());
}

function updateSessionStateFromPrompt(sessionId, cwd, status, userPrompt) {
  if (isLoomUserPrompt(userPrompt)) {
    writeSessionState(sessionId, cwd, true, "loom_user_prompt");
    return;
  }

  const previous = readSessionState(sessionId, cwd);
  const keepActiveForGate = previous?.active === true && isUserGatedLoomAction(status);
  writeSessionState(sessionId, cwd, keepActiveForGate, keepActiveForGate ? "loom_user_gate_reply" : "ordinary_user_prompt");
}

function writeSessionState(sessionId, cwd, active, reason) {
  if (!sessionId || !cwd) {
    return;
  }
  const statePath = sessionStatePath(sessionId);
  if (!statePath) {
    return;
  }
  const state = {
    schemaVersion: 1,
    sessionId,
    cwd: path.resolve(cwd),
    active,
    reason,
    updatedAt: new Date().toISOString(),
  };
  try {
    fs.mkdirSync(path.dirname(statePath), { recursive: true });
    fs.writeFileSync(statePath, `${JSON.stringify(state, null, 2)}\n`);
  } catch {
    // Hooks must never make ordinary Claude usage fail.
  }
}

function isUserGatedLoomAction(status) {
  const actionType = status?.effectiveNextAction?.type || status?.nextAction;
  return USER_GATED_ACTIONS.has(String(actionType || ""));
}

function readSessionState(sessionId, cwd) {
  if (!sessionId || !cwd) {
    return null;
  }
  const statePath = sessionStatePath(sessionId);
  if (!statePath) {
    return null;
  }
  try {
    const state = JSON.parse(fs.readFileSync(statePath, "utf8"));
    if (state.sessionId !== sessionId) {
      return null;
    }
    if (path.resolve(state.cwd || "") !== path.resolve(cwd)) {
      return null;
    }
    return state;
  } catch {
    return null;
  }
}

function sessionStatePath(sessionId) {
  const home = process.env.LOOM_HOME || (process.env.HOME ? path.join(process.env.HOME, ".loom") : "");
  if (!home) {
    return null;
  }
  return path.join(home, "adapters", "claude", "workflow-sessions", `${sanitizeSessionId(sessionId)}.json`);
}

function sanitizeSessionId(sessionId) {
  return String(sessionId).replace(/[^A-Za-z0-9_.-]/g, "_").slice(0, 160) || "unknown";
}

function isLoomUserPrompt(userPrompt) {
  const text = String(userPrompt || "");
  if (!text.trim()) {
    return false;
  }
  return (
    /(^|\s)\/loom(\s|$)/i.test(text) ||
    text.includes("LOOM_AGENT_PROFILE=claude") ||
    (text.includes("Base directory for this skill:") && text.includes("skills/loom"))
  );
}

function isLoomBashCommand(command) {
  const text = String(command || "");
  if (!text.trim()) {
    return false;
  }
  return (
    text.includes("$HOME/.loom/bin/loom-cli") ||
    text.includes("/.loom/bin/loom-cli") ||
    (text.includes("LOOM_AGENT_PROFILE=claude") && /\bloom(?:-cli)?\b/.test(text))
  );
}

function getRecoverableLoomOperation(status, cwd) {
  if (isUserGatedLoomAction(status)) {
    return null;
  }
  const lease = readActiveLease(status, cwd);
  const recoverableLease = getRecoverableLeaseOperation(lease, cwd);
  if (recoverableLease) {
    return recoverableLease;
  }
  return getRecoverableTaskFromStatus(status, cwd);
}

function getRecoverableTaskFromStatus(status, cwd) {
  const action = status && status.effectiveNextAction;
  if (!action || action.targetNode !== "task_execution") {
    return null;
  }
  const resultFile = action.refs && action.refs.resultFile;
  if (!resultFile) {
    return null;
  }
  const absoluteResultFile = path.isAbsolute(resultFile) ? resultFile : path.join(cwd, resultFile);
  if (fs.existsSync(absoluteResultFile)) {
    return null;
  }
  return {
    operationType: "task_execution",
    label: "TaskExecutionRequest",
    detail: `TaskResult file is missing: ${resultFile}.`,
    afterContinue: "Then follow the returned execute_task instruction until the resultFile is written and record-result succeeds.",
    resultFile: absoluteResultFile,
  };
}

function readActiveLease(status, cwd) {
  const deliveryId = status?.activeDeliveryId || status?.effectiveNextAction?.deliveryId;
  if (!deliveryId) {
    return null;
  }
  const leasePath = path.join(cwd, ".loom", "deliveries", deliveryId, "operations", "active-lease.json");
  const lease = readJsonFile(leasePath);
  if (!lease || lease.status !== "active") {
    return null;
  }
  return lease;
}

function getRecoverableLeaseOperation(lease, cwd) {
  if (!lease || !AUTO_RUNNABLE_OPERATION_TYPES.has(String(lease.operationType || ""))) {
    return null;
  }
  const operationType = String(lease.operationType);
  const request = readRequestForLease(lease, cwd);
  if (isUserGatedLeaseOperation(lease, request, cwd)) {
    return null;
  }
  const outputContract = outputContractFromRequest(request, cwd);
  const detail = detailForOperation(operationType, lease, outputContract, cwd);
  return {
    operationType,
    label: labelForOperation(operationType),
    detail,
    afterContinue: afterContinueForOperation(operationType),
  };
}

function readRequestForLease(lease, cwd) {
  const requestRef = stringValue(lease?.refs?.requestRef);
  if (!requestRef) {
    return null;
  }
  return readProjectJson(cwd, requestRef);
}

function isUserGatedLeaseOperation(lease, request, cwd) {
  if (String(lease?.operationType || "") !== "technical_baseline_generation") {
    return false;
  }
  const candidateRef = stringValue(lease?.refs?.candidateFile) || stringValue(request?.outputContract?.candidateFile);
  const candidate = candidateRef ? readProjectJson(cwd, candidateRef) : null;
  if (!isRecord(candidate)) {
    return false;
  }
  const status = stringValue(candidate.status);
  if (status === "needs_user_confirmation" || candidate.requiresUserConfirmation === true) {
    return true;
  }
  const approval = isRecord(candidate.approval) ? candidate.approval : null;
  if (stringValue(request?.projectKind) === "greenfield" && !(status === "confirmed" && approval?.type === "user_confirmed")) {
    return true;
  }
  return false;
}

function outputContractFromRequest(request, cwd) {
  if (!isRecord(request)) {
    return null;
  }
  if (isRecord(request.outputContract)) {
    return request.outputContract;
  }
  const outputContractRef = stringValue(request.outputContractRef) ||
    stringValue(request.requestManifest?.refs?.outputContract?.ref);
  if (!outputContractRef) {
    return null;
  }
  const outputContract = readProjectJson(cwd, outputContractRef);
  return isRecord(outputContract) ? outputContract : null;
}

function detailForOperation(operationType, lease, outputContract, cwd) {
  if (operationType === "architecture_generation") {
    return architectureGenerationDetail(lease, outputContract, cwd);
  }
  if (operationType === "taskplan_generation") {
    return taskPlanGenerationDetail(outputContract, cwd);
  }
  const refs = primaryOutputRefs(operationType, lease, outputContract);
  if (refs.length > 0) {
    const missing = refs.filter((ref) => !projectPathExists(cwd, ref));
    if (missing.length > 0) {
      return `Missing required output file(s): ${missing.join(", ")}.`;
    }
    return `Required output file(s) exist (${refs.join(", ")}), but the active ${operationType} lease is still open; the result must be submitted before stopping.`;
  }
  return `The active ${operationType} lease is still open; run continue so Loom can return the precise next generation, submit, or repair instruction.`;
}

function architectureGenerationDetail(lease, outputContract, cwd) {
  const sectionOutputs = arrayValue(lease?.refs?.sectionOutputs).length > 0
    ? arrayValue(lease.refs.sectionOutputs)
    : arrayValue(outputContract?.sectionOutputs);
  const files = sectionOutputs
    .filter(isRecord)
    .map((item) => ({
      section: stringValue(item.section) || "unknown",
      candidateFile: stringValue(item.candidateFile),
    }))
    .filter((item) => item.candidateFile);
  if (files.length === 0) {
    return "Architecture generation is active, but the hook could not resolve section output files; continue must inspect the active request and finish or submit it.";
  }
  const missing = files.filter((item) => !projectPathExists(cwd, item.candidateFile));
  if (missing.length > 0) {
    return `Missing architecture section candidate file(s): ${missing.map((item) => `${item.section} -> ${item.candidateFile}`).join(", ")}.`;
  }
  return "All architecture section candidate files appear to exist, but architecture_generation is still active; run continue so Loom can submit_existing_candidate or route the next instruction.";
}

function taskPlanGenerationDetail(outputContract, cwd) {
  const outlineFile = stringValue(outputContract?.outlineFile);
  const groupFilePattern = stringValue(outputContract?.groupFilePattern);
  if (!outlineFile) {
    return "TaskPlan generation is active, but outputContract.outlineFile could not be resolved; continue must inspect the active request and finish or submit it.";
  }
  if (!projectPathExists(cwd, outlineFile)) {
    return `Missing TaskPlan outline file: ${outlineFile}.`;
  }
  if (!groupFilePattern) {
    return `TaskPlan outline exists (${outlineFile}), but outputContract.groupFilePattern could not be resolved; continue must inspect the active request and finish or submit it.`;
  }
  const outline = readProjectJson(cwd, outlineFile);
  const groups = arrayValue(outline?.groups)
    .filter(isRecord)
    .map((group) => stringValue(group.groupId))
    .filter(Boolean);
  if (groups.length === 0) {
    return `TaskPlan outline exists (${outlineFile}), but no outline.groups[].groupId could be resolved; continue must finish or submit the active request.`;
  }
  const missingGroups = groups
    .map((groupId) => ({ groupId, file: groupFilePattern.replace("{groupId}", groupId) }))
    .filter((item) => !projectPathExists(cwd, item.file));
  if (missingGroups.length > 0) {
    return `Missing TaskPlan group file(s): ${missingGroups.map((item) => `${item.groupId} -> ${item.file}`).join(", ")}.`;
  }
  return "All TaskPlan grouped output files appear to exist, but taskplan_generation is still active; run continue so Loom can submit_existing_candidate or route the next instruction.";
}

function primaryOutputRefs(operationType, lease, outputContract) {
  const refs = [];
  for (const value of [
    lease?.refs?.resultFile,
    lease?.refs?.candidateFile,
    outputContract?.resultFile,
    outputContract?.candidateFile,
  ]) {
    const ref = stringValue(value);
    if (ref && !refs.includes(ref)) {
      refs.push(ref);
    }
  }
  if (operationType === "execution_repair") {
    return refs.filter((ref) => /result/i.test(path.basename(ref)) || refs.length === 1);
  }
  return refs;
}

function labelForOperation(operationType) {
  const labels = {
    technical_baseline_generation: "TechnicalBaseline generation",
    repository_context_generation: "RepositoryContext generation",
    architecture_generation: "ArchitectureSections generation",
    taskplan_generation: "TaskPlanGenerationRequest",
    task_execution: "TaskExecutionRequest",
    review_generation: "ReviewRequest",
    execution_repair: "ExecutionRepairRequest",
    task_result_repair: "TaskResult repair",
    taskplan_repair: "TaskPlan repair",
    architecture_artifact_repair: "Architecture repair",
  };
  return labels[operationType] || operationType;
}

function afterContinueForOperation(operationType) {
  if (operationType === "task_execution" || operationType === "execution_repair") {
    return "Then follow the returned execute_task instruction until the resultFile is written and submitCommand succeeds.";
  }
  if (operationType === "review_generation") {
    return "Then follow the returned review instruction until the ReviewResult is written and review accept succeeds.";
  }
  if (operationType.endsWith("_repair")) {
    return "Then follow the returned repair instruction until the repair candidate/result is written and submitted successfully.";
  }
  return "Then follow the returned generation instruction until all required candidate files are written and the submit command succeeds.";
}

function readProjectJson(cwd, ref) {
  if (!ref) {
    return null;
  }
  return readJsonFile(path.isAbsolute(ref) ? ref : path.join(cwd, ref));
}

function readJsonFile(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

function projectPathExists(cwd, ref) {
  if (!ref) {
    return false;
  }
  return fs.existsSync(path.isAbsolute(ref) ? ref : path.join(cwd, ref));
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function arrayValue(value) {
  return Array.isArray(value) ? value : [];
}

function stringValue(value) {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function writeJson(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}
