export const artifactChatOutputPolicy = {
  writeArtifactToFileOnly: true,
  doNotPasteArtifactJson: true,
  doNotPasteDiff: true,
  doNotPrintFullFileContent: true,
  doNotUseChatVisibleDiffForLoomArtifacts: true,
  doNotUseApplyPatchForLoomArtifacts: true,
  appliesToRepair: true,
  prohibitedWriteMethods: [
    "apply_patch for .loom candidate/result/request artifacts",
    "chat-visible unified diff workflows for .loom artifacts",
    "commands that echo or print the full artifact JSON into stdout",
  ],
  preferredWriteMethod: "Use a quiet programmatic file write for .loom candidate/result JSON, and print only the written file path or a short confirmation.",
  allowedChatSummary: [
    "artifact kind",
    "artifact path",
    "submit command result",
    "validation issues",
    "next action",
  ],
  rule: "Write loom candidate/result JSON to the requested file path only using a quiet file write. Do not use apply_patch for .loom artifacts, and do not paste generated JSON, unified diffs, or full file contents into chat unless the user explicitly asks to inspect them.",
} as const;

export const sourceChangeChatOutputPolicy = {
  doNotPasteSourceDiff: true,
  doNotPasteFullSourceFiles: true,
  doNotRunDiffForChat: true,
  doNotUseChatVisiblePatchWorkflow: true,
  doNotUseApplyPatchForSourceChanges: true,
  summarizeChangedFilesOnly: true,
  prohibitedChatVisibleMethods: [
    "apply_patch for project source changes when it would print a large patch into chat",
    "git diff, git show, or equivalent diff-printing commands unless the user explicitly asks",
    "commands that print full source files or generated build output into stdout",
  ],
  preferredEditMethod: "For execute_task, follow executionRules.sourceEditPreparationContract before modifying source files. Modify project files with quiet file edits or editor operations that do not paste large patches into chat; after editing, summarize changed file paths and verification results only. If a native file-write tool fails input validation because required path/content arguments were missing or invalid, return to the source edit preparation contract, rebuild complete write arguments, and continue; do not repeat the malformed tool call.",
  allowedChatSummary: [
    "changed file paths",
    "short change summary",
    "verification commands and pass/fail result",
    "blocking validation issues",
    "next action",
  ],
  rule: "When modifying project source files for a task, keep chat output compact and follow executionRules.sourceEditPreparationContract for write planning. Do not use chat-visible patch/diff workflows that paste large source patches into chat; do not paste unified diffs, large patches, generated build output, or full source file contents unless the user explicitly asks to inspect them. If a file-write tool call fails before writing because its required arguments were missing or invalid, return to the write plan sequence, correct the arguments, and continue the task.",
} as const;

export const quietSourceChangeStep =
  "Before modifying project source files during execute_task, follow executionRules.sourceEditPreparationContract: form a write plan with targetPath, writeKind, contentBasis, writeMethod, and writePayloadReady=true. Modify project source files quietly. Do not use apply_patch or any chat-visible patch/diff workflow for source changes when it would paste a large patch into chat; do not print unified diffs, large patches, generated build output, or full source files. If a native file-write tool fails input validation because path/content arguments are missing or invalid, return to the write plan sequence, retry with complete valid arguments, then use a quiet programmatic write if native validation fails again. Report only changed file paths, a short summary, verification results, submit result, validation issues, and next action.";

export const contextReadOutputPolicy = {
  fullReadAllowedForCorrectness: true,
  doNotPrintFullRequestJson: true,
  doNotPrintFullLoomArtifacts: true,
  doNotPrintFullSkillFile: true,
  doNotUseSedOrCatForChatVisibleLargeFileOutput: true,
  preferCompactSelectors: true,
  maxChatLinesPerFileRead: 80,
  preferredChatVisibleMethods: [
    "programmatic full-file parsing without printing contents",
    "loom continue/status compact response",
    "loom inspect field selection for request fields",
    "rg targeted search for displayed matches",
    "short excerpts under 80 lines only when necessary",
  ],
  rule: "Use loom inspect for loom request fields whenever requestReadPlan or agentAction.read.fieldGroups are present. You may read full loom artifacts only as a last-resort correctness fallback, but keep terminal/chat-visible output compact. Do not print full .loom JSON artifacts, historical TaskResult files, TaskPlan files, request files, SKILL.md, full source files, or large command outputs into chat unless the user explicitly asks.",
} as const;

export const compactContextReadStep =
  "Use requestReadPlan as the default request-reading plan when present; otherwise use agentAction.read.fieldGroups. For each required group, run its loom inspect readCommand first so the complete grouped field values are read together without printing full .loom artifacts. Do not open full agentAction/outputContract/contextProjection/task/sourceContext/executionRules sidecars to discover the read plan. If inspect fails, fall back only to that group's fields through requestManifest refs and targeted selectors. Full sidecar reads are a last-resort correctness fallback and must stay compact: do not print full .loom JSON, historical TaskResult files, TaskPlan/run/result files, request files, SKILL.md, full source files, or large command outputs.";

export const brainstormAskUserReadStep =
  "For Brainstorm ask_user gates, read requestRef and use requestReadPlan groups first when present; otherwise use agentAction.read.fieldGroups inspect commands before presenting the phase_scope, concept_grounding, frontend_experience, or final_summary blocks. Do not stop at a request-ready/path-only recap; stop only after presenting the next required Brainstorm block as a concrete user-facing question or confirmation summary. Do not infer Brainstorm scope, sources, concepts, frontend target, candidateFile, output schema, or submit command from guessed legacy root fields such as .objective, .scope, or .outputContract; if such selectors return null, discard that result and use requestManifest refs plus sourceFieldAccessHints. Do not read candidate_write_contract, outputContract, generationProtocol, enumRefs, candidateFile, or submitCommand until the dedicated final_summary block has been presented and explicitly confirmed by the user.";

