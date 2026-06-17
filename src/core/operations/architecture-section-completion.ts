export const architectureSingleSectionCompletionCondition =
  "instruction.targetCandidateFile exists and completionBarrier.followUpCommand has run; any returned auto-runnable instruction has been followed.";

export function architectureSingleSectionWriteTarget(
  target: { section?: unknown; schemaRef?: unknown; candidateFile?: unknown; schemaShape?: unknown; enumRefs?: unknown; generationRules?: unknown } | null | undefined,
): Record<string, unknown> {
  const currentTarget: Record<string, unknown> = {
    kind: "architecture_single_section",
    section: typeof target?.section === "string" ? target.section : null,
    schemaRef: typeof target?.schemaRef === "string" ? target.schemaRef : null,
    candidateFile: typeof target?.candidateFile === "string" ? target.candidateFile : null,
    completionCondition: architectureSingleSectionCompletionCondition,
    followUpCommand: {
      name: "continue",
      rule: "After candidateFile exists, run instruction.completionBarrier.followUpCommand.commandInvocation immediately as an agent tool action. Do not present it as a user action.",
    },
    notAStoppingPoint: true,
    rule: "This is the current section selected by the active instruction. Use schemaShape/enumRefs/generationRules here as the current section contract, write only this candidateFile, then run instruction.completionBarrier.followUpCommand.commandInvocation before any recap.",
  };
  if (isRecord(target?.schemaShape)) {
    currentTarget.schemaShape = target.schemaShape;
  }
  if (isRecord(target?.enumRefs)) {
    currentTarget.enumRefs = target.enumRefs;
  }
  if (Array.isArray(target?.generationRules)) {
    currentTarget.generationRules = target.generationRules.filter((rule): rule is string => typeof rule === "string");
  }
  return currentTarget;
}

export function architectureSingleSectionCompletionBarrier(targetCandidateFile: string | null): Record<string, unknown> {
  return {
    targetCandidateFile,
    followUpCommand: {
      name: "continue",
      argv: ["continue"],
    },
    rules: [
      "This single-section ArchitectureSections step is incomplete until targetCandidateFile exists and followUpCommand has been run.",
      "After targetCandidateFile exists, immediately run followUpCommand.commandInvocation as an agent tool action and read the returned CLI envelope.",
      "If the returned instruction is auto-runnable, follow it immediately before any final or progress response.",
      "Do not run architecture accept until the follow-up command returns submit_existing_candidate or all section files exist.",
      "A progress summary listing completedSections or missingSections is not a valid stop condition.",
      "Do not present the follow-up command as a user action.",
    ],
  };
}

export function architectureSingleSectionRequiredSteps(): string[] {
  return [
    "read instruction.requestRef and its root requestReadPlan when present",
    "use requestReadPlan.groups inspect commands for required ArchitectureSections fields; if absent, use agentAction.read.fieldGroups as compatibility fallback",
    "the current section schema lives in agentAction.write.currentTarget.schemaShape or the matching requestReadPlan field value",
    "write only instruction.targetSection to instruction.targetCandidateFile",
    "run instruction.completionBarrier.followUpCommand.commandInvocation immediately after targetCandidateFile exists",
    "read the returned CLI envelope",
    "immediately follow the returned instruction when it is auto-runnable",
  ];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
