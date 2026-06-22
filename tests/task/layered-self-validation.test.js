#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cli = path.join(repoRoot, "dist", "cli.js");

const checks = {
  L1: [],
  L2: [],
  L3: [],
};

function mark(level, message) {
  checks[level].push(message);
}

function run(args, projectRoot, options = {}) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return options.returnEnvelope ? envelope : envelope.data;
}

function runCompact(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope;
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function hydrateRequest(root, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(projectFile(root, value));
  }
  return hydrated;
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function copyDir(source, target) {
  fs.rmSync(target, { recursive: true, force: true });
  fs.cpSync(source, target, { recursive: true });
}

function requestFromCommand(data, root) {
  return data.request ?? hydrateRequest(root, readJson(projectFile(root, data.requestPath ?? data.requestRef)));
}

function assertRequestOutputParentDirsExist(root, request, label) {
  const refs = new Set();
  collectRequestOutputRefs(request, refs);
  assert.ok(refs.size > 0, `${label}: expected request output refs to inspect`);
  for (const ref of refs) {
    assert.ok(
      fs.existsSync(path.dirname(projectFile(root, ref))),
      `${label}: CLI must pre-create output parent directory for ${ref}`,
    );
  }
}

function collectRequestOutputRefs(value, refs) {
  if (Array.isArray(value)) {
    for (const item of value) collectRequestOutputRefs(item, refs);
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (
      ["candidateFile", "blockedFile", "resultFile", "outlineFile", "groupFilePattern", "targetCandidateFile"].includes(key) &&
      typeof child === "string" &&
      child.trim().length > 0 &&
      !child.trim().startsWith("{") &&
      !child.trim().includes("://")
    ) {
      refs.add(child);
      continue;
    }
    collectRequestOutputRefs(child, refs);
  }
}

function allReadFields(agentAction) {
  return (agentAction?.read?.fieldGroups ?? []).flatMap((group) => group.fields ?? []);
}

function assertStatusRoutesToActiveRequest(root, deliveryId, phaseId, expected) {
  const status = readJson(projectFile(root, ".loom/status.json"));
  assert.equal(status.effectiveNextAction?.type, expected.type, `${expected.label}: status must expose current request action`);
  assert.equal(status.effectiveNextAction?.ref, expected.requestRef, `${expected.label}: status must point to current requestRef`);
  assert.equal(status.effectiveNextAction?.refs?.requestRef, expected.requestRef, `${expected.label}: status refs must include requestRef`);
  assert.equal(status.effectiveNextAction?.refs?.activeOperationType, expected.operationType, `${expected.label}: status refs must include active operation type`);
  const staleContinueDecision = projectFile(root, `.loom/deliveries/${deliveryId}/control/${phaseId}/continue-latest.json`);
  assert.equal(fs.existsSync(staleContinueDecision), false, `${expected.label}: stale continue-latest must be cleared after request creation`);
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function assertRequestProtocol(request, label, options = {}) {
  assert.equal(request.generationProtocol?.readRequestBeforeActing, true, `${label}: missing readRequestBeforeActing`);
  if (options.allowProjectModification) {
    assert.equal(request.generationProtocol?.modifyProjectFilesAllowed, true, `${label}: expected project modification permission`);
  } else {
    assert.equal(request.generationProtocol?.writeCandidateFileOnly ?? request.generationProtocol?.doNotWriteAcceptedArtifact, true, `${label}: missing candidate-only protocol`);
    assert.equal(request.generationProtocol?.doNotModifyProjectFiles, true, `${label}: non-execution request must forbid project file changes`);
  }
  assertChatOutputPolicy(request.generationProtocol?.chatOutputPolicy, `${label} generationProtocol`);
  assert.equal(request.generationProtocol?.doNotWriteAcceptedArtifact, true, `${label}: missing doNotWriteAcceptedArtifact`);
  assert.ok(request.enumRefs && Object.keys(request.enumRefs).length > 0, `${label}: missing enumRefs`);
  const output = request.outputContract;
  assert.ok(output, `${label}: missing output contract`);
  assert.ok(output.schemaShape ?? output.outlineSchemaShape ?? output.groupSchemaShape, `${label}: missing schemaShape`);
  assert.ok(request.submitCommand, `${label}: missing top-level submitCommand`);
  assert.equal(output.submitCommand, undefined, `${label}: submitCommand must not be nested in outputContract`);
  assertReferencedArtifactReadGuide(request, label);
  if (options.expectBlocked !== false) {
    assert.ok(request.blockedOutput, `${label}: missing blockedOutput`);
  }
  mark("L2", `${label} request is self-describing`);
}

function assertReferencedArtifactReadGuide(request, label) {
  const refKeys = new Set();
  for (const containerKey of ["sourceRefs", "contextRefs"]) {
    const container = request[containerKey];
    if (!container || typeof container !== "object" || Array.isArray(container)) continue;
    for (const [key, value] of Object.entries(container)) {
      if (typeof value === "string" && value.length > 0 && key.endsWith("Ref")) {
        refKeys.add(key);
      }
    }
  }
  for (const key of ["reviewPacketRef", "changeContextRef"]) {
    if (typeof request[key] === "string" && request[key].length > 0) {
      refKeys.add(key);
    }
  }
  if (refKeys.size === 0) return;
  assert.ok(Array.isArray(request.referencedArtifactReadGuide), `${label}: missing referencedArtifactReadGuide for refs ${Array.from(refKeys).join(", ")}`);
  const guideKeys = new Set(request.referencedArtifactReadGuide.map((entry) => entry.refKey));
  for (const refKey of refKeys) {
    assert.ok(guideKeys.has(refKey), `${label}: referencedArtifactReadGuide missing ${refKey}`);
  }
  for (const entry of request.referencedArtifactReadGuide) {
    assert.equal(entry.doNotGuessAlternateRoots, true, `${label}: ${entry.refKey} guide must forbid alternate root guessing`);
    assert.ok(Array.isArray(entry.requiredSelectors) && entry.requiredSelectors.length > 0, `${label}: ${entry.refKey} guide must include required selectors`);
    assert.ok(typeof entry.nullSelectorHandling === "string" && entry.nullSelectorHandling.length > 0, `${label}: ${entry.refKey} guide must include null handling`);
  }
}

function assertChatOutputPolicy(policy, label) {
  assert.equal(policy?.writeArtifactToFileOnly, true, `${label}: missing writeArtifactToFileOnly`);
  assert.equal(policy?.doNotPasteArtifactJson, true, `${label}: missing doNotPasteArtifactJson`);
  assert.equal(policy?.doNotPasteDiff, true, `${label}: missing doNotPasteDiff`);
  assert.equal(policy?.doNotUseChatVisibleDiffForLoomArtifacts, true, `${label}: missing chat-visible diff guard`);
  assert.equal(policy?.doNotUseApplyPatchForLoomArtifacts, true, `${label}: missing apply_patch guard for loom artifacts`);
  assert.ok(
    policy?.prohibitedWriteMethods?.some((method) => method.includes("apply_patch")),
    `${label}: prohibitedWriteMethods must mention apply_patch`,
  );
}

function assertInstructionOutputPolicy(instruction, label) {
  assert.equal(instruction?.mustNotAskUserBeforeExecuting, true, `${label}: instruction must forbid asking before auto-runnable execution`);
  assert.equal(instruction?.mustNotReportProgressBeforeExecuting, true, `${label}: instruction must forbid progress-only messages before auto-runnable execution`);
  assert.ok(typeof instruction?.requestRef === "string" || instruction?.mode === "run_cli", `${label}: instruction must route through requestRef or run_cli command`);
  if (typeof instruction?.requestRef === "string") {
    assert.equal(instruction?.requestReadProtocol?.authority, "requestReadPlan", `${label}: instruction must expose requestReadPlan-first request read authority`);
    assert.ok(
      instruction?.requestReadProtocol?.nullFieldRule?.includes("requestReadPlan/inspect"),
      `${label}: instruction must tell Agent to use requestReadPlan/inspect before listed refs`,
    );
  }
  assert.equal(Object.prototype.hasOwnProperty.call(instruction ?? {}, "agentAction"), false, `${label}: top-level instruction must not echo full agentAction`);
  assert.equal(Object.prototype.hasOwnProperty.call(instruction ?? {}, "outputContract"), false, `${label}: top-level instruction must not echo full outputContract`);
  assert.equal(Object.prototype.hasOwnProperty.call(instruction ?? {}, "generationSteps"), false, `${label}: top-level instruction must not echo long generation steps`);
}

function assertPlanCompactBrainstormInstruction(envelope) {
  assert.equal(envelope.instruction?.mode, "ask_user", "plan --compact must return a Brainstorm ask_user instruction");
  assert.equal(envelope.instruction?.requestRef, envelope.data?.requestPath, "plan --compact instruction requestRef must point to requestPath");
  assert.equal(envelope.instruction?.candidateFile, undefined, "plan --compact ask_user must not expose candidateFile before final_summary confirmation");
  assert.equal(envelope.instruction?.submitCommand, undefined, "plan --compact ask_user must not expose brainstorm submitCommand before final_summary confirmation");
  assert.equal(envelope.instruction?.requestReadProtocol?.authority, "requestReadPlan", "plan --compact must expose requestReadPlan-first request read protocol");
  assert.equal(envelope.instruction?.expectedResponse?.requestRef, envelope.instruction?.requestRef, "plan expectedResponse must repeat requestRef");
  assert.equal(envelope.instruction?.expectedResponse?.kind, "brainstorm_progressive_clarification", "plan expectedResponse must describe progressive clarification, not immediate accept");
  assert.equal(envelope.instruction?.expectedResponse?.candidateFile, undefined, "plan expectedResponse must not expose candidateFile before final_summary confirmation");
  assert.equal(envelope.instruction?.expectedResponse?.submitCommand, undefined, "plan expectedResponse must not expose submitCommand before final_summary confirmation");
  assert.equal(envelope.actionRequired, undefined, "plan ask_user instruction must not be auto-runnable");
  assert.equal(Object.prototype.hasOwnProperty.call(envelope.instruction ?? {}, "agentAction"), false, "plan instruction must not echo full agentAction");
  assert.equal(Object.prototype.hasOwnProperty.call(envelope.instruction ?? {}, "outputContract"), false, "plan instruction must not echo full outputContract");
}

function assertBrainstormStartInstruction(instruction, requestPath, label) {
  assert.equal(instruction?.mode, "ask_user", `${label}: must return a Brainstorm ask_user instruction`);
  assert.equal(instruction?.requestRef, requestPath, `${label}: instruction requestRef must point to requestPath`);
  assert.equal(instruction?.candidateFile, undefined, `${label}: ask_user instruction must not expose candidateFile before final_summary confirmation`);
  assert.equal(instruction?.submitCommand, undefined, `${label}: ask_user instruction must not expose brainstorm submitCommand before final_summary confirmation`);
  assert.equal(instruction?.requestReadProtocol?.authority, "requestReadPlan", `${label}: instruction must expose requestReadPlan-first request read protocol`);
  assert.ok(
    instruction?.requestReadProtocol?.readRule?.includes("agentAction.read.fieldGroups"),
    `${label}: instruction must require Brainstorm ask_user inspect read plan`,
  );
  assert.ok(
    instruction?.expectedResponse?.requestReadRule?.includes(".objective") &&
      instruction.expectedResponse.requestReadRule.includes("sourceFieldAccessHints"),
    `${label}: expectedResponse must forbid guessed legacy Brainstorm root fields`,
  );
  assert.ok(
    instruction?.expectedResponse?.requestReadRule?.includes("request-ready/path-only"),
    `${label}: expectedResponse must prevent path-only Brainstorm ask_user stops`,
  );
  assert.equal(instruction?.expectedResponse?.kind, "brainstorm_progressive_clarification", `${label}: expectedResponse must not imply immediate Brainstorm accept`);
  assert.equal(instruction?.expectedResponse?.candidateFile, undefined, `${label}: expectedResponse must not expose candidateFile before final_summary confirmation`);
  assert.equal(instruction?.expectedResponse?.submitCommand, undefined, `${label}: expectedResponse must not expose submitCommand before final_summary confirmation`);
  assert.equal(instruction?.expectedResponse?.requestRef, instruction?.requestRef, `${label}: expectedResponse must repeat requestRef`);
}

function assertBrainstormRequestReadPlan(request, label) {
  assert.equal(request?.agentAction?.actionKind, "brainstorm_session", `${label}: Brainstorm request must expose agentAction`);
  assert.equal(request?.agentAction?.read?.primaryMethod, "inspect", `${label}: Brainstorm request must prefer inspect reads`);
  assert.equal(request?.agentAction?.read?.fields, undefined, `${label}: Brainstorm request must not expose legacy read.fields`);
  const groups = request?.agentAction?.read?.fieldGroups ?? [];
  assert.ok(Array.isArray(groups) && groups.length > 0, `${label}: Brainstorm read plan must expose fieldGroups`);
  assert.ok(
    request?.agentAction?.stopConditions?.some((condition) => condition.includes("after presenting the next required Brainstorm block")),
    `${label}: Brainstorm stopConditions must stop only after presenting the current user-facing block`,
  );
  const fields = new Set(groups.flatMap((group) => group.fields ?? []));
  for (const field of [
    "agentAction.instruction",
    "agentAction.stopConditions",
    "originalRequest",
    "contextRefs",
    "sourceFieldAccessHints",
    "firstClarificationGate",
    "clarificationConversationProtocol.requiredBlocks",
    "conceptGroundingRequest",
    "rules.phaseScopeOptionComparison",
  ]) {
    assert.ok(fields.has(field), `${label}: Brainstorm read plan must include ${field}`);
  }
  assert.ok(fields.has("requirementContext.sourceItems"), `${label}: Brainstorm read plan must cover original requirement source context`);
  if (request.contextRefs?.deliveryContextRef) {
    assert.ok(fields.has("deliveryContext.sources"), `${label}: phase continuation Brainstorm read plan must cover prior delivery source context`);
    assert.ok(fields.has("latestConfirmedRequirementDecision"), `${label}: phase continuation Brainstorm read plan must cover latest confirmed requirement decision`);
    assert.ok(fields.has("confirmedRequirementDecisionsIndex"), `${label}: phase continuation Brainstorm read plan must cover confirmed decisions index`);
  } else {
    assert.equal(fields.has("deliveryContext.sources"), false, `${label}: initial Brainstorm read plan must not include phase-continuation-only deliveryContext.sources`);
    assert.equal(fields.has("latestConfirmedRequirementDecision"), false, `${label}: initial Brainstorm read plan must not include phase-continuation-only decision history`);
  }
  assert.ok(
    groups.every((group) => group.readCommand?.argv?.[0] === "inspect" && group.readCommand.argv[2] === "{requestRef}" && Array.isArray(group.fields)),
    `${label}: every Brainstorm read fieldGroup must provide an inspect command`,
  );
  assert.ok(
    request.agentAction.write.rules.some((rule) => rule.includes("guessed selectors")),
    `${label}: Brainstorm write rules must reject null results from guessed selectors`,
  );
  assertNextPhasePreviewGenerationRules(request, label);
  assertPhaseScopeOptionComparisonRules(request, label);
  assertCandidateSelfReviewRules(request, label);
}

function assertNextPhasePreviewGenerationRules(request, label) {
  const generation = request?.rules?.nextPhasePreviewGeneration;
  assert.equal(generation?.validationMode, "generation_guidance_only", `${label}: nextPhasePreview specificity must be generation guidance, not an accept validator`);
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("non-binding seed")),
    `${label}: nextPhasePreview rules must preserve non-binding phase handoff semantics`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("kind=none") && rule.includes("only")),
    `${label}: nextPhasePreview rules must not use kind=none for uncertainty`,
  );
  const candidateRules = request?.outputContract?.schemaShape?.candidateRules ?? [];
  assert.ok(
    candidateRules.some((rule) => rule.includes("concrete source-grounded business objects")),
    `${label}: candidate schemaShape must require concrete source-grounded next phase preview content`,
  );
  assert.ok(
    candidateRules.some((rule) => rule.includes("keyword hints are never scope or acceptance authority")),
    `${label}: candidate schemaShape must keep keyword hints advisory-only`,
  );
}

