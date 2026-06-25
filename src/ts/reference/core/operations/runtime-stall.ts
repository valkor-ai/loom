import path from "node:path";
import type { OperationLease } from "../contracts";
import { pathExists } from "../state/fs";

export type PossibleRuntimeForegroundStall = {
  applies: boolean;
  reason: "task_execution_result_missing_and_runtime_probe_may_be_involved";
  confidence: "protocol_risk";
  userSummary: string;
  agentInstruction: {
    primaryAction: "resume_current_task_and_close_runtime_probe";
    rules: string[];
  };
  evidence: {
    activeOperationType: "task_execution";
    resultFile: string | null;
    resultFileStatus: "missing";
    runtimeProbeRelevantReasons: string[];
    processScanPerformed: false;
  };
};

export const runtimeForegroundProbeCloseoutRules = [
  "If a foreground runtime command is already running and shows a ready URL, listening port, or health-ready signal, do not wait for that command to exit naturally.",
  "Immediately use the ready URL, port, or health path to run the needed HTTP, browser, API, or code-level probe.",
  "After the probe, stop only the task-owned runtime process when ownership is clear.",
  "If runtime ownership is unclear, do not kill the process; record cleanup as not_safe_to_cleanup or unknown and continue TaskResult submission.",
  "Do not say or imply that the task is still waiting for the server to finish starting after readiness has been observed.",
  "Write TaskResult and run submitCommand after the probe/cleanup decision; do not leave the task at a running server prompt.",
];

export async function possibleRuntimeForegroundStall(input: {
  projectRoot: string;
  lease: OperationLease;
  request: unknown;
  resultFile?: string | null;
}): Promise<PossibleRuntimeForegroundStall | null> {
  if (input.lease.operationType !== "task_execution") {
    return null;
  }
  const resultFile = input.resultFile ?? resultFileFromLeaseOrRequest(input.lease, input.request);
  if (resultFile && await pathExists(path.join(input.projectRoot, resultFile))) {
    return null;
  }
  const runtimeProbeRelevantReasons = runtimeProbeRelevantReasonsForRequest(input.request);
  if (runtimeProbeRelevantReasons.length === 0) {
    return null;
  }
  return {
    applies: true,
    reason: "task_execution_result_missing_and_runtime_probe_may_be_involved",
    confidence: "protocol_risk",
    userSummary: "这不像权限问题。当前任务还没有提交结果，可能停在本地服务、预览、API server、worker、watcher 这类不会自动退出的运行命令上。",
    agentInstruction: {
      primaryAction: "resume_current_task_and_close_runtime_probe",
      rules: [
        "不要重做任务，也不要重新规划。",
        "读取当前 TaskExecutionRequest，继续完成这个 active task。",
        "如果你刚才启动了 dev/preview/server/worker 并已看到 ready URL、监听端口或 health-ready 信号，不要等待它自然退出。",
        "立即用该 URL、端口或健康路径完成 HTTP/browser/API/code-level 验证。",
        "验证后只停止你自己启动且归属明确的 runtime；如果无法确认归属，不要杀进程，记录 not_safe_to_cleanup 或 unknown。",
        "写 TaskResult，并执行 record-result/submitCommand。",
      ],
    },
    evidence: {
      activeOperationType: "task_execution",
      resultFile,
      resultFileStatus: "missing",
      runtimeProbeRelevantReasons,
      processScanPerformed: false,
    },
  };
}

function resultFileFromLeaseOrRequest(lease: OperationLease, request: unknown): string | null {
  if (typeof lease.refs.resultFile === "string" && lease.refs.resultFile.length > 0) {
    return lease.refs.resultFile;
  }
  if (
    request &&
    typeof request === "object" &&
    !Array.isArray(request) &&
    "outputContract" in request
  ) {
    const outputContract = (request as { outputContract?: unknown }).outputContract;
    if (
      outputContract &&
      typeof outputContract === "object" &&
      !Array.isArray(outputContract) &&
      typeof (outputContract as { resultFile?: unknown }).resultFile === "string"
    ) {
      return (outputContract as { resultFile: string }).resultFile;
    }
  }
  return null;
}

function runtimeProbeRelevantReasonsForRequest(request: unknown): string[] {
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    return [];
  }
  const record = request as Record<string, unknown>;
  const reasons: string[] = [];
  const task = isRecord(record.task) ? record.task : {};
  const executionRules = isRecord(record.executionRules) ? record.executionRules : {};
  const outputContract = isRecord(record.outputContract) ? record.outputContract : {};
  const taskKind = typeof task.taskKind === "string" ? task.taskKind : "";

  if (isRuntimeDeliveryRequirement(task.runtimeDeliveryRequirement)) {
    reasons.push("task_runtime_delivery_requirement");
  }
  if (taskKind === "runtime_delivery_closure") {
    reasons.push("runtime_delivery_closure_task");
  }
  if (isRecord(task.frontendExperienceRequirement)) {
    reasons.push("frontend_experience_task_may_require_browser_probe");
  }
  if (isRecord(executionRules.runtimeDeliveryExecutionRules)) {
    reasons.push("runtime_delivery_execution_rules");
  }
  if (isRecord(outputContract.requiredRuntimeEvidence) || isRecord(outputContract.schemaShape) && isRecord(outputContract.schemaShape.runtimeDeliveryEvidence)) {
    reasons.push("runtime_delivery_evidence_required");
  }

  return Array.from(new Set(reasons));
}

function isRuntimeDeliveryRequirement(value: unknown): boolean {
  return isRecord(value) && value.appliesToThisTask === true;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
