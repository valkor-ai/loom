const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function assertIncludes(file, needle, message) {
  const text = read(file);
  assert.ok(text.includes(needle), `${file}: ${message}`);
}

function assertNotIncludes(file, needle, message) {
  const text = read(file);
  assert.equal(text.includes(needle), false, `${file}: ${message}`);
}

function assertMatches(file, pattern, message) {
  const text = read(file);
  assert.ok(pattern.test(text), `${file}: ${message}`);
}

const requestWriteExpectations = [
  ["src/core/operations/brainstorm.ts", "writeRequestManifestAtomic(paths.root, requestPath, request)"],
  ["src/core/operations/repository-context.ts", "writeRequestManifestAtomic(root, requestPath, request)"],
  ["src/core/operations/repository-context.ts", "writeRequestManifestAtomic(root, brainstormPlan.requestPath, brainstormPlan.request)"],
  ["src/core/operations/contracts.ts", "writeRequestManifestAtomic(root, absolutePath, parsed)"],
  ["src/core/operations/tasks.ts", "writeRequestManifestAtomic(root, absolutePath, parsed)"],
  ["src/core/operations/tasks.ts", "writeRequestManifestAtomic(root, requestFile, requestWithOperation)"],
  ["src/core/operations/review.ts", "writeRequestManifestAtomic(root, requestFile, parsed)"],
  ["src/core/operations/repair.ts", "writeRequestManifestAtomic(root, requestFile, request)"],
  ["src/core/operations/repair.ts", "writeRequestManifestAtomic(root, paths.taskExecutionRequestFile"],
];

assertIncludes(
  "src/core/operations/request-manifest.ts",
  "protocolAuthority: \"request_manifest_refs\"",
  "request manifests must declare refs as protocol authority",
);
assertIncludes(
  "src/core/operations/request-manifest.ts",
  "Do not invent or probe unlisted sidecar files",
  "request manifests must forbid guessed sidecar refs",
);
assertIncludes(
  "src/core/operations/request-manifest.ts",
  "section-schemas.json",
  "request manifests must explicitly tell agents that architecture section schemas are inside outputContractRef",
);
assertIncludes(
  "src/core/operations/request-manifest.ts",
  "manifest[refKey] = ref",
  "request manifests must replace large fields with explicit *Ref fields",
);
assertIncludes(
  "src/core/operations/request-manifest.ts",
  "hydrateRequestManifest",
  "runtime validators must be able to hydrate ref-first requests",
);
for (const [file, text] of requestWriteExpectations) {
  assertIncludes(file, text, `${file} must write Agent-facing requests as ref-first manifests`);
}

for (const file of [
  "src/core/operations/contracts.ts",
  "src/core/operations/tasks.ts",
  "src/core/operations/review.ts",
  "src/core/operations/repair.ts",
  "src/core/operations/continue.ts",
  "src/core/operations/get-status.ts",
]) {
  assertIncludes(file, "hydrateRequestManifest", `${file} must hydrate ref-first requests before protocol reads`);
}

assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "const ROUTING_KEYS",
  "Agent-facing instruction must be a stable routing projection",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "\"requestRef\"",
  "routing projection must always allow requestRef",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "\"candidateKind\"",
  "routing projection must preserve candidate kind",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "\"outputSummary\"",
  "routing projection must preserve concrete output hints",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "\"finalResponseGuard\"",
  "routing projection must preserve TaskExecution completion guard",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "compactContinuationContract",
  "routing projection must expose a compact unified continuation contract",
);
assertIncludes(
  "src/commands/agent-facing-instruction.ts",
  "\"requestReadProtocol\"",
  "routing projection must preserve the ref-first read protocol",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "requestReadPlan.groups",
  "auto-runnable instructions must tell agents to use root request read groups",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "inspect readCommand first",
  "auto-runnable instructions must tell agents to run inspect read commands before fallback reads",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "requestReadPlan",
  "auto-runnable instructions must prefer the root requestReadPlan before fallback reads",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "that group's fields through requestManifest refs",
  "auto-runnable instructions must preserve requestManifest fallback reads",
);
assertIncludes(
  "src/commands/envelope.ts",
  "agentFacingInstruction(rawInstruction)",
  "top-level envelope instruction must use the routing projection",
);
assertIncludes(
  "src/commands/envelope.ts",
  "data.repairInstruction",
  "top-level envelope instruction must fall back to repairInstruction when business validation fails",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "parent directory",
  "artifact write policy must tell agents to create missing parent directories",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "retry the same write",
  "artifact write policy must tell agents to retry after creating missing parent directories",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "do not repeat the malformed tool call",
  "output policy must tell agents to recover from malformed file-write tool calls",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "executionRules.sourceEditPreparationContract",
  "output policy must route source writes through the request-owned source edit preparation contract",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "For Brainstorm ask_user gates",
  "output policy must route Brainstorm ask_user through a request read plan",
);
assertIncludes(
  "src/core/operations/output-policy.ts",
  "Do not stop at a request-ready/path-only recap",
  "output policy must prevent Brainstorm ask_user from stopping before presenting a user-facing block",
);
assertIncludes(
  "src/core/operations/agent-action.ts",
  "brainstormSessionAgentActionContract",
  "BrainstormSessionRequest must use a shared read contract source",
);
assertIncludes(
  "src/core/operations/agent-action.ts",
  "after presenting the next required Brainstorm block",
  "Brainstorm stopConditions must stop only after a block was presented to the user",
);
assertIncludes(
  "src/core/operations/agent-action.ts",
  "deliveryContext.sources",
  "Brainstorm read plan must cover phase-continuation delivery sources",
);
assertIncludes(
  "src/commands/inspect.ts",
  "resolveRequestContextRefField",
  "inspect must resolve contextRefs-backed Brainstorm fields",
);
assertIncludes(
  "src/core/operations/continue.ts",
  "writeRequestManifestAtomic(projectRoot, requestPath, repairedRequest)",
  "continue must repair legacy Brainstorm requests missing agentAction read fields",
);
assertIncludes(
  "src/core/operations/tasks.ts",
  "sourceEditPreparationContract({",
  "TaskExecutionRequest must include a request-owned source edit preparation contract",
);
assertIncludes(
  "src/core/operations/repair.ts",
  "sourceEditPreparationContract({",
  "deploy-sourced execution repair must reuse the same source edit preparation contract",
);
assertIncludes(
  "src/core/operations/routing-instructions.ts",
  "autoContinue: true",
  "auto-runnable instructions must be emitted through the routing-instructions builder",
);
assertIncludes(
  "src/core/operations/tasks.ts",
  "withAutoRunnableTransition({",
  "TaskResult repair instructions must use the unified auto-runnable transition builder",
);
assertNotIncludes(
  "src/commands/compact-request-output.ts",
  "agentFacingInstruction",
  "request command compaction must not trim instruction before envelope actionRequired is derived",
);
assertIncludes(
  "src/commands/compact-request-output.ts",
  "const { request: _request, ...rest } = result",
  "request command compaction should only remove the large request payload before envelope routing projection",
);
assertIncludes(
  "src/commands/output.ts",
  "agentFacingInstruction(instruction)",
  "compact output must be display-only and reuse the same routing projection",
);
assertNotIncludes(
  "src/commands/output.ts",
  "COMPACT_INSTRUCTION_OMIT_KEYS",
  "compact output must not own semantic instruction field deletion",
);
assertNotIncludes(
  "src/core/operations/continue.ts",
  "normalizeArchitectureSectionRequestForContinue",
  "continue must not rewrite old requests as a compatibility fallback",
);
assertNotIncludes(
  "src/core/operations/continue.ts",
  "normalizeTaskPlanGenerationRequestForContinue",
  "continue must not rewrite old requests as a compatibility fallback",
);

assertIncludes(
  "src/core/operations/brainstorm.ts",
  "brainstormSessionAgentActionContract({",
  "src/core/operations/brainstorm.ts must define Brainstorm agentAction through the shared read-plan helper",
);
assertIncludes(
  "src/core/operations/brainstorm.ts",
  "referencedArtifactReadGuide",
  "src/core/operations/brainstorm.ts must still expose artifact read guidance through request refs",
);