function assertPhaseScopeOptionComparisonRules(request, label) {
  const guidance = request?.clarificationGuidance?.phaseScopeOptionComparison;
  assert.equal(guidance?.requiredByDefault, true, `${label}: phase_scope option comparison must be required by default`);
  assert.equal(guidance?.nextPhaseSeedIsPreselectedAnswer, false, `${label}: nextPhaseSeed must not be treated as a preselected user answer`);
  assert.deepEqual(guidance?.optionCount, { min: 2, max: 3 }, `${label}: phase_scope guidance must request 2-3 options`);
  assert.equal(guidance?.recommendationMustPreserveCoreCurrentPhaseOutcome, true, `${label}: recommended phase_scope option must preserve the current phase core outcome`);
  assert.equal(guidance?.narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems, true, `${label}: narrower options must not be recommended when they defer explicit seed items`);
  assert.equal(guidance?.scopeReductionRequiresExplicitUserConfirmation, true, `${label}: scope reductions must require explicit user confirmation`);
  assert.equal(
    guidance?.atomicSingleScopeException?.disallowedWhenMultipleScopePreviewItemsOrLifecycleActionsExist,
    true,
    `${label}: multi-item or multi-action phase continuation must not use a single preselected scope`,
  );
  assert.equal(
    Object.prototype.hasOwnProperty.call(guidance ?? {}, "doNotFabricateAlternativesWhenSingleClearCut"),
    false,
    `${label}: phase_scope guidance must not keep the old broad single-clear-cut escape hatch`,
  );
  const generation = request?.rules?.phaseScopeOptionComparison;
  assert.equal(generation?.validationMode, "generation_guidance_only", `${label}: phase_scope option comparison must be generation guidance, not an accept validator`);
  assert.equal(generation?.requiredByDefault, true, `${label}: phase_scope rules must require option comparison by default`);
  assert.equal(generation?.nextPhaseSeedIsPreselectedAnswer, false, `${label}: phase_scope rules must keep nextPhaseSeed non-binding`);
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("present 2-3 source-grounded phase scope options") && rule.includes("by default")),
    `${label}: phase_scope rules must require 2-3 options by default`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("decompose the source-grounded current-phase candidate work into scope items")),
    `${label}: phase_scope rules must classify scope items before composing options`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("goal-essential item")) &&
      generation?.rules?.some((rule) => rule.includes("flow-support item")) &&
      generation?.rules?.some((rule) => rule.includes("current-object lifecycle item")),
    `${label}: phase_scope rules must define goal, support, and lifecycle scope categories`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("Do not expose these internal category names to the user")),
    `${label}: phase_scope category names must stay internal`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("recommended option from all goal-essential items plus all flow-support items")),
    `${label}: phase_scope rules must generate recommended option from required items`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("alternate option includes a support-like item")),
    `${label}: phase_scope rules must compare support items across options`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("compare the included-scope line of every alternate option against the recommended option")),
    `${label}: phase_scope rules must reconcile alternate included scope with recommended scope`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("broader option by adding real experience/management extension items")),
    `${label}: phase_scope rules must keep broad option to true extensions`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("Recommend exactly one")),
    `${label}: phase_scope rules must require one recommended option`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("one separate visual block per phase_scope option")),
    `${label}: phase_scope rules must prevent run-on option paragraphs`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("stable multi-line template")),
    `${label}: phase_scope rules must require a stable multi-line option template`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("nextPhaseSeed") && rule.includes("not as a preselected user answer")),
    `${label}: phase_scope rules must keep nextPhaseSeed from becoming a preselected answer`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("must preserve the current phase's source-grounded core outcome")),
    `${label}: phase_scope rules must keep recommendations from shrinking the current phase core outcome`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("do not recommend it when it defers explicit nextPhaseSeed.scopePreview items")),
    `${label}: phase_scope rules must prevent narrower alternatives from becoming the default recommendation`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("scope reduction") && rule.includes("ask the user to confirm")),
    `${label}: phase_scope rules must require explicit confirmation for seed-item reductions`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("Do not rewrite source-grounded or previously confirmed object relationships")),
    `${label}: phase_scope rules must protect confirmed object semantics`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("defer detailed relationship semantics to concept_grounding")),
    `${label}: phase_scope rules must defer uncertain relationship semantics to concept grounding`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("Avoid implying that one object is replaced, relinked, inherited, transferred, frozen, or restored")),
    `${label}: phase_scope rules must avoid inventing relationship effects`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("atomic phase") && rule.includes("single phase_scope")),
    `${label}: phase_scope rules must allow single scope only for atomic phases`,
  );
  assert.ok(
    generation?.rules?.some((rule) => rule.includes("multiple nextPhaseSeed.scopePreview items") && rule.includes("not atomic")),
    `${label}: phase continuation with multiple scopePreview items must require options`,
  );
  assert.ok(
    request?.clarificationConversationProtocol?.blockExecutionRules?.some((rule) => rule.includes("present 2-3 source-grounded phase scope options") && rule.includes("by default")),
    `${label}: phase_scope block must expose option comparison instructions to Agent`,
  );
  assert.ok(
    request?.clarificationConversationProtocol?.blockConfirmationRules?.phase_scope?.includes("recommended option"),
    `${label}: phase_scope confirmation rule must mention recommended option confirmation`,
  );
  const candidateRules = request?.outputContract?.schemaShape?.candidateRules ?? [];
  assert.ok(
    candidateRules.some((rule) => rule.includes("confirmed phase_scope option") && rule.includes("existing BrainstormCandidate fields")),
    `${label}: candidate rules must require confirmed phase_scope option to land in existing fields`,
  );
}

function assertCandidateSelfReviewRules(request, label) {
  const selfReview = request?.rules?.candidateSelfReview;
  assert.equal(selfReview?.validationMode, "generation_guidance_only", `${label}: BrainstormCandidate self-review must be generation guidance, not an accept validator`);
  assert.ok(
    selfReview?.rules?.some((rule) => rule.includes("Before writing or submitting BrainstormCandidate")),
    `${label}: self-review rules must run before BrainstormCandidate submit`,
  );
  assert.ok(
    selfReview?.rules?.some((rule) => rule.includes("scope.included[].items") && rule.includes("domainModel.businessFlows[].summary")),
    `${label}: self-review rules must verify final_summary details land in existing fields`,
  );
  assert.ok(
    selfReview?.rules?.some((rule) => rule.includes("every confirmed scope.included item") && rule.includes("scope-item coverage summary")),
    `${label}: self-review rules must verify every included scope item is covered`,
  );
  assert.ok(
    selfReview?.rules?.some((rule) => rule.includes("object-operation summary") && rule.includes("key field sets")),
    `${label}: self-review rules must verify object-operation details land in existing fields`,
  );
  assert.ok(
    selfReview?.rules?.some((rule) => rule.includes("Do not create a separate Markdown spec")),
    `${label}: self-review rules must not introduce Markdown spec as a parallel authority`,
  );
  assert.ok(
    request?.clarificationConversationProtocol?.blockExecutionRules?.some((rule) => rule.includes("Self-review must verify")),
    `${label}: Brainstorm block rules must expose self-review instructions to Agent`,
  );
  const candidateRules = request?.outputContract?.schemaShape?.candidateRules ?? [];
  assert.ok(
    candidateRules.some((rule) => rule.includes("Self-review must verify")),
    `${label}: candidate schemaShape must carry self-review rules`,
  );
}

function assertContextReadOutputPolicy(target, label) {
  assert.equal(target?.contextReadOutputPolicy?.fullReadAllowedForCorrectness, true, `${label}: full context reads must remain allowed for correctness`);
  assert.equal(target?.contextReadOutputPolicy?.doNotPrintFullLoomArtifacts, true, `${label}: must forbid full Loom artifact output`);
  assert.equal(target?.contextReadOutputPolicy?.doNotPrintFullSkillFile, true, `${label}: must forbid full skill output`);
  assert.equal(target?.contextReadOutputPolicy?.doNotUseSedOrCatForChatVisibleLargeFileOutput, true, `${label}: must discourage sed/cat chat-visible large-file output`);
  assert.ok(
    target?.communicationRules === undefined ||
      target.communicationRules.some((rule) => rule.includes("do not print full .loom JSON")),
    `${label}: communication rules must forbid full Loom JSON output`,
  );
}

function assertSourceChangeOutputPolicy(instruction, request, label) {
  assert.equal(request?.generationProtocol?.contextReadOutputPolicy?.doNotPrintFullLoomArtifacts, true, `${label}: generationProtocol must forbid full Loom context output`);
  assert.equal(request?.executionRules?.contextReadOutputPolicy?.doNotPrintFullLoomArtifacts, true, `${label}: executionRules must forbid full Loom context output`);
  assert.equal(Object.prototype.hasOwnProperty.call(instruction ?? {}, "sourceChangeOutputPolicy"), false, `${label}: top-level instruction must not echo source output policy`);
  assert.equal(request?.generationProtocol?.sourceChangeOutputPolicy?.doNotPasteSourceDiff, true, `${label}: request must forbid source diff chat output`);
  assert.equal(request?.generationProtocol?.sourceChangeOutputPolicy?.doNotUseChatVisiblePatchWorkflow, true, `${label}: request must forbid chat-visible source patch workflows`);
  assert.equal(request?.executionRules?.sourceChangeOutputPolicy?.doNotPasteSourceDiff, true, `${label}: executionRules must include source output policy`);
  assert.equal(request?.executionRules?.sourceChangeOutputPolicy?.doNotUseApplyPatchForSourceChanges, true, `${label}: executionRules must forbid apply_patch source patch output`);
  assert.ok(
    request?.executionRules?.rules?.some((rule) => rule.includes("do not paste unified diffs") || rule.includes("Do not use apply_patch")),
    `${label}: execution rules must include compact source-output guidance`,
  );
}

function assertSourceEditPreparationContract(request, label) {
  const contract = request?.executionRules?.sourceEditPreparationContract;
  assert.equal(contract?.contractKind, "source_edit_preparation", `${label}: missing source edit preparation contract`);
  assert.equal(contract?.authority, "TaskExecutionRequest.executionRules.sourceEditPreparationContract", `${label}: source edit contract must be request-authoritative`);
  assert.equal(contract?.resultFile, request?.outputContract?.resultFile, `${label}: source edit contract must point to outputContract.resultFile`);
  assert.ok(contract?.requiredWritePlanFields?.targetPath, `${label}: source edit contract must require targetPath`);
  assert.ok(contract?.requiredWritePlanFields?.writePayloadReady, `${label}: source edit contract must require writePayloadReady`);
  assert.ok(
    contract?.sequence?.some((step) => step.name === "form_write_plan"),
    `${label}: source edit contract must require write plan formation`,
  );
  assert.ok(
    contract?.sequence?.some((step) => step.name === "recover_write_tool_validation_failure"),
    `${label}: source edit contract must define malformed write recovery`,
  );
  assert.ok(
    contract?.forbiddenOutcomes?.some((rule) => rule.includes("Do not repeat a malformed")),
    `${label}: source edit contract must forbid repeated malformed write calls`,
  );
}

function assertRepairOutputPolicy(repairInstruction, label) {
  assertChatOutputPolicy(repairInstruction?.chatOutputPolicy, `${label} repairInstruction`);
  assert.ok(
    repairInstruction?.communicationRules?.some((rule) => rule.includes("parent directory") && rule.includes("retry")),
    `${label}: repairInstruction must tell agents to create missing artifact parent directories and retry`,
  );
  assert.ok(
    repairInstruction?.communicationRules?.some((rule) => rule.includes("Do not paste repaired JSON")),
    `${label}: repairInstruction must forbid repaired JSON chat output`,
  );
  assert.ok(
    repairInstruction?.communicationRules?.some((rule) => rule.includes("unified diffs")),
    `${label}: repairInstruction must forbid unified diff chat output`,
  );
}

function assertAutoRunInstruction(data, expectedName, label) {
  assert.equal(data.instruction?.mode, "run_cli", `${label}: expected run_cli instruction`);
  assert.equal(data.instruction?.autoContinue, true, `${label}: expected autoContinue`);
  assert.equal(data.instruction?.mustRunImmediately, true, `${label}: expected mustRunImmediately`);
  assert.equal(data.instruction?.mustNotAskUserBeforeRunning, true, `${label}: expected mustNotAskUserBeforeRunning`);
  assert.equal(data.instruction?.mustNotAskUserBeforeExecuting, true, `${label}: expected mustNotAskUserBeforeExecuting`);
  assert.equal(data.instruction?.mustNotReportProgressBeforeExecuting, true, `${label}: expected mustNotReportProgressBeforeExecuting`);
  assert.equal(data.instruction?.command?.name, expectedName, `${label}: unexpected command name`);
  assert.equal(data.instruction?.commandInvocation?.kind, "loom_user_launcher", `${label}: run_cli must expose commandInvocation`);
  assert.equal(data.instruction?.commandInvocation?.launcherRef, "$HOME/.loom/bin/loom-cli", `${label}: run_cli must use the user launcher`);
  assert.deepEqual(data.instruction?.commandInvocation?.env, {
    LOOM_AGENT_PROFILE: "codex",
    LOOM_COMPACT_OUTPUT: "1",
  }, `${label}: run_cli commandInvocation must carry agent env`);
  assert.deepEqual(data.instruction?.commandInvocation?.argv, data.instruction?.command?.argv, `${label}: run_cli commandInvocation argv must match command argv`);
  mark("L3", `${label} returned auto-runnable instruction`);
}

function assertNoMechanicalMaintenanceFields(data, label) {
  assert.equal(data.indexMaintenance, undefined, `${label}: must not expose internal index maintenance`);
  assert.equal(data.normalization, undefined, `${label}: must not expose mechanical normalization`);
}

function assertExecuteTaskInstruction(data, label) {
  assert.equal(data.instruction?.mode, "execute_task", `${label}: expected execute_task instruction`);
  assert.equal(data.instruction?.autoContinue, true, `${label}: expected autoContinue`);
  assert.equal(data.instruction?.mustStartImmediately, true, `${label}: expected mustStartImmediately`);
  assert.equal(data.instruction?.mustRunImmediately, true, `${label}: expected mustRunImmediately`);
  assert.equal(data.instruction?.mustNotAskUserBeforeRunning, true, `${label}: expected mustNotAskUserBeforeRunning`);
  assert.equal(data.instruction?.mustNotAskUserBeforeExecuting, true, `${label}: expected mustNotAskUserBeforeExecuting`);
  assert.equal(data.instruction?.mustNotReportProgressBeforeExecuting, true, `${label}: expected mustNotReportProgressBeforeExecuting`);
  assert.equal(data.instruction?.requestRef, data.executionRequestPath, `${label}: instruction requestRef must match executionRequestPath`);
  assert.equal(data.instruction?.resultFile, data.executionRequest?.resultFile, `${label}: instruction resultFile must match summary`);
  assert.deepEqual(data.instruction?.submitCommand, data.executionRequest?.submitCommand, `${label}: instruction submitCommand must match summary`);
  assert.equal(data.instruction?.completionBarrier?.resultFile, data.instruction?.resultFile, `${label}: instruction must expose completion resultFile`);
  assert.equal(data.instruction?.completionBarrier?.rules, undefined, `${label}: completion barrier rules must stay in request refs`);
  assert.equal(data.instruction?.stopRecoveryInstruction, undefined, `${label}: auto-runnable instruction must not expose recovery hint as a competing action`);
  assert.equal(data.instruction?.primaryAction?.action, "execute_current_task", `${label}: missing primary execute action`);
  assert.ok(data.instruction?.completionCondition?.completeWhen?.includes("TaskResult exists"), `${label}: missing completion condition`);
  assert.equal(data.instruction?.completionContinuityRequirement, undefined, `${label}: instruction must not duplicate completion continuity rules`);
  assert.equal(data.instruction?.mustNotDuringPrimaryAction, undefined, `${label}: instruction must not duplicate primary-action rule blocks`);
  assert.equal(data.instruction?.runtimeCommandGuard, undefined, `${label}: instruction must not duplicate runtime command guard rules`);
  assert.equal(data.instruction?.runtimeForegroundProbeCloseoutRules, undefined, `${label}: instruction must not duplicate runtime closeout rules`);
  assert.equal(data.instruction?.verificationCommandSchedulingRules, undefined, `${label}: instruction must not duplicate verification scheduling rules`);
  assert.ok(data.instruction?.finalResponseGuard?.invalidFinalResponseWhen, `${label}: instruction must expose final response guard`);
  assert.ok(data.instruction?.requestReadProtocol?.readRule?.includes("agentAction.read.fieldGroups"), `${label}: instruction must route rule reads through request refs`);
  assert.ok(
    data.instruction?.routingRule?.includes("Progress-only summaries are not completion"),
    `${label}: routingRule must forbid progress-only task stops`,
  );
  assert.equal(data.instruction?.routingRule?.includes("@loom continue"), false, `${label}: routingRule must not contain recovery command`);
  assert.ok(
    !data.instruction?.userMessage?.includes("@loom continue"),
    `${label}: userMessage must not surface recovery command during primary execution`,
  );
  assert.equal(data.instruction?.task?.taskId, data.task?.taskId, `${label}: instruction task must match next task`);
  mark("L3", `${label} returned execute_task instruction`);
}

function assertTaskExecutionCompletionBarrierProtocol(request, label) {
  assert.ok(
    request.agentAction?.instruction?.includes("do not stop with a progress summary before submitCommand succeeds"),
    `${label}: agentAction must forbid progress-summary stops`,
  );
  assert.ok(
    request.agentAction?.write?.rules?.some((rule) => rule.includes("Do not send progress-only summaries")),
    `${label}: agentAction.write rules must include completion barrier`,
  );
  assert.deepEqual(
    request.agentAction?.write?.requiredTopLevelFields,
    request.outputContract?.requiredTopLevelFields,
    `${label}: agentAction and outputContract must expose the same TaskResult required top-level fields`,
  );
  for (const field of ["blockedReasons", "createdAt", "updatedAt"]) {
    assert.ok(
      request.outputContract?.requiredTopLevelFields?.includes(field),
      `${label}: outputContract must foreground required TaskResult field ${field}`,
    );
  }
  assert.ok(
    request.outputContract?.requiredTopLevelFields?.includes("executionContinuity"),
    `${label}: outputContract must foreground executionContinuity`,
  );
  assert.equal(request.agentAction?.actionRequired?.mode, "execute_task", `${label}: agentAction must expose execute_task actionRequired`);
  assert.equal(request.agentAction?.actionRequired?.mustRunImmediately, true, `${label}: agentAction actionRequired must force immediate execution`);
  assert.equal(request.agentAction?.actionRequired?.mustNotReportProgress, true, `${label}: agentAction actionRequired must forbid progress-only final responses`);
  assert.ok(
    request.agentAction?.actionRequired?.completionContinuityRequirement?.forbiddenOutcome?.includes("long-running command"),
    `${label}: agentAction actionRequired must expose execution continuity closeout`,
  );
  assert.ok(
    request.agentAction?.finalResponseGuard?.invalidFinalResponseWhen?.some((rule) => rule.includes("does not exist")),
    `${label}: agentAction must expose final response guard in ref-first request`,
  );
  assert.ok(
    request.executionRules?.completionBarrier?.rules?.some((rule) => rule.includes("not complete until TaskResult JSON exists")),
    `${label}: executionRules must expose completion barrier`,
  );
  assert.ok(
    request.executionRules?.completionBarrier?.rules?.every((rule) => !rule.includes("@loom continue")),
    `${label}: executionRules completion barrier must not mix recovery command into primary execution rules`,
  );
  assert.equal(request.executionRules?.stopRecoveryInstruction, undefined, `${label}: request executionRules must not expose recovery hint as a competing action`);
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Do not stop with a progress summary")),
    `${label}: executionRules must forbid progress-only task stops`,
  );
  assert.ok(
    request.outputContract?.resultRules?.some((rule) => rule.includes("TaskExecution is not complete")),
    `${label}: outputContract must repeat completion barrier at TaskResult boundary`,
  );
  assert.ok(
    request.outputContract?.schemaShape?.executionContinuity?.agentOwnedLongRunningWork?.includes("unknown"),
    `${label}: TaskResult schemaShape must document executionContinuity choices`,
  );
  assert.ok(
    request.outputContract?.resultRules?.some((rule) => rule.includes("executionContinuity.agentOwnedLongRunningWork is unknown")),
    `${label}: outputContract must define unknown long-running work status rule`,
  );
  mark("L2", `${label} documents TaskExecution completion barrier`);
}

function assertTaskPlanVerificationEvidenceProtocol(request, label) {
  assert.deepEqual(request.enumRefs?.verificationEvidence, [
    "automated_test",
    "manual_command_output",
    "runtime_api_check",
    "static_check",
    "agent_review_explanation",
  ], `${label}: missing verificationEvidence enum`);
  assert.ok(
    request.generationRules?.verificationEvidenceRules?.some((rule) => rule.includes("Do not copy AAC verificationHints")),
    `${label}: missing AAC verification hint copy guard`,
  );
  assert.deepEqual(request.generationRules?.aacVerificationHintToTaskPlanEvidence, {
    unit: "automated_test",
    integration: "automated_test",
    e2e: "automated_test",
    contract: "automated_test",
    manual: "manual_command_output",
    static: "static_check",
  }, `${label}: missing AAC hint to TaskPlan evidence mapping`);
  mark("L2", `${label} exposes verification evidence enum and mapping`);
}