export const quietArtifactWriteStep =
  "Write .loom candidate/result files silently with a quiet programmatic file write. If the target file's parent directory does not exist, create that parent directory and retry the same write instead of stopping. If a native file-write tool fails input validation because path/content arguments are missing or invalid, retry with complete valid arguments or use a quiet programmatic write instead of repeating the malformed tool call. Do not use apply_patch or any chat-visible diff workflow for .loom artifacts because those tools print the full patch/body into the conversation. The only stdout after writing should be the artifact path or a short confirmation.";

export function artifactGenerationProtocolPolicy(): Record<string, unknown> {
  return {
    chatOutputPolicy: artifactChatOutputPolicy,
    contextReadOutputPolicy,
  };
}

export function artifactInstructionPolicy(): Record<string, unknown> {
  return {
    requestReadProtocol: refFirstRequestReadProtocol(),
    chatOutputPolicy: artifactChatOutputPolicy,
    contextReadOutputPolicy,
    mustNotReportProgressBeforeExecuting: true,
    mustNotAskUserBeforeExecuting: true,
    communicationRules: [
      compactContextReadStep,
      quietArtifactWriteStep,
      "Do not paste generated candidate/result JSON into chat.",
      "Do not paste unified diffs for .loom candidate/result files into chat.",
      "When this instruction is auto-runnable, do not send a progress summary before executing the next required command.",
      "After writing the artifact and running the required command, report only the artifact path, submit result, validation issues, and next action.",
    ],
  };
}

export function brainstormAskUserInstructionPolicy(): Record<string, unknown> {
  return {
    requestReadProtocol: refFirstRequestReadProtocol(),
    chatOutputPolicy: artifactChatOutputPolicy,
    contextReadOutputPolicy,
    communicationRules: [
      compactContextReadStep,
      brainstormAskUserReadStep,
      quietArtifactWriteStep,
      "Do not paste generated BrainstormCandidate JSON into chat.",
      "Do not paste full BrainstormSessionRequest JSON into chat.",
      "A Brainstorm request-ready/path-only message is not a valid ask_user response. Present the current required Brainstorm block so the user has something concrete to confirm or correct.",
      "Ask the user only for missing or ambiguous Brainstorm confirmation details; if the current user message already answers the current gate, consume it.",
      "During phase_scope, concept_grounding, and frontend_experience confirmations, continue the progressive conversation in chat; do not write BrainstormCandidate or run brainstorm accept.",
      "Write and submit BrainstormCandidate only after the user confirms the dedicated final_summary block.",
    ],
  };
}

export function refFirstRequestReadProtocol(): Record<string, unknown> {
  return {
    authority: "requestReadPlan",
    firstSelector: ".requestReadPlan",
    readRule: "After opening requestRef, use requestReadPlan.groups[].readCommand when present. If requestReadPlan is absent, use agentAction.read.fieldGroups and each group's inspect readCommand. Do not read the agentAction sidecar merely to discover the read plan when requestReadPlan is present.",
    nullFieldRule: "If a large field such as agentAction, sourceRefs, outputContract, rules, enumRefs, task, sourceContext, executionRules, or fieldAccessHints is null at the request root, read it through requestReadPlan/inspect first. Use listed *Ref sidecars only as targeted fallback for the exact fields requested by the read group.",
    unlistedSidecarRule: "Do not probe unlisted .refs files such as section-schemas.json; listed refs are the complete protocol authority.",
  };
}

export function taskExecutionOutputPolicy(): Record<string, unknown> {
  return {
    requestReadProtocol: refFirstRequestReadProtocol(),
    chatOutputPolicy: artifactChatOutputPolicy,
    contextReadOutputPolicy,
    sourceChangeOutputPolicy: sourceChangeChatOutputPolicy,
    mustNotReportProgressBeforeExecuting: true,
    mustNotAskUserBeforeExecuting: true,
    communicationRules: [
      compactContextReadStep,
      quietArtifactWriteStep,
      "Do not paste generated candidate/result JSON into chat.",
      "Write loom TaskResult JSON silently to resultFile; do not paste it into chat.",
      "Before any source edit or TaskResult artifact write, follow executionRules.sourceEditPreparationContract from the request.",
      quietSourceChangeStep,
      "For execute_task, do not send progress-only summaries, interim handoff notes, or next-step summaries before TaskResult is written and submitCommand succeeds.",
      "A recovery command is not a normal model choice during auto-runnable execution; continue the current instruction while tool calls are available.",
      "After source edits, report only changed file paths, a short summary, verification results, submit result, validation issues, and next action.",
    ],
  };
}

export function artifactRepairPolicy(): Record<string, unknown> {
  return {
    chatOutputPolicy: artifactChatOutputPolicy,
    contextReadOutputPolicy,
    mustNotReportProgressBeforeExecuting: true,
    mustNotAskUserBeforeExecuting: true,
    communicationRules: [
      compactContextReadStep,
      quietArtifactWriteStep,
      "Repair the requested loom artifact file silently.",
      "Do not paste repaired JSON or unified diffs into chat.",
      "Report only the repaired file path, submit result, validation issues, and next action.",
    ],
  };
}
