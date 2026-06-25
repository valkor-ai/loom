export function repairSubmitRouting(options: {
  kind: "candidate" | "result";
  submitCommandName: string;
}): Record<string, unknown> {
  return {
    submitCommandReturnsInstruction: true,
    followReturnedInstructionImmediately: true,
    doNotRunContinueBeforeSuccessfulSubmit: true,
    doNotAskUserAfterSuccessfulSubmit: true,
    submitCommandName: options.submitCommandName,
    rule: `Repair the ${options.kind} file first, then rerun ${options.submitCommandName} with the same file. Do not run loom continue or any next request command until that submit command succeeds.`,
  };
}