function assertTaskExecutionEnvironmentPreparation(request, label) {
  assert.equal(
    request.executionRules?.environmentPreparation?.packageManager,
    "npm",
    `${label}: missing package-manager environment preparation`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Do not modify .loom")),
    `${label}: missing .loom boundary`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("workspace side-effect files")),
    `${label}: missing package-manager install permission rule`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Only record verification status not_run")),
    `${label}: missing not_run dependency-attempt rule`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Run verification commands serially by default")),
    `${label}: missing serial verification command scheduling rule`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Do not issue multiple tool calls")),
    `${label}: missing tool-call-level scheduling rule for write-producing verification commands`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("temporary runtime is running for a bounded probe")),
    `${label}: missing server/probe command scheduling rule`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("Never run long-lived runtime/server commands")),
    `${label}: missing foreground runtime command guard`,
  );
  assert.deepEqual(request.task?.writeBoundary?.forbiddenPaths, [".loom"], `${label}: only .loom should be hard forbidden`);
  mark("L2", `${label} documents lightweight workspace boundary and environment preparation`);
}

function assertTaskResultSelfRepairProtocol(request, label) {
  const output = request.outputContract;
  assert.ok(
    output?.resultRules?.some((rule) => rule.includes("verificationResults[].status records whether verification passed")),
    `${label}: resultRules must distinguish verification outcome from self-repair stop reason`,
  );
  assert.ok(
    output?.resultRules?.some((rule) => rule.includes("Never combine attempted=false with stopReason verification_passed")),
    `${label}: resultRules must forbid attempted=false plus verification_passed`,
  );
  assert.equal(
    output?.schemaShape?.selfRepairSummaryExamples?.noSelfRepairAttempted?.stopReason,
    "not_attempted",
    `${label}: schemaShape must show no-self-repair stopReason example`,
  );
  assert.equal(
    output?.schemaShape?.selfRepairSummaryExamples?.selfRepairAttemptedAndVerificationPassed?.attempted,
    true,
    `${label}: schemaShape must show verification_passed requires attempted=true`,
  );
  assert.ok(
    output?.schemaShape?.selfRepairSummaryRules?.some((rule) => rule.includes("even if verificationResults[].status is passed")),
    `${label}: schemaShape must repeat the passed-verification/no-self-repair rule`,
  );
  mark("L2", `${label} documents TaskResult selfRepairSummary branch rules`);
}

function assertRuntimeDeliveryEvidenceProtocol(request, label) {
  if (!request.task?.runtimeDeliveryRequirement?.appliesToThisTask) {
    return;
  }
  const requiredIds = request.task.runtimeDeliveryRequirement.requiredCodeLevelChecks.map((check) => check.checkId);
  assert.deepEqual(
    request.agentAction?.write?.requiredRuntimeEvidence?.requiredCheckIds,
    requiredIds,
    `${label}: agentAction.write must expose exact runtime checkIds before schemaShape`,
  );
  assert.deepEqual(
    request.outputContract?.requiredRuntimeEvidence?.requiredCheckIds,
    requiredIds,
    `${label}: outputContract must expose exact runtime checkIds before nested schemaShape`,
  );
  assert.ok(
    request.agentAction?.write?.rules?.some((rule) => rule.includes("Copy every exact checkId")),
    `${label}: agentAction.write rules must tell Agent to copy exact runtime checkIds`,
  );
  assert.ok(
    request.outputContract?.schemaShape?.runtimeDeliveryEvidence?.codeLevelCheckRules?.some((rule) => rule.includes("Do not invent generic runtime checkIds")),
    `${label}: schemaShape must still forbid invented runtime checkIds`,
  );
  assert.equal(
    request.executionRules?.runtimeDeliveryExecutionRules?.foregroundRuntimeCommandsForbidden,
    true,
    `${label}: runtimeDeliveryExecutionRules must forbid foreground runtime commands`,
  );
  assert.ok(
    request.executionRules?.rules?.some((rule) => rule.includes("ready URL") && rule.includes("do not wait")),
    `${label}: executionRules must include fixed ready-runtime closeout action`,
  );
  assert.ok(
    request.outputContract?.schemaShape?.runtimeDeliveryEvidence?.codeLevelCheckRules?.some((rule) => rule.includes("foreground verification commands")),
    `${label}: TaskResult schemaShape must surface foreground command guard`,
  );
  mark("L2", `${label} exposes runtimeDeliveryEvidence exact checkIds at the write boundary`);
}

function assertBrainstormConceptGroundingRequest(request) {
  const conceptGroundingRequest = request.conceptGroundingRequest;
  const blockRules = request.clarificationConversationProtocol?.blockExecutionRules ?? [];
  const blockConfirmationRules = request.clarificationConversationProtocol?.blockConfirmationRules ?? {};
  assert.ok(
    blockRules.some((rule) => rule.includes("Do not merge required clarification blocks")),
    "BrainstormSessionRequest: clarification blocks must not be mergeable",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("phase_scope option") && rule.includes("do not satisfy concept_grounding or frontend_experience")),
    "BrainstormSessionRequest: phase_scope mentions must not satisfy concept/frontend blocks",
  );
  assert.ok(
    blockConfirmationRules.concept_grounding?.includes("dedicated concept and business-rules summary"),
    "BrainstormSessionRequest: concept_grounding must require a dedicated concept and business-rules summary",
  );
  assert.ok(
    blockConfirmationRules.concept_grounding?.includes("inputs or fields") &&
      blockConfirmationRules.concept_grounding?.includes("actions or behaviors"),
    "BrainstormSessionRequest: concept_grounding must require applicable inputs/fields and actions/behaviors",
  );
  assert.ok(
    request.firstClarificationGate?.mustPresentBeforeAccept?.includes("businessObjectOperationSummary"),
    "BrainstormSessionRequest: first clarification gate must include businessObjectOperationSummary",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("applicable objects or subjects, actions or behaviors, inputs or fields")),
    "BrainstormSessionRequest: concept_grounding block must own applicable scope-detail clarification",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("map every confirmed scope.included item")),
    "BrainstormSessionRequest: concept_grounding block must map every included scope item",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("Do not use a fixed capability taxonomy")),
    "BrainstormSessionRequest: scope coverage must avoid fixed capability taxonomy",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("业务理解与规则确认") && rule.includes("Do not show internal names")),
    "BrainstormSessionRequest: concept_grounding must use user-facing title and hide internal names",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("stable user-visible section order") && rule.includes("scope-by-scope coverage")),
    "BrainstormSessionRequest: concept_grounding must use a stable readable section order",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("one separate bullet or mini-block per confirmed current-phase scope item")),
    "BrainstormSessionRequest: concept_grounding must not collapse scope coverage into prose",
  );
  assert.ok(
    blockConfirmationRules.frontend_experience?.includes("dedicated frontend target"),
    "BrainstormSessionRequest: frontend_experience must require a dedicated frontend target confirmation",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("页面办理路径确认") && rule.includes("Do not show internal names")),
    "BrainstormSessionRequest: frontend_experience must use user-facing title and hide internal names",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("query criteria as a separate labeled line")),
    "BrainstormSessionRequest: frontend_experience must surface query criteria as a separate line",
  );
  assert.ok(
    blockRules.some((rule) => rule.includes("one separate bullet or compact mini-block per operation")),
    "BrainstormSessionRequest: frontend_experience must not collapse operations into prose",
  );
  assert.ok(
    conceptGroundingRequest?.selectionGuidance?.preferConceptsAffecting?.includes("business_invariant"),
    "BrainstormSessionRequest: concept grounding must tell Agent how to select high-risk business concepts",
  );
  assert.ok(
    conceptGroundingRequest?.selectionGuidance?.includeConceptTypes?.some((item) => item.includes("Abstract concepts")),
    "BrainstormSessionRequest: concept grounding must ask for abstract cross-section concepts",
  );
  assert.ok(
    conceptGroundingRequest?.selectionGuidance?.includeConceptTypes?.some((item) => item.includes("Concrete terms")),
    "BrainstormSessionRequest: concept grounding must ask for explicit source terms",
  );
  assert.ok(
    conceptGroundingRequest?.selectionGuidance?.antiPatterns?.some((item) => item.includes("generic project label")),
    "BrainstormSessionRequest: concept grounding must prevent one generic project-label glossary",
  );
  assert.equal(
    conceptGroundingRequest?.userPresentationGuidance?.askUserToConfirmOrCorrect,
    true,
    "BrainstormSessionRequest: concept grounding must be shown to the user for confirmation",
  );
  assert.equal(
    conceptGroundingRequest?.coverageExpectation,
    undefined,
    "BrainstormSessionRequest: concept grounding must not impose count-based semantic gates",
  );
  mark("L2", "BrainstormSessionRequest provides concept extraction selection guidance without count gates");
}

function assertBrainstormCandidateRules(request) {
  assert.equal(request.rules?.preservePhase1InRoadmapPhases, true, "BrainstormSessionRequest: missing phase-1 preservation rule");
  assert.equal(request.rules?.phasePlanCurrentScopeRefsMustUseOnlyIncludedScopeIds, true, "BrainstormSessionRequest: missing current scopeRefs included-only rule");
  assert.ok(
    request.rules?.requirementSemanticGrounding?.rules?.some((rule) => rule.includes("preserve requirement semantics in existing BrainstormCandidate fields")),
    "BrainstormSessionRequest: missing requirement semantic grounding rules",
  );
  assert.ok(
    request.clarificationConversationProtocol?.blockExecutionRules?.some((rule) => rule.includes("structured BrainstormCandidate fields")),
    "BrainstormSessionRequest: confirmed details must be written into structured fields",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.requiredUserVisibleTopicsWhenApplicable?.includes("coverage checklist from confirmed phase scope including concrete included work and deferred or not-done boundaries"),
    "BrainstormSessionRequest: final_summary must include concrete scope coverage checklist topic",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.requiredUserVisibleTopicsWhenApplicable?.includes("business-rule checklist from confirmed business understanding including concrete objects, relationships, operations, field-set headlines, state changes, blocking rules, success outcomes, and high-risk misunderstanding guards when applicable"),
    "BrainstormSessionRequest: final_summary must include concrete business-rule checklist topic",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.requiredUserVisibleTopicsWhenApplicable?.includes("page-operation checklist from confirmed frontend path including surface or entry, target discovery or query selection, pagination and query criteria when confirmed, action entry, feedback, and refresh or readback when applicable"),
    "BrainstormSessionRequest: final_summary must include concrete page-operation checklist topic",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.frontendOperationPathContract?.presentationRules?.some((rule) => rule.includes("页面办理路径确认")),
    "BrainstormSessionRequest: frontend operation-path contract must carry user-facing presentation rules",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.frontendOperationPathContract?.presentationRules?.some((rule) => rule.includes("query criteria as a separate labeled line")),
    "BrainstormSessionRequest: frontend operation-path contract must require separate query-criteria display",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.requiredUserVisibleTopicsWhenApplicable?.includes("explicit final_summary corrections that must be written back to structured fields"),
    "BrainstormSessionRequest: final_summary corrections must write back to structured fields",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.finalSummaryReviewContract?.rules?.some((rule) => rule.includes("not the source of detailed requirements")),
    "BrainstormSessionRequest: final_summary review contract must not be the detail source",
  );
  assert.equal(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.confirmedBlockDetailRetentionContract?.sourceOfTruth,
    "all_confirmed_brainstorm_blocks_plus_final_summary_corrections",
    "BrainstormSessionRequest: candidate writing must retain all confirmed block details, not final_summary alone",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.confirmedBlockDetailRetentionContract?.rules?.some((rule) => rule.includes("For phase_scope, preserve")),
    "BrainstormSessionRequest: retention contract must preserve phase_scope details",
  );
  assert.ok(
    request.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.confirmedBlockDetailRetentionContract?.rules?.some((rule) => rule.includes("For frontend_experience, preserve")),
    "BrainstormSessionRequest: retention contract must preserve frontend_experience details",
  );
  assert.equal(
    request.rules?.requirementSemanticGrounding?.validationMode,
    "generation_guidance_only",
    "BrainstormSessionRequest: requirement semantic grounding must remain generation guidance, not accept validation",
  );
  assertBrainstormConceptGroundingRequest(request);
  assert.deepEqual(
    request.enumRefs?.requirementSourceType,
    ["user_text", "pdf", "word", "markdown", "text", "code", "spreadsheet", "unknown"],
    "BrainstormSessionRequest: missing source type enum refs",
  );
  if (request.contextRefs?.requirementContextRef) {
    assert.equal(
      request.sourceFieldAccessHints?.requirementContextInput?.sourceItemsSelector,
      ".sourceItems[]",
      "BrainstormSessionRequest: missing RequirementContext sourceItems selector hint",
    );
    assert.equal(
      request.sourceFieldAccessHints?.requirementContextInput?.itemIdField,
      "itemId",
      "BrainstormSessionRequest: missing RequirementContext itemId source field hint",
    );
  } else {
    assert.equal(
      request.sourceFieldAccessHints?.previousContractInput?.sourcesSelector,
      ".sources[]",
      "BrainstormSessionRequest: missing previous contract sources selector hint",
    );
    assert.equal(
      request.sourceFieldAccessHints?.previousContractInput?.sourceIdField,
      "sourceId",
      "BrainstormSessionRequest: missing previous contract sourceId field hint",
    );
  }
  assert.equal(
    request.sourceFieldAccessHints?.candidateOutput?.sourceIdField,
    "sourceId",
    "BrainstormSessionRequest: missing BrainstormCandidate sourceId output field hint",
  );
  assert.ok(
    request.sourceFieldAccessHints?.mappingRules?.some((rule) => rule.includes("sourceFieldAccessHints")),
    "BrainstormSessionRequest: missing RequirementContext-to-candidate source mapping rule",
  );
  const shape = request.outputContract?.schemaShape;
  assert.equal(
    shape?.sources?.[0]?.type,
    "user_text | pdf | word | markdown | text | code | spreadsheet | unknown",
    "BrainstormCandidate schemaShape: sources[].type must show allowed enum values",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("phasePlan.current.scopeRefs may reference only scope.included ids")),
    "BrainstormCandidate schemaShape: missing included-only scopeRefs rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("sources[].type must use exactly one enumRefs.requirementSourceType value")),
    "BrainstormCandidate schemaShape: missing source type enum rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("preserve prior roadmap phases")),
    "BrainstormCandidate schemaShape: missing prior phase preservation rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("delivery-wide high-risk concepts")),
    "BrainstormCandidate schemaShape: missing delivery glossary concept quality rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("phase_scope mentions are context only")),
    "BrainstormCandidate schemaShape: phase_scope mentions must not satisfy concept/frontend confirmation",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("Set conceptConfirmation.shownToUser=true only after a dedicated concept_grounding block")),
    "BrainstormCandidate schemaShape: missing dedicated concept confirmation rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("Set frontend_experience confirmed only after a dedicated frontend_experience block")),
    "BrainstormCandidate schemaShape: missing dedicated frontend confirmation rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("pre-submit coverage checklist")),
    "BrainstormCandidate schemaShape: missing pre-submit final_summary checklist rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("do not rely on final_summary text")),
    "BrainstormCandidate schemaShape: missing no-final-summary-detail-source rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("using existing fields")),
    "BrainstormCandidate schemaShape: missing requirement semantic preservation in existing fields",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("all user-confirmed Brainstorm blocks")),
    "BrainstormCandidate schemaShape: missing all-confirmed-block retention rule",
  );
  assert.ok(
    shape?.candidateRules?.some((rule) => rule.includes("A checklist-style final_summary does not make earlier phase_scope, concept_grounding, or frontend_experience details optional")),
    "BrainstormCandidate schemaShape: missing checklist-final-summary retention rule",
  );
  mark("L2", "BrainstormCandidate request documents Phase N roadmap/ref rules");
}

function assertRequirementDetailTransferProtocol(pgc, architectureRequest) {
  const acceptance = pgc.phaseScope.acceptanceCandidates[0];
  assert.deepEqual(acceptance.sourceRefs, ["src-001"], "PGC must preserve Brainstorm acceptance sourceRefs.");
  assert.deepEqual(acceptance.capabilityRefs, ["cap-core"], "PGC must preserve Brainstorm acceptance capabilityRefs.");
  assert.ok(
    pgc.phaseScope.included[0].items.some((item) => item.includes("validationName") && item.includes("VALIDATION_NAME_REQUIRED")),
    "PGC phaseScope included items must preserve concrete fields and blocking reasons.",
  );
  const businessFlowSummary = pgc.planningInputs.businessFlows[0].summary;
  assert.ok(
    businessFlowSummary.toLowerCase().includes("precondition") &&
      businessFlowSummary.includes("VALIDATION_NAME_REQUIRED") &&
      businessFlowSummary.includes("status=ready"),
    "PGC businessFlows summary must preserve flow steps, preconditions, blocking reason, and success state.",
  );

  const transfer = architectureRequest.contextProjection?.requirementDetailTransfer;
  assert.equal(transfer?.authority, "planning_generation_contract", "Architecture request must expose requirement detail transfer authority.");
  assert.ok(
    transfer.currentPhaseScope.included[0].items.some((item) => item.includes("validationName") && item.includes("VALIDATION_NAME_REQUIRED")),
    "Architecture requirementDetailTransfer must carry detailed scope items.",
  );
  assert.deepEqual(
    transfer.acceptanceDetails[0],
    {
      id: "AC-001",
      statement: acceptance.statement,
      priority: "must",
      capabilityRefs: ["cap-core"],
      sourceRefs: ["src-001"],
    },
    "Architecture requirementDetailTransfer must carry acceptance statements with sourceRefs/capabilityRefs.",
  );
  assert.ok(
    transfer.businessFlowDetails[0].summary.includes("validationName") &&
      transfer.businessFlowDetails[0].summary.includes("status=ready"),
    "Architecture requirementDetailTransfer must carry business flow detail.",
  );

  const planningGuide = architectureRequest.referencedArtifactReadGuide.find((entry) => entry.refKey === "planningContractRef");
  for (const selector of [
    ".phaseScope.included[].items",
    ".phaseScope.acceptanceCandidates[].sourceRefs",
    ".phaseScope.acceptanceCandidates[].capabilityRefs",
    ".planningInputs.businessFlows[].summary",
  ]) {
    assert.ok(
      planningGuide?.requiredSelectors?.includes(selector),
      `Architecture request read guide must include ${selector}`,
    );
  }

  const sectionByName = new Map(architectureRequest.outputContract.sectionOutputs.map((output) => [output.section, output]));
  assert.ok(
    sectionByName.get("foundation")?.generationRules?.some((rule) => rule.includes("current-phase module responsibility")),
    "foundation generation rules must consume requirementDetailTransfer details.",
  );
  assert.ok(
    sectionByName.get("domain_contract")?.generationRules?.some((rule) => rule.includes("current phase domain-detail source")),
    "domain_contract generation rules must consume requirementDetailTransfer details.",
  );
  assert.ok(
    sectionByName.get("behavior")?.generationRules?.some((rule) => rule.includes("current phase behavior-detail source")),
    "behavior generation rules must consume requirementDetailTransfer details.",
  );
  assert.ok(
    sectionByName.get("frontend_experience")?.generationRules?.some((rule) => rule.includes("input, display, feedback")),
    "frontend_experience generation rules must consume requirementDetailTransfer frontend expectations.",
  );
  assert.ok(
    sectionByName.get("coverage")?.generationRules?.some((rule) => rule.includes("preserve the PGC acceptance statement exactly")),
    "coverage generation rules must preserve PGC acceptance details.",
  );
  mark("L2", "PGC requirement details transfer into AAC request and section generation rules");
}

