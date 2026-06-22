import type { BrainstormStartResult } from "../core/operations/brainstorm";
import {
  brainstormAskUserInstructionPolicy,
  brainstormAskUserReadStep,
} from "../core/operations/output-policy";

export function brainstormStartInstruction(result: BrainstormStartResult): Record<string, unknown> {
  return {
    mode: "ask_user",
    ...brainstormAskUserInstructionPolicy(),
    requestRef: result.requestPath,
    nextAction: {
      type: "brainstorm_clarification",
      source: "brainstorm_session_request",
      deliveryId: result.deliveryId,
      phaseId: result.phaseId,
      ref: result.requestPath,
      reason: "BRAINSTORM_SESSION_REQUEST_CREATED",
    },
    userMessage: "Read the BrainstormSessionRequest through requestRef, then present the next required Brainstorm clarification block. Write and submit BrainstormCandidate only after the user confirms the dedicated final_summary block.",
    expectedResponse: {
      kind: "brainstorm_progressive_clarification",
      rule: "Agent manages the Brainstorm conversation. Read requestRef through requestReadPlan.groups inspect commands. For phase_scope, concept_grounding, and frontend_experience confirmations, continue to the next Brainstorm block in chat. Read outputContract/generationProtocol/enumRefs, write BrainstormCandidate, and run submitCommand only after the user explicitly confirms the dedicated final_summary block.",
      requestReadRule: brainstormAskUserReadStep,
      requestRef: result.requestPath,
      currentTurnAnswerRule: {
        consumeCurrentUserMessage: true,
        meaning: "If the same user message that invoked @loom plan already contains clear phase scope, concept, frontend, and final confirmation details, treat it as the user's answer for the relevant Brainstorm gates instead of asking again.",
        doNotAskAgainWhenCurrentMessageIsExplicit: true,
        ifAmbiguousAskUser: true,
      },
    },
  };
}