for (const file of [
  "src/core/operations/contracts.ts",
  "src/core/operations/repository-context.ts",
  "src/core/operations/tasks.ts",
  "src/core/operations/review.ts",
  "src/core/operations/repair.ts",
]) {
  assertIncludes(file, "agentActionContract({", `${file} must still define complete Agent action protocol in request refs`);
  assertIncludes(file, "referencedArtifactReadGuide", `${file} must still expose artifact read guidance through request refs`);
}

for (const file of [
  "src/core/contracts.ts",
  "src/core/validators.ts",
  "src/core/operations/brainstorm.ts",
  "src/core/operations/contracts.ts",
  "src/core/operations/repository-context.ts",
  "src/core/operations/tasks.ts",
  "src/core/operations/review.ts",
  "src/core/operations/repair.ts",
  "src/core/operations/continue.ts",
  "src/core/operations/request-manifest.ts",
  "src/core/operations/artifact-read-guide.ts",
  "src/commands/agent-facing-instruction.ts",
  "src/commands/compact-request-output.ts",
  "src/commands/envelope.ts",
  "src/commands/output.ts",
  "plugins/codex/skills/loom/SKILL.md",
]) {
  for (const oldToken of [
    "codexAction",
    "CodexAction",
    "codex-action",
    "codex-facing",
    "Codex-facing",
    "codex_repairable",
    "codex_review_explanation",
    "codex_inferred",
    "codex_recommended",
    "codex_managed",
    "codexOwns",
    "codexResolution",
  ]) {
    assertNotIncludes(file, oldToken, `${file} must not expose Codex-specific protocol token ${oldToken}`);
  }
}

assertNotIncludes(
  "src/core/operations/contracts.ts",
  "Write one candidate per outputContract.sectionOutputs[] entry.",
  "Architecture request must not expose legacy write-all wording",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "Single-section protocol",
  "Architecture request must retain the single-section continue protocol",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "ArchitectureSectionCandidate.status is the wrapper status and must be ready",
  "runtime_delivery section rules must distinguish wrapper status from nested runtimeDelivery.status",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "outputContract.allowedRefsUsage",
  "Architecture request must foreground mechanical allowedRefs usage before section generation",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "Every generated acceptanceRefs[] value and coverage acceptanceId must be selected exactly from allowedRefs.acceptanceRefs",
  "Architecture request must forbid invented acceptance refs before validator repair",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "fieldPresenceMatrix",
  "runtime_delivery schema shape must expose required/omit/nullable field presence rules",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "omitWhenNotApplicableNeverNull",
  "runtime_delivery schema shape must make omit-vs-null behavior mechanical",
);
assertIncludes(
  "src/core/operations/contracts.ts",
  "Omit when api.required=false or no API entry exists; do not write null.",
  "runtime_delivery schema shape must make optional/non-null API fields explicit",
);
assertIncludes(
  "src/core/operations/tasks.ts",
  "verificationResults[].evidenceType must use task.verificationIntents[].acceptableEvidence",
  "TaskExecution request must disambiguate verification evidence enums from concept evidence enums",
);
assertIncludes(
  "src/core/operations/review.ts",
  "recommendedNextAction: reviewNextActionTypeSchema.options.join",
  "ReviewResult schema shape must project recommendedNextAction from the validator enum",
);
for (const file of [
  "plugins/codex/skills/loom/SKILL.md",
  "plugins/claude-code/skills/loom/SKILL.md",
  "plugins/opencode/.opencode/commands/loom.md",
]) {
  assertIncludes(file, "repair_candidate", `${file} must document candidate repair instruction mode`);
}
assertMatches(
  "plugins/codex/skills/loom/SKILL.md",
  /Do not invent shell selectors/,
  "skill must prevent guessed request selectors when requestReadPlan or agentAction exists",
);

console.log("Agent-facing request protocol verification passed.");