function assertBrainstormRepairRules(repairInstruction) {
  assertRepairOutputPolicy(repairInstruction, "Brainstorm");
  assert.deepEqual(
    repairInstruction?.enumRefs?.requirementSourceType,
    ["user_text", "pdf", "word", "markdown", "text", "code", "spreadsheet", "unknown"],
    "Brainstorm repairInstruction: missing source type enum refs",
  );
  assert.ok(
    repairInstruction?.instructions?.some((rule) => rule.includes("Preserve phase-1 in roadmap.phases")),
    "Brainstorm repairInstruction: missing phase-1 preservation rule",
  );
  assert.ok(
    repairInstruction?.instructions?.some((rule) => rule.includes("phasePlan.current.scopeRefs must contain only ids from scope.included")),
    "Brainstorm repairInstruction: missing included-only current scopeRefs rule",
  );
  assert.equal(
    repairInstruction?.primaryAction,
    "Edit candidateFile according to fieldRepairPlan, write a complete replacement BrainstormCandidate JSON, then run submitCommand.",
    "Brainstorm repairInstruction: missing explicit primary repair action",
  );
  assert.ok(
    Array.isArray(repairInstruction?.fieldRepairPlan) && repairInstruction.fieldRepairPlan.length > 0,
    "Brainstorm repairInstruction: missing field-level repair plan",
  );
  assert.ok(
    repairInstruction.fieldRepairPlan.every((item) => item.path && item.suggestedAction),
    "Brainstorm repairInstruction: each field repair item must include path and suggestedAction",
  );
  assert.ok(repairInstruction?.schemaShape?.candidateRules, "Brainstorm repairInstruction: missing schemaShape candidateRules");
  mark("L2", "Brainstorm repairInstruction repeats Phase N roadmap/ref rules");
}

function assertRepositoryContextSchemaShape(request) {
  const shape = request.outputContract?.schemaShape;
  assert.ok(shape, "RepositoryContextRequest: missing schemaShape");
  assert.deepEqual(
    request.enumRefs?.recommendedReadReason,
    ["implemented_capability", "dependency_context", "integration_boundary", "test_or_validation", "risk_review", "extension_point"],
    "RepositoryContextRequest: recommendedReadReason enum must be explicit",
  );
  assert.deepEqual(
    request.enumRefs?.relevantSurfaceKind,
    ["entrypoint", "module", "service", "controller", "data_access", "ui", "config", "test", "script", "documentation", "unknown"],
    "RepositoryContextRequest: relevantSurfaceKind enum must match RepositoryContext schema",
  );
  assert.equal(request.enumRefs?.surfaceType, undefined, "RepositoryContextRequest: must not expose stale surfaceType enum");
  assert.ok(
    request.outputContract?.referenceRules?.recommendedReadRefsReason?.allowedValues?.includes("test_or_validation"),
    "RepositoryContextRequest: outputContract must expose recommendedReadRefs reason enum rules",
  );
  assert.equal(
    request.outputContract?.referenceRules?.recommendedReadRefsSurfaceRefs?.mustReference,
    "relevantSurfaces[].surfaceId",
    "RepositoryContextRequest: outputContract must define surfaceRefs id domain",
  );
  assert.ok(
    request.generationRules?.some((rule) => rule.includes("surfaceRefs[] value must be a relevantSurfaces[].surfaceId")),
    "RepositoryContextRequest: generationRules must forbid path values in surfaceRefs",
  );
  assert.equal(shape.structureSignals?.entryPoints?.[0]?.path, "src/index.ts", "RepositoryContextRequest: entryPoints must show object array shape");
  assert.equal(shape.structureSignals?.entryPoints?.[0]?.kind, "module | cli | server | page | test | config | unknown", "RepositoryContextRequest: entryPoints kind enum/example missing");
  assert.ok(
    shape.structureSignals?.entryPoints?.[0]?.description?.includes("objects, not strings"),
    "RepositoryContextRequest: entryPoints must warn against string array output",
  );
  assert.equal(shape.contextQuality?.warnings?.[0]?.code, "LOW_CONFIDENCE_REPOSITORY_SCAN", "RepositoryContextRequest: contextQuality.warnings must show { code, message } object shape");
  assert.equal(shape.contextQuality?.warnings?.[0]?.message.includes("Use [] only when there are no warnings."), true, "RepositoryContextRequest: contextQuality warning example must explain empty array usage");
  assert.equal(shape.warnings?.[0]?.code, "LOW_CONFIDENCE_REPOSITORY_SCAN", "RepositoryContextRequest: warnings must show { code, message } object shape");
  mark("L2", "RepositoryContextRequest schemaShape documents object-array fields");
}

function assertRepositoryContextRepairSchemaShape(repairInstruction) {
  assertRepairOutputPolicy(repairInstruction, "RepositoryContext");
  assert.deepEqual(
    repairInstruction?.enumRefs?.recommendedReadReason,
    ["implemented_capability", "dependency_context", "integration_boundary", "test_or_validation", "risk_review", "extension_point"],
    "RepositoryContext repairInstruction: recommendedReadReason enum must be repeated",
  );
  assert.equal(
    repairInstruction?.referenceRules?.recommendedReadRefsSurfaceRefs?.mustReference,
    "relevantSurfaces[].surfaceId",
    "RepositoryContext repairInstruction: surfaceRefs id-domain rules must be repeated",
  );
  assert.ok(
    repairInstruction?.instructions?.some((rule) => rule.includes("recommendedReadRefs[].reason")),
    "RepositoryContext repairInstruction: must tell Agent to repair recommendedReadRefs reason using enumRefs",
  );
  const shape = repairInstruction?.schemaShape;
  assert.ok(shape, "RepositoryContext repairInstruction: missing schemaShape");
  assert.equal(shape.structureSignals?.entryPoints?.[0]?.path, "src/index.ts", "RepositoryContext repairInstruction: entryPoints must show object array shape");
  assert.equal(shape.contextQuality?.warnings?.[0]?.code, "LOW_CONFIDENCE_REPOSITORY_SCAN", "RepositoryContext repairInstruction: contextQuality.warnings must show object shape");
  assert.equal(shape.warnings?.[0]?.message.includes("Use [] only when there are no warnings."), true, "RepositoryContext repairInstruction: warnings must explain empty array usage");
  mark("L2", "RepositoryContext repairInstruction repeats schemaShape");
}

function brainstormCandidate(request, options = {}) {
  const includeNextPhase = options.includeNextPhase ?? false;
  const conceptId = "concept-core-validation";
  const concept = {
    conceptId,
    term: "Tiny CLI validation increment",
    normalizedName: "tiny_cli_validation_increment",
    explanation: "A minimal code increment that proves the delivery protocol without changing unrelated product scope.",
    mustNotMisinterpretAs: ["deployment work", "future phase work"],
    phaseRelevance: "current",
    priority: "must_understand",
    attentionRank: 1,
    riskFactors: ["scope_confusion_risk"],
    scopeRefs: ["scope-core"],
    acceptanceRefs: ["AC-001"],
    humanReadableReason: "Misreading this concept would make the fixture do unrelated work.",
  };
  const isPhaseOne = request.phaseId === "phase-1";
  return {
    schemaVersion: "1.0",
    candidateId: "brainstorm-candidate-layered",
    brainstormRunId: request.brainstormRunId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "confirmed",
    requestSummary: {
      title: "Layered Self Validation",
      oneLine: "Build a tiny CLI validation increment with one validated input.",
      businessGoal: "Verify Loom protocol flow without losing current-phase field, validation, blocking, and status details.",
      complexity: "small",
    },
    sources: [{ sourceId: "src-001", type: "user_text", title: "layered fixture", extracted: true }],
    scope: {
      included: [{
        id: "scope-core",
        label: "Core validation increment",
        items: [
          "Add a tiny CLI validation action that accepts the validationName field, rejects an empty validationName with blocking reason VALIDATION_NAME_REQUIRED, and returns status=ready with normalizedName when valid.",
        ],
        source: "user_confirmed",
      }],
      excluded: [{ id: "scope-deploy", label: "Deployment", items: ["deployment"], source: "user_confirmed" }],
      deferred: [],
      assumptions: [{ id: "assumption-001", text: "Local Node project is available.", requiresConfirmation: false }],
    },
    roadmap: {
      required: includeNextPhase,
      currentPhaseId: request.phaseId,
      phases: [
        {
          phaseId: request.phaseId,
          title: "Phase 1",
          status: "scope_confirmed",
          goal: "Deliver a tiny local CLI validation increment.",
          scopeRefs: ["scope-core"],
          acceptanceRefs: ["AC-001"],
          dependsOn: [],
        },
        ...(includeNextPhase
          ? [{
              phaseId: "phase-2",
              title: "Follow-up Validation",
              status: "proposed",
              goal: "Continue with a later validation phase.",
              scopeRefs: [],
              acceptanceRefs: [],
              dependsOn: [request.phaseId],
            }]
          : []),
      ],
    },
    phasePlan: {
      current: {
        phaseId: request.phaseId,
        title: "Phase 1",
        goal: "Deliver a tiny local CLI validation increment.",
        scopeRefs: ["scope-core"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        ...(includeNextPhase
          ? {
              kind: "candidate",
              suggestedPhaseId: "phase-2",
              title: "Follow-up Validation",
              goal: "Continue with a later validation phase.",
              scopePreview: ["Later validation scope"],
              reason: "Layered self-validation verifies phase activation from nextPhasePreview.",
            }
          : {
              kind: "none",
              reason: "Layered self-validation uses a single phase.",
            }),
      },
    },
    domainModel: {
      actors: [{ id: "actor-user", name: "User", description: "Runs the CLI." }],
      capabilityGroups: [{ id: "cap-core", name: "Core CLI", description: "Tiny validation capability." }],
      businessFlows: [{
        id: "flow-core",
        name: "Run CLI validation",
        actors: ["actor-user"],
        capabilityRefs: ["cap-core"],
        summary: "Flow steps: user provides validationName, system validates it is non-empty, blocks with reason VALIDATION_NAME_REQUIRED when missing, and on success returns status=ready plus normalizedName. Preconditions: local CLI entry exists. Fields: validationName input, normalizedName output, status output.",
      }],
    },
    acceptance: [{
      id: "AC-001",
      statement: "Given a validationName input, the tiny CLI validation action rejects empty validationName with VALIDATION_NAME_REQUIRED and returns status=ready with normalizedName for a valid value.",
      capabilityRefs: ["cap-core"],
      sourceRefs: ["src-001"],
      priority: "must",
    }],
    userConfirmation: {
      confirmed: true,
      confirmedAt: now(),
      confirmationSummary: "Confirmed current phase scope after reviewing the summary.",
      confirmationBasis: {
        initialRequestOnly: false,
        summaryPresentedToUser: true,
        confirmedAfterSummary: true,
        presentedItems: [
          "currentPhaseScopeSummary",
          "includedDeferredExcludedBoundary",
          "nextPhasePreview",
          "conceptSummary",
          "businessObjectOperationSummary",
          "businessDetailConfirmation",
        ],
      },
    },
    conceptGrounding: {
      ...(isPhaseOne
        ? {
            deliveryConceptGlossary: {
              mode: "concepts_present",
              concepts: [concept],
            },
          }
        : {}),
      phaseConceptGrounding: {
        mode: "concepts_present",
        concepts: [concept],
      },
      glossaryUpdates: [],
    },
    conceptConfirmation: {
      shownToUser: true,
      confirmedConceptRefs: [conceptId],
      confirmationSummary: "User confirmed the core validation concept and its boundary.",
    },
    clarificationProgress: {
      mode: "progressive_blocks",
      confirmedBlocks: [
        { block: "phase_scope", summary: "Confirmed the current phase scope.", confirmedByUser: true },
        { block: "concept_grounding", summary: "Confirmed high-risk concepts.", confirmedByUser: true },
        { block: "final_summary", summary: "Confirmed the final phase summary including validationName, VALIDATION_NAME_REQUIRED, normalizedName, and status=ready details.", confirmedByUser: true },
      ],
      skippedBlocks: [
        { block: "frontend_experience", reason: "This CLI fixture has no user-visible frontend." },
      ],
      finalSummaryConfirmed: true,
    },
    handoff: { ready: true, nextNode: "technical_baseline_generation", blockingReasons: [] },
  };
}

function technicalBaselineCandidate(request) {
  return {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-layered",
    status: "confirmed",
    source: "agent_inferred_from_repo_signals",
    projectKind: "existing_project",
    scope: "project",
    stack: {
      runtime: "node",
      language: "javascript",
      packageManager: "npm",
      test: "node tests/task/layered-self-validation.test.js",
    },
    constraints: [],
    evidence: [{ path: "package.json", reason: "Existing Node project fixture." }],
    approval: { type: "user_confirmed", reason: "Fixture confirms existing project baseline." },
    confidence: "high",
    requiresUserConfirmation: false,
    reasoningSummary: ["Existing package.json indicates a Node project."],
    alternatives: [{ name: "greenfield", tradeoff: "Not chosen because package.json already exists." }],
    createdAt: request.createdAt ?? now(),
    updatedAt: now(),
  };
}

function repositoryContextCandidate(data, root) {
  const request = requestFromCommand(data, root);
  return {
    schemaVersion: "1.0",
    repositoryContextId: "repoctx-layered",
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    source: {
      requestRef: data.requestRef,
      brainstormContractRef: request.source.brainstormContractRef,
      technicalBaselineRef: request.source.technicalBaselineRef,
    },
    requestLens: {
      projectKind: "existing_project",
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Tiny existing Node project.",
      repositoryShape: "single_package",
      primaryApplications: [{ applicationId: "app-main", name: "Main package", kind: "cli", rootPath: "." }],
    },
    technologySignals: {
      primaryLanguages: ["javascript"],
      frameworks: [],
      packageManagers: ["npm"],
      buildCommands: [],
      testCommands: [],
      notes: [],
    },
    structureSignals: {
      rootPaths: [{ path: "src", role: "source_root" }],
      entryPoints: [{ path: "src/index.js", kind: "module" }],
      configurationFiles: ["package.json"],
    },
    existingCapabilities: [],
    relevantSurfaces: [{ surfaceId: "surface-index", kind: "module", path: "src/index.js", summary: "Existing tiny module.", relevance: "implemented_capability", suggestedUse: "inspect_or_extend" }],
    recommendedReadRefs: [{ path: "src/index.js", reason: "implemented_capability", priority: "high", summary: "Primary source file.", surfaceRefs: ["surface-index"] }],
    roadmapImplications: [],
    contextQuality: { coverage: "focused", confidence: "high", warnings: [] },
    warnings: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function writeArchitectureSections(root, request, pgc) {
  const base = {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    createdAt: now(),
  };
  const contentBySection = {
    foundation: {
      source: {
        planningGenerationContractId: pgc.planningContractId,
        technicalBaselineId: "tb-layered",
        brainstormContractId: pgc.source.brainstormContractId,
        roadmapId: pgc.source.roadmapId,
        phaseId: request.phaseId,
      },
      engineeringBoundary: {
        projectKind: "existing_project",
        strategy: "extend_existing_modules",
        applications: [{ appId: "app-main", type: "cli", root: "." }],
        modules: [{ moduleId: "module-core", appId: "app-main", paths: ["src"], responsibility: "Tiny CLI validation increment." }],
        creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
      },
      modules: [{ moduleId: "module-core", name: "Core CLI", responsibility: "Expose the tiny validation increment.", dependsOn: [], scopeRefs: ["scope-core"], acceptanceRefs: ["AC-001"] }],
    },
    domain_contract: {
      dataModel: {
        entities: [{
          entityId: "entity-validation-result",
          name: "Validation Result",
          type: "value_object",
          implementationIntent: "full",
          moduleRefs: ["module-core"],
          scopeRefs: ["scope-core"],
          acceptanceRefs: ["AC-001"],
          fields: [
            { fieldId: "field-validation-name", name: "validationName", type: "string", required: true, semanticType: "input", enumValues: [] },
            { fieldId: "field-normalized-name", name: "normalizedName", type: "string", required: true, semanticType: "output", enumValues: [] },
            { fieldId: "field-status", name: "status", type: "string", required: true, semanticType: "status", enumValues: ["ready"] },
          ],
          constraints: [{
            constraintId: "constraint-validation-name-required",
            type: "business_rule",
            description: "validationName must be non-empty; otherwise block with VALIDATION_NAME_REQUIRED.",
          }],
        }],
        relationships: [],
        constraints: [{
          constraintId: "constraint-validation-success-status",
          type: "validation",
          description: "Valid validationName returns status=ready and normalizedName.",
          entityRefs: ["entity-validation-result"],
          acceptanceRefs: ["AC-001"],
        }],
      },
      interfaces: [{
        interfaceId: "interface-run-validation",
        name: "runValidation",
        type: "service_method",
        moduleRefs: ["module-core"],
        entityRefs: ["entity-validation-result"],
        scopeRefs: ["scope-core"],
        acceptanceRefs: ["AC-001"],
        requestSchema: [{ fieldId: "request-validation-name", name: "validationName", type: "string", required: true, semanticType: "input", enumValues: [] }],
        responseSchema: [
          { fieldId: "response-normalized-name", name: "normalizedName", type: "string", required: true, semanticType: "output", enumValues: [] },
          { fieldId: "response-status", name: "status", type: "string", required: true, semanticType: "status", enumValues: ["ready"] },
        ],
        errorSchema: [{ fieldId: "error-code", name: "errorCode", type: "string", required: true, semanticType: "error_code", enumValues: ["VALIDATION_NAME_REQUIRED"] }],
      }],
    },
    behavior: {
      userFlows: [{
        flowId: "flow-run-validation",
        name: "Run CLI validation",
        kind: "cli_flow",
        moduleRefs: ["module-core"],
        interfaceRefs: ["interface-run-validation"],
        entityRefs: ["entity-validation-result"],
        scopeRefs: ["scope-core"],
        acceptanceRefs: ["AC-001"],
        entry: { type: "command", ref: "interface-run-validation", label: "run validation" },
        steps: [{
          stepId: "step-submit-validation-name",
          actor: "User",
          action: "Provide validationName to the validation action.",
          systemResponse: "Return status=ready with normalizedName, or block with VALIDATION_NAME_REQUIRED when validationName is empty.",
          interfaceRefs: ["interface-run-validation"],
          stateMachineRefs: [],
        }],
        outcomes: [
          { type: "success", description: "status=ready and normalizedName is returned." },
          { type: "error", description: "Empty validationName is blocked.", errorCode: "VALIDATION_NAME_REQUIRED" },
        ],
      }],
      stateMachines: [],
    },
    frontend_experience: {
      frontendExperience: {
        required: false,
        kind: "none",
        experienceLevel: "none",
        surfaces: [],
        navigation: { required: false, pattern: "none", items: [] },
        interactionStates: [],
        mustNot: [],
        notes: ["No frontend in this CLI fixture."],
      },
    },
    runtime_delivery: {
      runtimeDelivery: {
        status: "not_applicable",
        contractVersion: "phase-1-v1",
        runtimeKind: "api_only",
        basis: {
          previousRuntimeDeliveryRef: null,
          reason: "Layered self validation fixture is not a deployable app.",
        },
      },
    },
    coverage: {
      acceptanceMatrix: [{
        acceptanceId: "AC-001",
        priority: "must",
        statement: "Given a validationName input, the tiny CLI validation action rejects empty validationName with VALIDATION_NAME_REQUIRED and returns status=ready with normalizedName for a valid value.",
        coverageStatus: "covered",
        coverage: [
          { type: "module", refs: ["module-core"], description: "Core module covers the increment." },
          { type: "data_entity", refs: ["entity-validation-result"], description: "Validation Result carries validationName, normalizedName, and status fields." },
          { type: "data_constraint", refs: ["constraint-validation-name-required", "constraint-validation-success-status"], description: "Constraints carry required input and success state rules." },
          { type: "interface", refs: ["interface-run-validation"], description: "Interface carries request, response, and error schemas." },
          { type: "user_flow", refs: ["flow-run-validation"], description: "Flow carries the user action, blocking reason, and success outcome." },
        ],
        verificationHints: [{ kind: "static", description: "Verify validationName handling, VALIDATION_NAME_REQUIRED blocking, and status=ready output in source file and TaskResult." }],
      }],
      detailCoverage: (pgc.requirementDetails?.items ?? [])
        .filter((item) => item.requiredForCurrentPhase)
        .map((item) => ({
          detailId: item.detailId,
          coverageStatus: "covered",
          artifactRefs: {
            modules: ["module-core"],
            entities: ["entity-validation-result"],
            fields: ["field-validation-name", "field-normalized-name", "field-status"],
            constraints: ["constraint-validation-name-required", "constraint-validation-success-status"],
            interfaces: ["interface-run-validation"],
            userFlows: ["flow-run-validation"],
            stateMachines: [],
            frontendDataViews: [],
            frontendActions: [],
            frontendOperationPaths: [],
            acceptanceMatrix: ["AC-001"],
          },
        })),
      risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
      handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    },
  };
  for (const output of request.outputContract.sectionOutputs) {
    writeJson(projectFile(root, output.candidateFile), {
      ...base,
      section: output.section,
      content: contentBySection[output.section],
      blockedReasons: [],
    });
  }
}

function writeTaskPlanGroupedOutputs(root, request) {
  const requirementDetailRefs = (request.contextProjection?.requirementDetailTransfer?.requirementDetailAssignment?.items ?? [])
    .filter((item) => item.coverageStatus === "covered")
    .map((item) => item.detailId);
  const outline = {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    taskPlanId: "taskplan-layered",
    groups: [{
      groupId: "group-core",
      title: "Core validation increment",
      objective: "Deliver the tiny CLI validation increment.",
      dependsOn: [],
      scopeRefs: ["scope-core"],
      acceptanceRefs: ["AC-001"],
      taskIds: ["task-core"],
    }],
    createdAt: now(),
  };
  const group = {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    group: outline.groups[0],
    tasks: [{
      taskId: "task-core",
      groupId: "group-core",
      title: "Add tiny validation export",
      taskKind: "feature_increment",
      implementationActions: ["create_or_update_interface", "create_or_update_business_rule", "add_or_update_tests"],
      objective: "Implement validationName handling: reject empty validationName with VALIDATION_NAME_REQUIRED and return status=ready with normalizedName for valid input.",
      dependsOn: [],
      scopeRefs: ["scope-core"],
      acceptanceRefs: ["AC-001"],
      requirementDetailRefs,
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: ["module-core"],
          entities: ["entity-validation-result"],
          interfaces: ["interface-run-validation"],
          userFlows: ["flow-run-validation"],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "VI-core",
        acceptanceRefs: ["AC-001"],
        requirementDetailRefs,
        behavior: "Given validationName, verify empty input is blocked with VALIDATION_NAME_REQUIRED and valid input returns status=ready with normalizedName.",
        preferredEvidence: ["static_check"],
        acceptableEvidence: ["static_check", "agent_review_explanation"],
      }],
      conceptRefs: ["concept-core-validation"],
      conceptResponsibilities: [{
        conceptRef: "concept-core-validation",
        responsibility: "Keep the implementation limited to the tiny CLI validation increment.",
      }],
      conceptVerificationIntents: [{
        conceptRef: "concept-core-validation",
        evidenceType: "code",
        intent: "Show the concrete source file that implements the small validation increment.",
      }],
    }],
    createdAt: now(),
  };
  writeJson(projectFile(root, request.outputContract.outlineFile), outline);
  writeJson(projectFile(root, request.outputContract.groupFilePattern.replace("{groupId}", "group-core")), group);
  return {
    outlineFile: request.outputContract.outlineFile,
    groupFile: request.outputContract.groupFilePattern.replace("{groupId}", "group-core"),
  };
}

function taskResult(request) {
  return {
    schemaVersion: "1.0",
    taskResultId: "result-layered",
    taskId: request.source.taskId,
    taskPlanId: request.source.taskPlanId,
    status: "completed",
    changedFiles: [
      "src/layered.js",
      "./src/layered.js",
      "package-lock.json",
      "node_modules/example/index.js",
      "coverage/lcov.info",
      "debug.log",
    ],
    noChangeReason: null,
    verificationResults: [{
      verificationId: request.task.verificationIntents[0].verificationId,
      status: "passed",
      evidenceType: "static_check",
      summary: "src/layered.js exists and exports a validation value.",
    }],
    selfRepairSummary: null,
    failure: null,
    executionContinuity: {
      taskResultSubmittedAfterVerification: true,
      agentOwnedLongRunningWork: "none",
      notes: [],
    },
    notes: [],
    requirementDetailEvidence: (request.task.requirementDetailRefs ?? []).map((detailId) => ({
      detailId,
      status: "satisfied",
      verificationIds: request.task.verificationIntents
        .filter((intent) => (intent.requirementDetailRefs ?? []).includes(detailId))
        .map((intent) => intent.verificationId),
      evidenceRefs: ["src/layered.js", request.task.verificationIntents[0].verificationId],
      summary: "The validation detail is implemented and verified by the layered fixture source and static verification intent.",
    })),
    conceptEvidence: [{
      conceptRef: "concept-core-validation",
      evidenceType: "code",
      refs: ["src/layered.js"],
      summary: "src/layered.js contains the tiny validation increment only.",
    }],
    blockedReasons: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function reviewResult(request) {
  const taskResultId = request.outputContract.allowedRefs.taskResultIds[0];
  return {
    schemaVersion: "1.0",
    reviewId: request.requestId,
    source: {
      requestId: request.requestId,
      phaseId: request.source.phaseId,
      taskPlanId: request.source.taskPlanId,
      taskPlanRunId: request.source.taskPlanRunId,
    },
    decision: "approved",
    findings: [],
    coverageAssessment: {
      mustAcceptance: request.reviewScope.acceptanceRefs.map((acceptanceRef) => ({
        acceptanceRef,
        status: "satisfied",
        supportingTaskResults: [taskResultId],
        evidenceStatus: "sufficient",
        notes: ["Layered self-validation fixture satisfied this acceptance."],
      })),
      summary: {
        totalMust: request.reviewScope.acceptanceRefs.length,
        satisfied: request.reviewScope.acceptanceRefs.length,
        insufficientEvidence: 0,
        notSatisfied: 0,
        notReviewed: 0,
      },
    },
    limitations: [],
    pendingActions: [],
    nextAction: {
      type: "done",
      reason: "Layered self-validation approved the current phase.",
      targetNode: "done",
    },
    createdAt: now(),
    updatedAt: now(),
  };
}

function reviewResultContinueToPhase(request, targetPhaseId) {
  return {
    ...reviewResult(request),
    nextAction: {
      type: "continue_to_next_phase",
      reason: `Layered self-validation approved ${request.source.phaseId} and should activate ${targetPhaseId}.`,
      targetNode: "continue_to_next_phase",
      targetPhaseId,
    },
  };
}

function warningOnlyReviewResult(request) {
  return {
    ...reviewResultContinueToPhase(request, "phase-2"),
    decision: "approved_with_notes",
    findings: [{
      findingId: "finding-warning-001",
      severity: "major",
      severityClass: "warning",
      evidenceKind: "static",
      failureClass: "subjective_quality",
      category: "frontend_experience",
      summary: "Synthetic non-blocking frontend quality warning.",
      evidence: "This fixture verifies severityClass controls routing rather than legacy severity alone.",
      readRefs: [{
        type: "review_packet",
        ref: request.reviewPacketRef,
        reason: "Review packet is the requested review source.",
      }],
      evidenceRefs: [],
      groupRefs: request.reviewScope.groupIds,
      taskRefs: request.outputContract.allowedRefs.taskIds,
      acceptanceRefs: request.reviewScope.acceptanceRefs,
      artifactRefs: {
        modules: [],
        entities: [],
        interfaces: [],
        userFlows: [],
        stateMachines: [],
        decisions: [],
        risks: [],
      },
      location: { file: null, line: null, diffRef: null },
      taskRelevance: "direct",
      scopeRelation: "within_task_changed_files",
      introducedByCurrentTask: "unknown",
      recommendedNextAction: "continue_to_next_phase",
    }],
  };
}

function manualReviewResult(request) {
  const taskResultId = request.outputContract.allowedRefs.taskResultIds[0];
  return {
    schemaVersion: "1.0",
    reviewId: request.requestId,
    source: {
      requestId: request.requestId,
      phaseId: request.source.phaseId,
      taskPlanId: request.source.taskPlanId,
      taskPlanRunId: request.source.taskPlanRunId,
    },
    decision: "blocked",
    findings: [{
      findingId: "finding-env-001",
      severity: "major",
      category: "environment_or_dependency",
      summary: "Verification environment requires manual confirmation.",
      evidence: "Layered self-validation synthetic review limitation.",
      readRefs: [{
        type: "review_packet",
        ref: request.reviewPacketRef,
        reason: "Review packet confirms task result evidence.",
      }],
      evidenceRefs: [{
        type: "task_result",
        ref: taskResultId,
        reason: "Task result requires manual confirmation.",
      }],
      groupRefs: request.reviewScope.groupIds,
      taskRefs: request.outputContract.allowedRefs.taskIds,
      acceptanceRefs: request.reviewScope.acceptanceRefs,
      artifactRefs: {
        modules: [],
        entities: [],
        interfaces: [],
        userFlows: [],
        stateMachines: [],
        decisions: [],
        risks: [],
      },
      location: {
        file: null,
        line: null,
        diffRef: null,
      },
      taskRelevance: "direct",
      scopeRelation: "within_task_changed_files",
      introducedByCurrentTask: "unknown",
      recommendedNextAction: "manual_review",
    }],
    coverageAssessment: {
      mustAcceptance: request.reviewScope.acceptanceRefs.map((acceptanceRef) => ({
        acceptanceRef,
        status: "insufficient_evidence",
        supportingTaskResults: [taskResultId],
        evidenceStatus: "insufficient",
        notes: ["Synthetic manual review gate."],
      })),
      summary: {
        totalMust: request.reviewScope.acceptanceRefs.length,
        satisfied: 0,
        insufficientEvidence: request.reviewScope.acceptanceRefs.length,
        notSatisfied: 0,
        notReviewed: 0,
      },
    },
    limitations: [],
    pendingActions: [],
    nextAction: {
      type: "manual_review",
      reason: "Synthetic manual review gate.",
      targetNode: "manual_review",
      findingRefs: ["finding-env-001"],
    },
    createdAt: now(),
    updatedAt: now(),
  };
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-layered-self-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "commonjs", scripts: { test: "node -e \"require('./src')\"" } }, null, 2));
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/index.js"), "module.exports = { ok: true };\n");

    const planCompactRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-plan-compact-"));
    const planCompact = runCompact(["plan", "--request", "Build a tiny status page."], planCompactRoot);
    assertPlanCompactBrainstormInstruction(planCompact);
    const planCompactRequest = hydrateRequest(planCompactRoot, readJson(projectFile(planCompactRoot, planCompact.data.requestPath)));
    assertRequestProtocol(planCompactRequest, "Plan BrainstormSessionRequest");
    assertBrainstormRequestReadPlan(planCompactRequest, "Plan BrainstormSessionRequest");
    assertRequestOutputParentDirsExist(planCompactRoot, planCompactRequest, "Plan BrainstormSessionRequest");
    mark("L3", "plan --compact returns ref-first Brainstorm instruction");

    const brainstormStartCompactRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-start-compact-"));
    run(["init"], brainstormStartCompactRoot);
    const brainstormStartCompact = runCompact(["brainstorm", "start", "--request", "Build a tiny status page."], brainstormStartCompactRoot);
    assertBrainstormStartInstruction(brainstormStartCompact.instruction, brainstormStartCompact.data.requestPath, "brainstorm start --compact");
    const brainstormStartCompactRequest = hydrateRequest(brainstormStartCompactRoot, readJson(projectFile(brainstormStartCompactRoot, brainstormStartCompact.data.requestPath)));
    assertRequestOutputParentDirsExist(brainstormStartCompactRoot, brainstormStartCompactRequest, "brainstorm start --compact BrainstormSessionRequest");
    mark("L3", "brainstorm start --compact returns ref-first Brainstorm instruction");

    run(["init"], root);
    mark("L1", "init created .loom state");

    const started = run(["brainstorm", "start", "--request", "Add a tiny CLI validation increment."], root);
    assertBrainstormStartInstruction(started.instruction, started.requestPath, "BrainstormSessionRequest");
    const startedRequest = requestFromCommand(started, root);
    assertRequestProtocol(startedRequest, "BrainstormSessionRequest");
    assertBrainstormRequestReadPlan(startedRequest, "BrainstormSessionRequest");
    assertBrainstormCandidateRules(startedRequest);
    assertRequestOutputParentDirsExist(root, startedRequest, "BrainstormSessionRequest");
    assert.equal(startedRequest.contextRefs.originalRequirementContextRef, startedRequest.contextRefs.requirementContextRef, "BrainstormSessionRequest must expose originalRequirementContextRef alias.");
    const acceptedBrainstormCandidate = brainstormCandidate(startedRequest, { includeNextPhase: true });
    writeJson(projectFile(root, startedRequest.outputContract.candidateFile), acceptedBrainstormCandidate);
    const brainstormAccepted = run([
      "brainstorm", "accept",
      "--delivery-id", started.deliveryId,
      "--phase-id", started.phaseId,
      "--request-id", started.requestId,
      "--run-id", started.brainstormRunId,
      "--candidate-file", startedRequest.outputContract.candidateFile,
    ], root);
    assert.equal(brainstormAccepted.accepted, true);
    assertAutoRunInstruction(brainstormAccepted, "technical_baseline_request", "Brainstorm accept");
    const indexAfterBrainstormAccept = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`));
    const phase1AfterBrainstormAccept = indexAfterBrainstormAccept.phases.find((phase) => phase.phaseId === started.phaseId);
    assert.ok(phase1AfterBrainstormAccept.latestRefs.brainstormDecision, "Brainstorm accept must write phase decision ref.");
    assert.ok(phase1AfterBrainstormAccept.latestRefs.brainstormDecisionsIndex, "Brainstorm accept must write decisions index ref.");
    const phase1Decision = readJson(projectFile(root, phase1AfterBrainstormAccept.latestRefs.brainstormDecision));
    assert.equal(phase1Decision.phaseId, started.phaseId);
    assert.deepEqual(phase1Decision.scope, acceptedBrainstormCandidate.scope);
    assert.ok(phase1Decision.domainModel.businessFlows.length > 0, "Brainstorm decision snapshot must preserve domainModel business flows.");
    const phase1Contract = readJson(projectFile(root, brainstormAccepted.contractPath));
    assert.equal(
      phase1Contract.userConfirmation.confirmationSummary,
      acceptedBrainstormCandidate.userConfirmation.confirmationSummary,
      "Brainstorm contract must preserve userConfirmation for downstream readers.",
    );
    const decisionIndex = readJson(projectFile(root, phase1AfterBrainstormAccept.latestRefs.brainstormDecisionsIndex));
    assert.equal(decisionIndex.latestConfirmedPhaseId, started.phaseId);
    assert.equal(decisionIndex.decisions[0].decisionRef, phase1AfterBrainstormAccept.latestRefs.brainstormDecision);
    assert.deepEqual(
      indexAfterBrainstormAccept.phases.map((phase) => phase.phaseId),
      [started.phaseId],
      "Brainstorm accept must not eagerly materialize future phases from roadmap.phases",
    );
    mark("L1", "BrainstormCandidate accept");

    const offsetDateRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-offset-date-"));
    fs.writeFileSync(projectFile(offsetDateRoot, "package.json"), JSON.stringify({ type: "commonjs" }, null, 2));
    run(["init"], offsetDateRoot);
    const offsetDateStarted = run(["brainstorm", "start", "--request", "Verify timezone normalization."], offsetDateRoot);
    const offsetDateRequest = requestFromCommand(offsetDateStarted, offsetDateRoot);
    assertRequestOutputParentDirsExist(offsetDateRoot, offsetDateRequest, "offset-date BrainstormSessionRequest");
    const offsetDateCandidate = brainstormCandidate(offsetDateRequest);
    offsetDateCandidate.userConfirmation.confirmedAt = "2026-05-25T20:51:00.000+08:00";
    writeJson(projectFile(offsetDateRoot, offsetDateRequest.outputContract.candidateFile), offsetDateCandidate);
    const offsetDateAccepted = run([
      "brainstorm", "accept",
      "--delivery-id", offsetDateStarted.deliveryId,
      "--phase-id", offsetDateStarted.phaseId,
      "--request-id", offsetDateStarted.requestId,
      "--run-id", offsetDateStarted.brainstormRunId,
      "--candidate-file", offsetDateRequest.outputContract.candidateFile,
    ], offsetDateRoot);
    assert.equal(offsetDateAccepted.accepted, true);
    const offsetDateContract = readJson(projectFile(offsetDateRoot, offsetDateAccepted.contractPath));
    assert.equal(offsetDateContract.handoff.confirmedAt, "2026-05-25T12:51:00.000Z");
    assert.equal(offsetDateContract.userConfirmation.confirmedAt, "2026-05-25T12:51:00.000Z");
    mark("L1", "BrainstormCandidate accept normalizes confirmedAt timezone offsets");

    const invalidPhase2Candidate = brainstormCandidate({
      ...startedRequest,
      phaseId: "phase-2",
    });
    invalidPhase2Candidate.phaseId = "phase-2";
    invalidPhase2Candidate.roadmap.currentPhaseId = "phase-2";
    invalidPhase2Candidate.roadmap.phases = [{
      phaseId: "phase-2",
      title: "Phase 2",
      status: "scope_confirmed",
      goal: "Phase 2 goal.",
      scopeRefs: ["scope-core"],
      acceptanceRefs: ["AC-001"],
      dependsOn: ["phase-1"],
    }];
    invalidPhase2Candidate.phasePlan.current.phaseId = "phase-2";
    invalidPhase2Candidate.scope.excluded.push({ id: "scope-excluded-in-current", label: "Excluded", items: ["excluded"], source: "user_confirmed" });
    invalidPhase2Candidate.phasePlan.current.scopeRefs.push("scope-excluded-in-current");
    const invalidPhase2File = startedRequest.outputContract.candidateFile.replace(/candidate\.json$/, "invalid-phase2-candidate.json");
    writeJson(projectFile(root, invalidPhase2File), invalidPhase2Candidate);
    const invalidBrainstormEnvelope = run([
      "brainstorm", "accept",
      "--delivery-id", started.deliveryId,
      "--phase-id", "phase-2",
      "--request-id", started.requestId,
      "--run-id", started.brainstormRunId,
      "--candidate-file", invalidPhase2File,
    ], root, { returnEnvelope: true });
    const invalidBrainstorm = invalidBrainstormEnvelope.data;
    assert.equal(invalidBrainstorm.accepted, false);
    assertBrainstormRepairRules(invalidBrainstorm.repairInstruction);
    assert.equal(invalidBrainstormEnvelope.actionRequired.mode, "repair_candidate");
    assert.ok(
      Array.isArray(invalidBrainstormEnvelope.actionRequired.fieldRepairPlan) && invalidBrainstormEnvelope.actionRequired.fieldRepairPlan.length > 0,
      "Brainstorm repair actionRequired must expose fieldRepairPlan.",
    );

    let decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "technical_baseline_request");
    assertNoMechanicalMaintenanceFields(decision, "continue to TechnicalBaselineRequest");
    mark("L3", "continue routes to TechnicalBaselineRequest");

    const baselineRequest = run(["technical-baseline", "request", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--project-kind", "existing_project"], root);
    assertInstructionOutputPolicy(baselineRequest.instruction, "TechnicalBaselineRequest");
    const baselineRequestBody = requestFromCommand(baselineRequest, root);
    assertStatusRoutesToActiveRequest(root, started.deliveryId, started.phaseId, {
      label: "TechnicalBaselineRequest",
      type: "technical_baseline_request",
      requestRef: baselineRequest.instruction.requestRef,
      operationType: "technical_baseline_generation",
    });
    assert.equal(baselineRequest.request, undefined, "TechnicalBaseline request stdout must stay compact");
    assertRequestProtocol(baselineRequestBody, "TechnicalBaselineRequest");
    assertRequestOutputParentDirsExist(root, baselineRequestBody, "TechnicalBaselineRequest");
    {
      const baselineReadFields = baselineRequestBody.agentAction.read.fieldGroups.flatMap((group) => group.fields);
      assert.equal(baselineReadFields.includes("contextRefs.latestRepositoryContextRef"), false, "TechnicalBaselineRequest must not read absent optional latestRepositoryContextRef.");
      assert.equal(baselineReadFields.includes("contextRefs.previousTechnicalBaselineRef"), false, "TechnicalBaselineRequest must not read absent optional previousTechnicalBaselineRef.");
    }
    writeJson(projectFile(root, baselineRequestBody.outputContract.candidateFile), technicalBaselineCandidate(baselineRequestBody));
    const baselineAccepted = run(["technical-baseline", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--candidate-file", baselineRequestBody.outputContract.candidateFile], root);
    assert.equal(baselineAccepted.accepted, true);
    assertAutoRunInstruction(baselineAccepted, "repository_context_request", "TechnicalBaseline accept");
    mark("L1", "TechnicalBaseline accept");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "repository_context_request");
    assertNoMechanicalMaintenanceFields(decision, "continue to RepositoryContextRequest");
    mark("L3", "continue routes to RepositoryContextRequest");

    const repoRequest = run(["repository-context", "request", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    assertInstructionOutputPolicy(repoRequest.instruction, "RepositoryContextRequest");
    const repoRequestBody = requestFromCommand(repoRequest, root);
    assert.equal(repoRequest.request, undefined, "RepositoryContextRequest stdout must stay compact");
    assertRequestProtocol(repoRequestBody, "RepositoryContextRequest");
    assertRepositoryContextSchemaShape(repoRequestBody);
    assertRequestOutputParentDirsExist(root, repoRequestBody, "RepositoryContextRequest");
    const invalidRepoCandidate = repositoryContextCandidate(repoRequest, root);
    invalidRepoCandidate.structureSignals.entryPoints = ["src/index.js"];
    writeJson(projectFile(root, repoRequest.candidateFile), invalidRepoCandidate);
    const invalidRepoAccepted = run(["repository-context", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--request-id", repoRequest.requestId, "--candidate-file", repoRequest.candidateFile], root);
    assert.equal(invalidRepoAccepted.operation, "repository_context_invalid_candidate");
    assertRepositoryContextRepairSchemaShape(invalidRepoAccepted.repairInstruction);
    mark("L1", "RepositoryContext invalid candidate returns repair schemaShape");
    const invalidSurfaceRefCandidate = repositoryContextCandidate(repoRequest, root);
    invalidSurfaceRefCandidate.recommendedReadRefs[0].surfaceRefs = ["src/index.js"];
    writeJson(projectFile(root, repoRequest.candidateFile), invalidSurfaceRefCandidate);
    const invalidSurfaceRefAccepted = run(["repository-context", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--request-id", repoRequest.requestId, "--candidate-file", repoRequest.candidateFile], root);
    assert.equal(invalidSurfaceRefAccepted.operation, "repository_context_invalid_candidate");
    assert.ok(
      invalidSurfaceRefAccepted.issues.some((issue) => issue.code === "UNKNOWN_SURFACE_REF" && issue.path.includes("recommendedReadRefs")),
      "RepositoryContext validation must reject file paths in recommendedReadRefs.surfaceRefs",
    );
    assertRepositoryContextRepairSchemaShape(invalidSurfaceRefAccepted.repairInstruction);
    writeJson(projectFile(root, repoRequest.candidateFile), repositoryContextCandidate(repoRequest, root));
    const repoAccepted = run(["repository-context", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--request-id", repoRequest.requestId, "--candidate-file", repoRequest.candidateFile], root);
    assert.equal(repoAccepted.operation, "repository_context_accepted");
    assertAutoRunInstruction(repoAccepted, "planning_contract_create", "RepositoryContext accept");
    mark("L1", "RepositoryContext accept");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "planning_contract_create");
    assertNoMechanicalMaintenanceFields(decision, "continue to PGC create");
    mark("L3", "continue routes to PGC create");

    const pgc = run(["planning-contract", "create", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    assert.equal(pgc.status, "ready");
    assert.equal(pgc.contract.contextRefs?.normalizedBrainstormRunIdFrom, undefined);
    assert.ok(
      pgc.contract.phaseScope.acceptanceCandidates[0].statement.includes("VALIDATION_NAME_REQUIRED"),
      "PlanningGenerationContract must preserve detailed Brainstorm acceptance statement.",
    );
    assertAutoRunInstruction(pgc, "architecture_artifact_contract", "PlanningGenerationContract create");
    mark("L1", "PlanningGenerationContract create");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "architecture_artifact_contract");
    assertNoMechanicalMaintenanceFields(decision, "continue to ArchitectureSectionsGenerationRequest");
    mark("L3", "continue routes to ArchitectureSectionsGenerationRequest");

    const arch = run(["architecture", "request", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    assertInstructionOutputPolicy(arch.instruction, "ArchitectureSectionsGenerationRequest");
    const archRequestRaw = readJson(projectFile(root, arch.instruction.requestRef));
    assert.equal(archRequestRaw.contextProjection, undefined, "Architecture request root must not inline contextProjection.");
    assert.ok(archRequestRaw.contextProjectionRef, "Architecture request must store contextProjection as a request manifest ref.");
    const archRequestBody = requestFromCommand(arch, root);
    assertStatusRoutesToActiveRequest(root, started.deliveryId, started.phaseId, {
      label: "ArchitectureSectionsGenerationRequest",
      type: "architecture_artifact_contract",
      requestRef: arch.instruction.requestRef,
      operationType: "architecture_generation",
    });
    assert.equal(arch.request, undefined, "Architecture request stdout must stay compact");
    assertRequestProtocol({ ...archRequestBody, outputContract: { schemaShape: archRequestBody.outputContract.sectionOutputs[0].schemaShape } }, "ArchitectureSectionsGenerationRequest");
    assertRequestOutputParentDirsExist(root, archRequestBody, "ArchitectureSectionsGenerationRequest");
    assert.ok(
      archRequestBody.requestManifest?.refs?.outputContract?.rule?.includes("section-schemas.json"),
      "Architecture request manifest must make outputContractRef the section schema authority and forbid guessed section sidecars",
    );
    assert.ok(
      archRequestBody.agentAction.write.rules.some((rule) => rule.includes("Single-section protocol")),
      "ArchitectureSectionsGenerationRequest must expose single-section protocol in agentAction.write.rules",
    );
    assert.equal(
      archRequestBody.agentAction.write.rules.some((rule) => rule === "Write one candidate per outputContract.sectionOutputs[] entry."),
      false,
      "ArchitectureSectionsGenerationRequest must not expose ambiguous write-all section rule",
    );
    assert.ok(
      archRequestBody.agentAction.write.rules.some((rule) => rule.includes("completionBarrier.followUpCommand.commandInvocation")),
      "ArchitectureSectionsGenerationRequest must tell Agent to continue immediately after each target section",
    );
    assert.equal(
      archRequestBody.outputContract.allowedRefsUsage.exactOnly,
      true,
      "ArchitectureSectionsGenerationRequest must expose mechanical allowedRefs usage in outputContract",
    );
    assert.ok(
      archRequestBody.agentAction.write.rules.some((rule) => rule.includes("allowedRefs.acceptanceRefs") && rule.includes("inventing AC ids")),
      "ArchitectureSectionsGenerationRequest must forbid invented acceptance refs before section generation",
    );
    assert.ok(
      archRequestBody.enumRefs.section.includes("frontend_experience") && archRequestBody.enumRefs.section.includes("runtime_delivery"),
      "ArchitectureSectionsGenerationRequest section enum refs must include all current sections",
    );
    const domainOutput = archRequestBody.outputContract.sectionOutputs.find((output) => output.section === "domain_contract");
    const behaviorOutput = archRequestBody.outputContract.sectionOutputs.find((output) => output.section === "behavior");
    const coverageOutput = archRequestBody.outputContract.sectionOutputs.find((output) => output.section === "coverage");
    assertRequirementDetailTransferProtocol(pgc.contract, archRequestBody);
    assert.ok(
      domainOutput?.generationRules?.some((rule) => rule.includes("earlier phase") && rule.includes("implementationIntent=reference_only")),
      "domain_contract generation rules must explain current-phase projections for earlier phase entities",
    );
    assert.ok(
      behaviorOutput?.generationRules?.some((rule) => rule.includes("current domain_contract")),
      "behavior generation rules must require entity/interface refs from current domain_contract",
    );
    assert.ok(
      domainOutput?.generationRules?.some((rule) => rule.includes("allowedRefs.scopeRefs")),
      "section generation rules must inherit exact allowedRefs scope rules",
    );
    const runtimeOutput = archRequestBody.outputContract.sectionOutputs.find((output) => output.section === "runtime_delivery");
    assert.ok(
      runtimeOutput?.schemaShape?.content?.fieldPresenceMatrix?.omitWhenNotApplicableNeverNull?.includes("api"),
      "runtime_delivery section schema must include omit-vs-null field presence matrix",
    );
    assert.ok(
      coverageOutput?.generationRules?.some((rule) => rule.includes("current phase AAC")),
      "coverage generation rules must forbid direct old-phase AAC refs",
    );
    decision = run(["continue"], root);
    assert.equal(decision.instruction.mode, "generate_candidate");
    assert.equal(decision.instruction.candidateKind, "ArchitectureSections");
    assertNoMechanicalMaintenanceFields(decision, "active architecture lease recovery");
    assertInstructionOutputPolicy(decision.instruction, "ArchitectureSections continue recovery");
    assert.equal(decision.instruction.mustRunImmediately, true, "ArchitectureSections continue recovery must be immediate");
    assert.equal(decision.instruction.mustNotAskUserBeforeExecuting, true, "ArchitectureSections continue recovery must not ask before execution");
    assert.equal(decision.instruction.mustNotReportProgressBeforeExecuting, true, "ArchitectureSections continue recovery must not report progress before execution");
    assert.equal(decision.instruction.outputSummary, undefined, "ArchitectureSections continue recovery must not expose progress summary as the agent-facing instruction");
    assert.deepEqual(decision.instruction.completionBarrier?.followUpCommand?.argv, ["continue"], "ArchitectureSections continue recovery must expose continue as the completion follow-up");
    assert.equal(decision.instruction.completionBarrier?.followUpCommand?.commandInvocation?.kind, "loom_user_launcher", "ArchitectureSections continue recovery follow-up must use adapter launcher");
    assert.equal(decision.instruction.completionBarrier?.rules, undefined, "ArchitectureSections completion barrier rules must stay out of the agent-facing stdout");
    assert.equal(decision.instruction.continuationContract?.userVisibleMeaning?.notAStoppingPoint, true, "ArchitectureSections continuation contract must mark the step as non-final");
    const refreshedArchRequest = requestFromCommand({ requestPath: arch.instruction.requestRef }, root);
    assertRequestOutputParentDirsExist(root, refreshedArchRequest, "refreshed ArchitectureSectionsGenerationRequest");
    assert.equal(refreshedArchRequest.agentAction.schema.enumLocation, "agentAction.write.currentTarget.enumRefs", "ArchitectureSections current target enum authority must not fall back to broad enumRefs");
    assert.equal(allReadFields(refreshedArchRequest.agentAction).includes("enumRefs"), false, "ArchitectureSections single-section read plan must not read broad enumRefs when currentTarget is present");
    assert.equal(decision.instruction.submitCommand, undefined, "ArchitectureSections single-section instruction must not expose submitCommand before submit_existing_candidate");
    assert.ok(
      decision.instruction.routingRule.includes("completionBarrier.followUpCommand.commandInvocation"),
      "ArchitectureSections continue recovery must require the structured follow-up command after target file",
    );
    mark("L3", "active architecture lease recovers to generate_candidate");
    writeArchitectureSections(root, archRequestBody, pgc.contract);
    const archAccepted = run(["architecture", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--request-id", archRequestBody.requestId], root);
    assert.equal(archAccepted.accepted, true);
    assertAutoRunInstruction(archAccepted, "taskplan_generation", "Architecture accept");
    let indexAfterAccept = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`));
    let activePhaseRefs = indexAfterAccept.phases.find((phase) => phase.phaseId === started.phaseId).latestRefs;
    assert.equal(activePhaseRefs.architectureRequestId, archRequestBody.requestId, "Architecture accept must preserve source request id for repair routing.");
    assert.ok(activePhaseRefs.architectureRequest, "Architecture accept must preserve source request ref for repair routing.");
    mark("L1", "AAC section accept and assemble");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "taskplan_generation");
    assertNoMechanicalMaintenanceFields(decision, "continue to TaskPlanGenerationRequest");
    mark("L3", "continue routes to TaskPlanGenerationRequest");

    const taskPlanRequest = run(["task-plan", "request", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    assertInstructionOutputPolicy(taskPlanRequest.instruction, "TaskPlanGroupedGenerationRequest");
    const taskPlanRequestRaw = readJson(projectFile(root, taskPlanRequest.instruction.requestRef));
    assert.equal(taskPlanRequestRaw.contextProjection, undefined, "TaskPlan request root must not inline contextProjection.");
    assert.ok(taskPlanRequestRaw.contextProjectionRef, "TaskPlan request must store contextProjection as a request manifest ref.");
    const taskPlanRequestBody = requestFromCommand(taskPlanRequest, root);
    assertStatusRoutesToActiveRequest(root, started.deliveryId, started.phaseId, {
      label: "TaskPlanGroupedGenerationRequest",
      type: "taskplan_generation",
      requestRef: taskPlanRequest.instruction.requestRef,
      operationType: "taskplan_generation",
    });
    assert.equal(taskPlanRequest.request, undefined, "TaskPlan request stdout must stay compact");
    assertRequestProtocol(taskPlanRequestBody, "TaskPlanGroupedGenerationRequest");
    assertRequestOutputParentDirsExist(root, taskPlanRequestBody, "TaskPlanGroupedGenerationRequest");
    assertTaskPlanVerificationEvidenceProtocol(taskPlanRequestBody, "TaskPlanGroupedGenerationRequest");
    assert.ok(
      taskPlanRequestBody.agentAction.read.required.includes("contextProjection.requirementDetailTransfer"),
      "TaskPlan request must require reading requirementDetailTransfer.",
    );
    assert.ok(
      taskPlanRequestBody.contextProjection.requirementDetailTransfer.acceptanceDetails[0].statement.includes("VALIDATION_NAME_REQUIRED"),
      "TaskPlan requirementDetailTransfer must carry PGC acceptance detail.",
    );
    assert.ok(
      taskPlanRequestBody.contextProjection.requirementDetailTransfer.architectureDetails.interfaces.some((item) => item.interfaceId === "interface-run-validation"),
      "TaskPlan requirementDetailTransfer must carry AAC interface details.",
    );
    assert.ok(
      taskPlanRequestBody.contextProjection.requirementDetailTransfer.architectureDetails.userFlows.some((item) => item.flowId === "flow-run-validation"),
      "TaskPlan requirementDetailTransfer must carry AAC user flow details.",
    );
    assert.ok(
      taskPlanRequestBody.contextProjection.requirementDetailTransfer.requirementDetailAssignment.items.some((item) =>
        item.detailId &&
        item.coverageStatus === "covered" &&
        item.artifactRefs.interfaces.includes("interface-run-validation")
      ),
      "TaskPlan requirementDetailTransfer must carry detailId assignment with AAC artifact refs.",
    );
    assert.ok(
      taskPlanRequestBody.generationRules.requirementDetailTransferRules.rules.some((rule) => rule.includes("verificationIntents[].behavior")),
      "TaskPlan generation rules must tell Agent to carry concrete details into verification intents.",
    );
    assert.ok(taskPlanRequestBody.sourceRefs.phaseConceptGroundingRef, "TaskPlan request must expose phaseConceptGroundingRef in sourceRefs");
    assert.equal(
      taskPlanRequestBody.conceptGroundingSource.phaseConceptGroundingRef,
      taskPlanRequestBody.sourceRefs.phaseConceptGroundingRef,
      "TaskPlan conceptGroundingSource must mirror the sourceRefs phase grounding path",
    );
    assert.ok(
      taskPlanRequestBody.fieldAccessHints.phaseConceptGroundingRef.includes("sourceRefs.phaseConceptGroundingRef"),
      "TaskPlan fieldAccessHints must point Agent to sourceRefs.phaseConceptGroundingRef",
    );
    assert.ok(
      taskPlanRequestBody.agentAction.write.rules.some((rule) => rule.includes("sourceRefs.phaseConceptGroundingRef") && rule.includes("do not guess")),
      "TaskPlan request must prevent guessed concept grounding paths",
    );
    decision = run(["continue"], root);
    assert.equal(decision.instruction.mode, "generate_candidate");
    assert.equal(decision.instruction.candidateKind, "TaskPlanGroupedOutputs");
    assertNoMechanicalMaintenanceFields(decision, "active taskplan lease recovery");
    assertInstructionOutputPolicy(decision.instruction, "TaskPlan continue recovery");
    mark("L3", "active taskplan lease recovers to generate_candidate");
    const taskPlanOutputFiles = writeTaskPlanGroupedOutputs(root, taskPlanRequestBody);
    const overRestrictiveTaskPlanGroup = readJson(projectFile(root, taskPlanOutputFiles.groupFile));
    overRestrictiveTaskPlanGroup.tasks[0].writeBoundary.forbiddenPaths = [".git", "node_modules", ".loom"];
    delete overRestrictiveTaskPlanGroup.createdAt;
    writeJson(projectFile(root, taskPlanOutputFiles.groupFile), overRestrictiveTaskPlanGroup);
    const taskPlanAccepted = run(["task-plan", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--request-id", taskPlanRequestBody.requestId], root);
    assert.equal(taskPlanAccepted.accepted, true);
    indexAfterAccept = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`));
    activePhaseRefs = indexAfterAccept.phases.find((phase) => phase.phaseId === started.phaseId).latestRefs;
    assert.equal(activePhaseRefs.taskPlanRequestId, taskPlanRequestBody.requestId, "TaskPlan accept must preserve source request id for repair routing.");
    assert.ok(activePhaseRefs.taskPlanRequest, "TaskPlan accept must preserve source request ref for repair routing.");
    const architectureRepairFixtureRoot = path.join(os.tmpdir(), `loom-layered-architecture-repair-${Date.now()}`);
    copyDir(root, architectureRepairFixtureRoot);
    const architectureRepair = run(["repair", "request", "--type", "architecture", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], architectureRepairFixtureRoot);
    const architectureRepairRequest = requestFromCommand(architectureRepair, architectureRepairFixtureRoot);
    assertRequestOutputParentDirsExist(architectureRepairFixtureRoot, architectureRepairRequest, "Architecture repair request");
    assert.equal(architectureRepair.instruction.mode, "generate_candidate");
    assert.equal(architectureRepair.instruction.submitCommand.argv.includes("{requestId}"), false, "Architecture repair submitCommand must be concrete.");
    assert.equal(architectureRepair.instruction.submitCommand.argv.includes(archRequestBody.requestId), true, "Architecture repair submitCommand must target original request id.");
    assert.equal(architectureRepairRequest.outputContract.originalRequestId, archRequestBody.requestId);
    assert.ok(
      architectureRepairRequest.outputContract.contextProjection.requirementDetailTransfer.acceptanceDetails[0].statement.includes("VALIDATION_NAME_REQUIRED"),
      "Architecture repair request must carry requirementDetailTransfer.",
    );
    assert.ok(
      architectureRepairRequest.outputContract.sectionOutputs.some((output) =>
        output.section === "foundation" &&
        output.generationRules.some((rule) => rule.includes("current-phase module responsibility"))
      ),
      "Architecture repair request must carry section generation rules.",
    );
    const taskPlanRepairFixtureRoot = path.join(os.tmpdir(), `loom-layered-taskplan-repair-${Date.now()}`);
    copyDir(root, taskPlanRepairFixtureRoot);
    const taskPlanRepair = run(["repair", "request", "--type", "taskplan", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], taskPlanRepairFixtureRoot);
    const taskPlanRepairRequest = requestFromCommand(taskPlanRepair, taskPlanRepairFixtureRoot);
    assertRequestOutputParentDirsExist(taskPlanRepairFixtureRoot, taskPlanRepairRequest, "TaskPlan repair request");
    assert.equal(taskPlanRepair.instruction.mode, "generate_candidate");
    assert.equal(taskPlanRepair.instruction.submitCommand.argv.includes("{requestId}"), false, "TaskPlan repair submitCommand must be concrete.");
    assert.equal(taskPlanRepair.instruction.submitCommand.argv.includes(taskPlanRequestBody.requestId), true, "TaskPlan repair submitCommand must target original request id.");
    assert.equal(taskPlanRepairRequest.outputContract.originalRequestId, taskPlanRequestBody.requestId);
    mark("L3", "Architecture and TaskPlan repair requests return concrete submit commands");
    assert.equal(taskPlanAccepted.normalization, undefined, "TaskPlan accept must not expose mechanical normalization to Agent");
    assert.deepEqual(readJson(projectFile(root, taskPlanAccepted.taskPlanPath)).tasks[0].writeBoundary.forbiddenPaths, [".loom"]);
    mark("L2", "TaskPlan accept silently normalizes over-restrictive forbiddenPaths");
    assertAutoRunInstruction(taskPlanAccepted, "continue_execution", "TaskPlan accept");
    mark("L1", "TaskPlan grouped accept and assemble");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "continue_execution");
    assertNoMechanicalMaintenanceFields(decision, "continue to next-task");
    mark("L3", "continue routes to next-task");

    const nextTask = run(["next-task", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    assert.equal(nextTask.hasTask, true);
    assert.equal(nextTask.executionRequest.requestRef, nextTask.executionRequestPath);
    assertExecuteTaskInstruction(nextTask, "next-task");
    assertInstructionOutputPolicy(nextTask.instruction, "TaskExecutionRequest");
    const executionRequest = hydrateRequest(root, readJson(projectFile(root, nextTask.executionRequestPath)));
    assertRequestOutputParentDirsExist(root, executionRequest, "TaskExecutionRequest");
    assert.equal(
      fs.existsSync(path.dirname(projectFile(root, executionRequest.outputContract.resultFile))),
      true,
      "TaskExecutionRequest must precreate resultFile parent directory for agent Write tools",
    );
    assertRequestProtocol(executionRequest, "TaskExecutionRequest", { expectBlocked: true, allowProjectModification: true });
    assertSourceEditPreparationContract(executionRequest, "TaskExecutionRequest");
    assert.ok(executionRequest.agentAction.read.required.some((item) => item.includes("taskConceptGrounding")));
    assert.ok(executionRequest.agentAction.write.rules.some((rule) => rule.includes("conceptEvidence")));
    assert.ok(
      executionRequest.agentAction.read.required.includes("sourceContext.requirementDetailSnapshot"),
      "TaskExecutionRequest must require reading task-scoped requirement detail snapshot when TaskPlan assigned detail refs.",
    );
    assert.ok(
      executionRequest.sourceContext.requirementDetailSnapshot.some((item) =>
        item.detailId &&
        item.aacCoverage?.artifactRefs?.interfaces?.includes("interface-run-validation") &&
        item.verificationIntentRefs.includes("VI-core")
      ),
      "TaskExecutionRequest must carry task-scoped detail snapshot with AAC refs and verification linkage.",
    );
    assertSourceChangeOutputPolicy(nextTask.instruction, executionRequest, "TaskExecutionRequest");
    assertTaskExecutionEnvironmentPreparation(executionRequest, "TaskExecutionRequest");
    assertTaskExecutionCompletionBarrierProtocol(executionRequest, "TaskExecutionRequest");
    assertTaskResultSelfRepairProtocol(executionRequest, "TaskExecutionRequest");
    assertRuntimeDeliveryEvidenceProtocol(executionRequest, "TaskExecutionRequest");
    assert.deepEqual(
      executionRequest.sourceContext.acceptanceSnapshot[0].sourceRefs,
      ["src-001"],
      "TaskExecution acceptanceSnapshot must carry PGC acceptance sourceRefs.",
    );
    assert.deepEqual(
      executionRequest.sourceContext.acceptanceSnapshot[0].capabilityRefs,
      ["cap-core"],
      "TaskExecution acceptanceSnapshot must carry PGC acceptance capabilityRefs.",
    );
    assert.ok(
      executionRequest.sourceContext.acceptanceSnapshot[0].aacCoverage.coverage.some((entry) => entry.type === "interface" && entry.refs.includes("interface-run-validation")),
      "TaskExecution acceptanceSnapshot must carry AAC coverage details.",
    );
    fs.writeFileSync(projectFile(root, "src/layered.js"), "module.exports = { layered: true };\n");
    fs.writeFileSync(projectFile(root, "package-lock.json"), "{}\n");
    const invalidContinuityResultFile = executionRequest.outputContract.resultFile.replace(/\.json$/, "-invalid-continuity.json");
    writeJson(projectFile(root, invalidContinuityResultFile), {
      ...taskResult(executionRequest),
      taskResultId: "result-layered-invalid-continuity",
      executionContinuity: {
        taskResultSubmittedAfterVerification: true,
        agentOwnedLongRunningWork: "unknown",
        notes: [],
      },
    });
    const invalidContinuity = run(["record-result", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--input-file", invalidContinuityResultFile], root, { returnEnvelope: true });
    assert.equal(invalidContinuity.ok, true);
    assert.equal(invalidContinuity.data.recorded, false);
    assert.ok(
      invalidContinuity.data.issues.some((issue) => issue.path === "/executionContinuity/agentOwnedLongRunningWork"),
      "record-result must reject pure completed when agent-owned long-running work is unknown",
    );
    writeJson(projectFile(root, executionRequest.outputContract.resultFile), taskResult(executionRequest));
    const recorded = run(["record-result", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--input-file", executionRequest.outputContract.resultFile], root);
    assert.equal(recorded.recorded, true);
    assert.deepEqual(readJson(projectFile(root, recorded.resultPath)).changedFiles, ["src/layered.js", "package-lock.json"]);
    assertAutoRunInstruction(recorded, "review", "record-result");
    mark("L1", "TaskResult record");

    decision = run(["continue"], root);
    assert.equal(decision.nextAction.type, "review");
    assertNoMechanicalMaintenanceFields(decision, "continue to ReviewRequest");
    mark("L3", "continue routes to ReviewRequest");

    const review = run(["review", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId], root);
    const reviewRequest = requestFromCommand(review, root);
    assert.equal(review.request, undefined, "ReviewRequest stdout must stay compact");
    assertRequestProtocol(reviewRequest, "ReviewRequest", { expectBlocked: false });
    assertRequestOutputParentDirsExist(root, reviewRequest, "ReviewRequest");
    assert.equal(review.instruction.mode, "generate_candidate", "review request must return generate_candidate instruction");
    assert.equal(review.instruction.autoContinue, true, "review request must auto-continue");
    assert.equal(review.instruction.requestRef, review.requestPath);
    assert.equal(review.instruction.resultFile, reviewRequest.outputContract.resultFile);
    assert.ok(reviewRequest.agentAction.read.required.includes("conceptReviewMatrix"));
    assert.ok(reviewRequest.agentAction.read.required.includes("detailReviewMatrix"));
    assert.ok(reviewRequest.agentAction.read.required.includes("outputContract.conceptReviewRules"));
    assert.ok(reviewRequest.agentAction.read.required.includes("outputContract.requirementDetailReview"));
    assert.ok(reviewRequest.agentAction.write.rules.some((rule) => rule.includes("conceptRef from conceptReviewMatrix")));
    assert.ok(reviewRequest.agentAction.write.rules.some((rule) => rule.includes("detailReviewMatrix")));
    assert.ok(
      reviewRequest.agentAction.write.rules.some((rule) => rule.includes("add it as a new reachable entry")),
      "ReviewRequest must tell agents to check that new frontend modules are added as entries instead of replacing existing app-shell entries.",
    );
    assert.ok(
      reviewRequest.outputContract.frontendExperienceReview.reviewGuidance.some((rule) => rule.includes("add it as a new reachable entry")),
      "frontendExperienceReview must carry the app-shell entry preservation review rule.",
    );
    assert.equal(reviewRequest.conceptReviewMatrix[0].conceptRef, "concept-core-validation");
    assert.equal(reviewRequest.conceptReviewMatrix[0].priority, "must_understand");
    assert.ok(reviewRequest.conceptReviewMatrix[0].mustNotMisinterpretAs.includes("deployment work"));
    assert.ok(
      reviewRequest.detailReviewMatrix.some((item) =>
        item.status === "satisfied" &&
        item.taskRefs.includes("task-core") &&
        item.evidenceRefs.includes("result-layered")
      ),
      "ReviewRequest must carry requirement detail review matrix from TaskResult evidence.",
    );
    assert.ok(
      reviewRequest.outputContract.reviewSignals.some((signal) =>
        signal.kind === "requirement_detail_evidence" &&
        signal.detailSatisfied === true
      ),
      "ReviewRequest reviewSignals must include satisfied requirement detail evidence facts.",
    );
    mark("L3", "ReviewRequest create returned auto-runnable instruction");
    assert.deepEqual(readJson(projectFile(root, reviewRequest.changeContextRef)).changedFiles.map((file) => file.path), ["src/layered.js", "package-lock.json"]);
    const manualReviewFile = reviewRequest.outputContract.resultFile.replace(/result\.json$/, "manual-result.json");
    writeJson(projectFile(root, manualReviewFile), manualReviewResult(reviewRequest));
    const manualAccepted = run(["review", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--result-file", manualReviewFile], root);
    assert.equal(manualAccepted.accepted, true);
    assert.equal(manualAccepted.instruction.mode, "ask_user");
    const manualDecision = run(["continue"], root);
    assert.equal(manualDecision.nextAction.type, "manual_review");
    assertNoMechanicalMaintenanceFields(manualDecision, "manual_review continue");
    assert.ok(manualDecision.instruction.requestRef, "manual_review continue must expose requestRef");
    assert.ok(manualDecision.instruction.candidateFile, "manual_review continue must expose candidateFile");
    assert.ok(manualDecision.instruction.submitCommand, "manual_review continue must expose submitCommand");
    assert.ok(manualDecision.instruction.instruction.visibleReason, "manual_review continue must expose visibleReason");
    assert.ok(manualDecision.instruction.userMessage.includes("可以继续"), "manual_review userMessage must expose approve option");
    assert.ok(manualDecision.instruction.userMessage.includes("需要修改"), "manual_review userMessage must expose repair option");
    assert.ok(manualDecision.instruction.userMessage.includes("请直接回复"), "manual_review userMessage must tell user how to reply");
    assert.ok(manualDecision.instruction.instruction.userPrompt.includes("可以继续"), "manual_review request must include user-facing choice prompt");
    assert.ok(manualDecision.instruction.instruction.acceptedShortReplies[0].effect, "manual_review choices must explain effects");
    assert.ok(manualDecision.instruction.instruction.agentResolutionProtocol, "manual_review request must tell Agent how to resolve natural-language answers");
    assert.equal(manualDecision.instruction.instruction.visibleReason.findings[0].category, "environment_or_dependency");
    assert.equal(manualDecision.instruction.instruction.visibleReason.findings[0].summary.includes("Verification environment"), true);
    assert.equal(manualDecision.instruction.instruction.visibleReason.userDecisionHint.includes("网络"), true);
    assert.equal(manualDecision.instruction.userMessage.includes("环境"), true);
    mark("L3", "manual_review continue exposes user choices and resolution contract");

    writeJson(projectFile(root, reviewRequest.outputContract.resultFile), warningOnlyReviewResult(reviewRequest));
    const warningAccepted = run(["review", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--result-file", reviewRequest.outputContract.resultFile], root);
    assert.equal(warningAccepted.accepted, true, JSON.stringify(warningAccepted.issues, null, 2));
    assert.equal(warningAccepted.instruction.nextAction.type, "continue_to_next_phase");
    mark("L2", "ReviewResult warning-only severityClass does not block approval");

    writeJson(projectFile(root, reviewRequest.outputContract.resultFile), reviewResultContinueToPhase(reviewRequest, "phase-2"));
    const reviewAccepted = run(["review", "accept", "--delivery-id", started.deliveryId, "--phase-id", started.phaseId, "--result-file", reviewRequest.outputContract.resultFile], root);
    assert.equal(reviewAccepted.accepted, true);
    assert.equal(reviewAccepted.instruction.mode, "run_cli");
    assert.equal(reviewAccepted.instruction.nextAction.type, "continue_to_next_phase");
    mark("L1", "ReviewResult accept");

    const activatePhase2 = run(["continue"], root);
    assert.equal(activatePhase2.status, "ready");
    assert.equal(activatePhase2.nextAction.type, "repository_context_request");
    assert.equal(activatePhase2.phaseId, "phase-2");
    assertNoMechanicalMaintenanceFields(activatePhase2, "continue_to_next_phase");
    const indexAfterPhase2Activation = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`));
    assert.deepEqual(
      indexAfterPhase2Activation.phases.map((phase) => phase.phaseId),
      [started.phaseId, "phase-2"],
      "continue_to_next_phase must materialize the target phase from nextPhasePreview",
    );
    assert.equal(indexAfterPhase2Activation.activePhaseId, "phase-2");
    mark("L3", "continue_to_next_phase materializes nextPhasePreview");

    const phase2RepoRequest = run(["repository-context", "request", "--delivery-id", started.deliveryId, "--phase-id", "phase-2"], root);
    const phase2RepoRequestBody = requestFromCommand(phase2RepoRequest, root);
    assertRequestOutputParentDirsExist(root, phase2RepoRequestBody, "phase-2 RepositoryContextRequest");
    writeJson(projectFile(root, phase2RepoRequest.candidateFile), repositoryContextCandidate(phase2RepoRequest, root));
    const phase2RepoAccepted = run([
      "repository-context", "accept",
      "--delivery-id", started.deliveryId,
      "--phase-id", "phase-2",
      "--request-id", phase2RepoRequest.requestId,
      "--candidate-file", phase2RepoRequest.candidateFile,
    ], root);
    assert.equal(phase2RepoAccepted.operation, "repository_context_accepted");
    const indexBeforeBrainstormResume = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`));
    const phase2Index = indexBeforeBrainstormResume.phases.find((phase) => phase.phaseId === "phase-2");
    const phase1Index = indexBeforeBrainstormResume.phases.find((phase) => phase.phaseId === started.phaseId);
    const phase2BrainstormRequest = hydrateRequest(root, readJson(projectFile(root, phase2Index.latestRefs.brainstormRequest)));
    assertRequestOutputParentDirsExist(root, phase2BrainstormRequest, "Phase continuation BrainstormSessionRequest");
    assertBrainstormRequestReadPlan(phase2BrainstormRequest, "Phase continuation BrainstormSessionRequest");
    assertBrainstormConceptGroundingRequest(phase2BrainstormRequest);
    assert.ok(
      phase2BrainstormRequest.clarificationConversationProtocol?.blockExecutionRules?.some((rule) => rule.includes("business scenario confirmation")),
      "Phase continuation Brainstorm request must keep business scenario clarification rules.",
    );
    assert.ok(
      phase2BrainstormRequest.clarificationConversationProtocol?.blockExecutionRules?.some((rule) => rule.includes("structured BrainstormCandidate fields")),
      "Phase continuation Brainstorm request must keep structured detail retention rules.",
    );
    assert.ok(
      phase2BrainstormRequest.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.requiredUserVisibleTopicsWhenApplicable?.includes("explicit final_summary corrections that must be written back to structured fields"),
      "Phase continuation Brainstorm request must route final_summary corrections to structured fields.",
    );
    assert.ok(
      phase2BrainstormRequest.rules?.requirementSemanticGrounding?.finalSummaryBusinessDetailContract?.finalSummaryReviewContract?.rules?.some((rule) => rule.includes("not the source of detailed requirements")),
      "Phase continuation Brainstorm request must keep final_summary as a checklist gate.",
    );
    assert.equal(phase2Index.latestRefs.requirementContextRef, phase1Index.latestRefs.requirementContextRef, "Phase continuation latestRefs must inherit requirementContextRef");
    assert.equal(phase2Index.latestRefs.originalRequirementContextRef, phase1Index.latestRefs.requirementContextRef, "Phase continuation latestRefs must expose originalRequirementContextRef alias");
    assert.equal(phase2Index.latestRefs.normalizedRequirementTextRef, phase1Index.latestRefs.normalizedRequirementTextRef, "Phase continuation latestRefs must inherit normalizedRequirementTextRef");
    assert.equal(phase2Index.latestRefs.keywordHintsRef, phase1Index.latestRefs.keywordHintsRef, "Phase continuation latestRefs must inherit keywordHintsRef");
    assert.equal(phase2Index.latestRefs.latestConfirmedRequirementDecisionRef, phase1Index.latestRefs.brainstormDecision, "Phase continuation latestRefs must expose latest confirmed requirement decision");
    assert.equal(phase2Index.latestRefs.confirmedRequirementDecisionsIndexRef, phase1Index.latestRefs.brainstormDecisionsIndex, "Phase continuation latestRefs must expose confirmed decisions index");
    assert.equal(phase2BrainstormRequest.contextRefs.originalRequirementContextRef, phase1Index.latestRefs.requirementContextRef, "Phase continuation request must expose originalRequirementContextRef");
    assert.equal(phase2BrainstormRequest.contextRefs.requirementContextRef, phase1Index.latestRefs.requirementContextRef, "Phase continuation request must expose requirementContextRef");
    assert.equal(phase2BrainstormRequest.contextRefs.normalizedRequirementTextRef, phase1Index.latestRefs.normalizedRequirementTextRef, "Phase continuation request must expose normalizedRequirementTextRef");
    assert.equal(phase2BrainstormRequest.contextRefs.keywordHintsRef, phase1Index.latestRefs.keywordHintsRef, "Phase continuation request must expose keywordHintsRef");
    assert.equal(phase2BrainstormRequest.contextRefs.latestConfirmedRequirementDecisionRef, phase1Index.latestRefs.brainstormDecision, "Phase continuation request must expose latestConfirmedRequirementDecisionRef");
    assert.equal(phase2BrainstormRequest.contextRefs.confirmedRequirementDecisionsIndexRef, phase1Index.latestRefs.brainstormDecisionsIndex, "Phase continuation request must expose confirmedRequirementDecisionsIndexRef");
    assert.ok(
      phase2BrainstormRequest.referencedArtifactReadGuide.some((entry) => entry.refKey === "latestConfirmedRequirementDecisionRef"),
      "Phase continuation request read guide must include latestConfirmedRequirementDecisionRef",
    );
    assert.ok(
      phase2BrainstormRequest.referencedArtifactReadGuide.some((entry) => entry.refKey === "confirmedRequirementDecisionsIndexRef"),
      "Phase continuation request read guide must include confirmedRequirementDecisionsIndexRef",
    );
    assert.ok(
      phase2BrainstormRequest.referencedArtifactReadGuide.some((entry) => entry.refKey === "normalizedRequirementTextRef"),
      "Phase continuation request read guide must include normalizedRequirementTextRef",
    );
    assert.ok(
      phase2BrainstormRequest.referencedArtifactReadGuide.some((entry) => entry.refKey === "keywordHintsRef"),
      "Phase continuation request read guide must include keywordHintsRef",
    );
    assert.ok(
      phase2BrainstormRequest.phaseContinuationContext.rules.some((rule) => rule.includes("accepted BrainstormContract")),
      "Phase continuation rules must describe deliveryContextRef as BrainstormContract, not original full requirement text",
    );
    const phase2Inspect = run([
      "inspect",
      "--request",
      phase2Index.latestRefs.brainstormRequest,
      "--field",
      "requirementContext.normalizedText,keywordHints,latestConfirmedRequirementDecision,confirmedRequirementDecisionsIndex,phaseContinuationContext",
    ], root);
    assert.equal(typeof phase2Inspect.fields["requirementContext.normalizedText"].value, "string", "inspect must resolve inherited normalized requirement text");
    assert.equal(phase2Inspect.fields.keywordHints.status, "resolved", "inspect must resolve inherited keyword hints");
    assert.equal(phase2Inspect.fields.latestConfirmedRequirementDecision.value.phaseId, started.phaseId, "inspect must resolve latest confirmed requirement decision");
    assert.equal(phase2Inspect.fields.confirmedRequirementDecisionsIndex.value.latestConfirmedPhaseId, started.phaseId, "inspect must resolve confirmed decisions index");
    assert.ok(
      phase2BrainstormRequest.rules?.requirementSemanticGrounding?.rules?.some((rule) => rule.includes("Correction, completion, or optimization phases") || rule.includes("correction, completion, or optimization phases")),
      "Phase continuation Brainstorm request must include correction/optimization semantic grounding rule",
    );
    assert.ok(phase2BrainstormRequest.outputContract.schemaShape.frontendExperience);
    assert.equal(phase2BrainstormRequest.outputContract.schemaShape.frontendExperienceDelta, undefined);
    assert.ok(
      phase2BrainstormRequest.outputContract.schemaShape.candidateRules.some((rule) => rule.includes("frontendExperience.dataViews/actions/operationPaths")),
      "Phase continuation Brainstorm request must route frontend details into frontendExperience",
    );
    const expectedCandidateFile = phase2Index.latestRefs.brainstormCandidateFile;
    delete phase2Index.latestRefs.brainstormCandidateFile;
    writeJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`), indexBeforeBrainstormResume);
    const phase2BrainstormGate = run(["continue"], root);
    assert.equal(phase2BrainstormGate.nextAction.type, "brainstorm_confirmation");
    assertBrainstormStartInstruction(phase2BrainstormGate.instruction, phase2Index.latestRefs.brainstormRequest, "brainstorm waiting_user continue");
    assertNoMechanicalMaintenanceFields(phase2BrainstormGate, "brainstorm waiting_user continue");
    assert.equal(phase2BrainstormGate.instruction.expectedResponse.requestRef, phase2Index.latestRefs.brainstormRequest);
    assert.equal(phase2BrainstormGate.instruction.expectedResponse.candidateFile, undefined);
    assert.equal(phase2BrainstormGate.instruction.expectedResponse.submitCommand, undefined);
    assert.equal(phase2BrainstormRequest.outputContract.candidateFile, expectedCandidateFile);
    assert.ok(
      phase2BrainstormRequest.agentAction.read.fieldGroups
        .find((group) => group.groupId === "brainstorm_session_candidate_write_controls")
        ?.whenToRead.includes("final_summary"),
      "Brainstorm request must keep candidate path in delayed final_summary write contract",
    );
    mark("L3", "brainstorm waiting_user continue hides submit command until final_summary confirmation");

    const phase2Candidate = brainstormCandidate(phase2BrainstormRequest);
    phase2Candidate.phaseId = "phase-2";
    phase2Candidate.roadmap.currentPhaseId = "phase-2";
    phase2Candidate.roadmap.phases = [{
      phaseId: "phase-2",
      title: "Follow-up Validation",
      status: "scope_confirmed",
      goal: "Confirm the follow-up validation phase.",
      scopeRefs: ["scope-core"],
      acceptanceRefs: ["AC-001"],
      dependsOn: ["phase-1"],
    }];
    phase2Candidate.phasePlan.current = {
      phaseId: "phase-2",
      title: "Follow-up Validation",
      goal: "Confirm the follow-up validation phase.",
      scopeRefs: ["scope-core"],
      acceptanceRefs: ["AC-001"],
      status: "scope_confirmed",
    };
    phase2Candidate.conceptGrounding.glossaryUpdates = [{
      updateId: "glossary-update-phase2-001",
      operation: "add",
      concept: {
        conceptId: "concept-phase2-added",
        term: "Follow-up validation",
        normalizedName: "follow_up_validation",
        explanation: "A later phase validation concept confirmed during phase 2.",
        mustNotMisinterpretAs: ["phase 1 scope"],
        phaseRelevance: "current",
        priority: "should_understand",
        attentionRank: 1,
        riskFactors: ["scope_confusion_risk"],
        scopeRefs: ["scope-core"],
        acceptanceRefs: ["AC-001"],
        humanReadableReason: "Verifies glossary updates are merged into the delivery glossary.",
      },
      reason: "Phase 2 introduced a confirmed glossary concept.",
    }];
    const currentFrontendPath = `.loom/deliveries/${started.deliveryId}/frontend-experience/current.json`;
    writeJson(projectFile(root, currentFrontendPath), {
      schemaVersion: "1.0",
      deliveryId: started.deliveryId,
      phaseId: "phase-1",
      currentPhaseId: "phase-1",
      updatedAt: now(),
      frontendExperience: {
        required: true,
        kind: "business_application",
        experienceLevel: "usable_internal_product",
        audiences: [{ audienceId: "audience-existing", name: "Existing user", primaryJobs: ["Use existing workflow."] }],
        surfaces: [{ surfaceId: "surface-existing", name: "Existing workspace", audienceRefs: ["audience-existing"], primaryJobs: ["Use existing workflow."] }],
        mustNot: ["unstyled_browser_default"],
        confirmationSummary: "Existing frontend target.",
      },
      source: "test_existing_frontend_target",
    });
    phase2Candidate.clarificationProgress.confirmedBlocks = [
      ...phase2Candidate.clarificationProgress.confirmedBlocks.filter((block) => block.block !== "frontend_experience"),
      { block: "frontend_experience", summary: "User confirmed inherited frontend with phase 2 adjustments.", confirmedByUser: true },
    ];
    phase2Candidate.clarificationProgress.skippedBlocks = [];
    phase2Candidate.frontendExperience = {
      required: true,
      kind: "business_application",
      experienceLevel: "usable_internal_product",
      audiences: [{ audienceId: "audience-existing", name: "Existing user", primaryJobs: ["Use existing workflow with phase 2 validation."] }],
      surfaces: [{ surfaceId: "surface-existing", name: "Existing workspace", audienceRefs: ["audience-existing"], primaryJobs: ["Use phase 2 validation workflow."] }],
      dataViews: [{
        viewId: "view-phase2-validation",
        name: "Phase 2 validation list",
        purpose: "Let users find and select the phase 2 validation target.",
        targetObject: "Follow-up validation",
        selectionMode: "query_and_select",
        paginationRequired: true,
        defaultLoadsFirstPage: true,
        searchCriteria: [],
        sourceRefs: ["src-001"],
      }],
      actions: [{
        actionId: "action-phase2-validation",
        label: "Run phase 2 validation",
        targetObject: "Follow-up validation",
        entryPoint: "result_row_action",
        inputFields: [],
        resultObservation: ["list_refresh", "response_message"],
        refreshPolicy: "refresh_current_query",
        successFeedback: ["Validation result is visible after refresh."],
        blockingOrErrorFeedback: ["Validation blocking reason is visible."],
        sourceRefs: ["src-001"],
      }],
      operationPaths: [{
        pathId: "path-phase2-validation",
        name: "Phase 2 validation operation path",
        userGoal: "Complete the follow-up validation workflow.",
        surfaceRef: "surface-existing",
        workflowRef: "flow-core",
        targetObject: "Follow-up validation",
        selectionMode: "query_and_select",
        selectionSummary: "Paginated list -> select validation target -> run validation -> see refreshed result.",
        dataViewRefs: ["view-phase2-validation"],
        actionRefs: ["action-phase2-validation"],
        requiredStates: ["loading", "success", "error", "empty", "business_blocking"],
        sourceRefs: ["src-001"],
      }],
      mustNot: ["unstyled_browser_default"],
      confirmationSummary: "User confirmed phase 2 frontend target.",
    };
    writeJson(projectFile(root, phase2BrainstormRequest.outputContract.candidateFile), phase2Candidate);
    const phase2BrainstormAccepted = run([
      "brainstorm", "accept",
      "--delivery-id", started.deliveryId,
      "--phase-id", "phase-2",
      "--request-id", phase2BrainstormRequest.requestId,
      "--run-id", phase2BrainstormRequest.brainstormRunId,
      "--candidate-file", phase2BrainstormRequest.outputContract.candidateFile,
    ], root);
    assert.equal(phase2BrainstormAccepted.accepted, true);
    assert.equal(phase2BrainstormAccepted.repairInstruction, undefined, "missing prior roadmap phases must be normalized without repair");
    const phase2Contract = readJson(projectFile(root, phase2BrainstormAccepted.contractPath));
    assert.deepEqual(
      phase2Contract.roadmap.phases.map((phase) => phase.phaseId),
      ["phase-1", "phase-2"],
      "Brainstorm accept must preserve prior roadmap phases when Phase N candidate only outputs current phase",
    );
    const deliveryGlossary = readJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/concepts/delivery-glossary.json`));
    assert.ok(deliveryGlossary.concepts.some((concept) => concept.conceptId === "concept-phase2-added"), "Phase N glossaryUpdates must merge into delivery glossary");
    const currentFrontend = readJson(projectFile(root, currentFrontendPath));
    assert.equal(currentFrontend.frontendExperience.confirmationSummary, "User confirmed phase 2 frontend target.");
    assert.equal(currentFrontend.frontendExperienceDelta, undefined);
    assert.equal(currentFrontend.inheritedFrontendExperience, undefined);
    mark("L2", "BrainstormCandidate accept normalizes omitted prior roadmap phases");

    indexAfterPhase2Activation.status = "completed";
    indexAfterPhase2Activation.phases[1].status = "completed";
    indexAfterPhase2Activation.phases[1].nextAction = {
      type: "done",
      source: "test",
      deliveryId: started.deliveryId,
      phaseId: "phase-2",
      reason: "TEST_COMPLETED",
    };
    indexAfterPhase2Activation.updatedAt = now();
    writeJson(projectFile(root, `.loom/deliveries/${started.deliveryId}/index.json`), indexAfterPhase2Activation);
    const completedStatus = readJson(projectFile(root, ".loom/status.json"));
    completedStatus.activeDeliveryId = null;
    completedStatus.lastCompletedDeliveryId = started.deliveryId;
    completedStatus.deliveries = completedStatus.deliveries.map((delivery) =>
      delivery.deliveryId === started.deliveryId
        ? { ...delivery, status: "completed", activePhaseId: "phase-2", updatedAt: now() }
        : delivery,
    );
    completedStatus.effectiveNextAction = {
      type: "done",
      source: "test",
      deliveryId: started.deliveryId,
      phaseId: "phase-2",
      reason: "TEST_COMPLETED",
    };
    completedStatus.phase = "completed";
    completedStatus.nextAction = "none";
    completedStatus.updatedAt = now();
    writeJson(projectFile(root, ".loom/status.json"), completedStatus);
    mark("L3", "test fixture marks delivery completed after phase activation");

    const status = readJson(projectFile(root, ".loom/status.json"));
    assert.equal(status.activeDeliveryId, null);
    assert.equal(status.lastCompletedDeliveryId, started.deliveryId);
    mark("L3", "status.json records completed delivery");

    const completedContinue = run(["continue"], root);
    assert.equal(completedContinue.status, "done");
    assert.equal(completedContinue.nextAction.type, "done");
    assert.equal(completedContinue.nextAction.reason, "DELIVERY_ALREADY_COMPLETED");
    assert.equal(completedContinue.instruction.mode, "report_done");
    assert.ok(completedContinue.instruction.userMessage.includes("不是卡住状态"), "completed continue must clarify this is not a stuck state");
    assert.ok(completedContinue.instruction.userMessage.includes("无需再执行 continue"), "completed continue must tell user no continue is needed");
    assert.ok(completedContinue.instruction.userMessage.includes("@loom 实现新的需求"), "completed continue must tell Codex users how to start a new delivery");
    assertNoMechanicalMaintenanceFields(completedContinue, "completed delivery continue");
    mark("L3", "completed delivery continue reports done");

    const completedStatusGuidance = run(["status"], root);
    assert.ok(completedStatusGuidance.userGuidance.includes("不是卡住状态"), "completed status must clarify this is not a stuck state");
    assert.ok(completedStatusGuidance.userGuidance.includes("无需再执行 continue"), "completed status must tell user no continue is needed");
    assert.ok(completedStatusGuidance.userGuidance.includes("@loom 实现新的需求"), "completed status must tell Codex users how to start a new delivery");
    const compactCompletedStatus = runCompact(["status"], root).data;
    assert.ok(compactCompletedStatus.userGuidance.includes("不是卡住状态"), "compact completed status must preserve user guidance");
    mark("L3", "completed delivery status exposes user guidance");

    console.log(JSON.stringify({
      ok: true,
      summary: "Layer 1-3 self-validation passed",
      checks,
    }, null, 2));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();
