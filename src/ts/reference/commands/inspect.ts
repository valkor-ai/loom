import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument, LoomError } from "../core/errors";
import { ok } from "./envelope";
import { readJsonFile } from "../core/state/fs";
import { normalizeAgentActionForRequest } from "../core/operations/agent-action";
import { hydrateRequestManifest } from "../core/operations/request-manifest";
import { prettyJsonByteLength, recordTokenSavingEvent } from "../core/operations/token-saving-telemetry";
import type { CliEnvelope, CommandContext } from "./types";

export function createInspectHandler(options: {
  request?: string;
  field?: string;
  valuesOnly?: boolean;
}) {
  return async function handleInspect(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.request || options.request.trim().length === 0) {
      throw invalidArgument("inspect requires --request.", {
        requiredArgs: ["--request", "--field"],
      });
    }
    if (!options.field || options.field.trim().length === 0) {
      throw invalidArgument("inspect requires --field.", {
        requiredArgs: ["--request", "--field"],
      });
    }

    const requestRef = options.request.trim();
    const fields = parseFields(options.field);
    const requestFile = resolveProjectFile(ctx.projectRoot, requestRef);
    const request = await hydrateRequestManifest(ctx.projectRoot, requestFile);
    if (!isRecord(request)) {
      throw invalidArgument("inspect request must point to a JSON object.", {
        requestRef,
      });
    }

    const resolvedFields: Record<string, {
      status: "resolved" | "not_available";
      value: unknown;
      fieldRead: {
        status: "resolved" | "not_available";
        resolvedRefKey: string | null;
        resolvedRef: string | null;
        selector: string;
        source: "request_root" | "request_manifest_ref";
        unavailableReason?: string;
      };
    }> = {};
    for (const field of fields) {
      const resolved = await resolveRequestFieldWithRecovery(ctx.projectRoot, requestRef, request, fields, field);
      resolvedFields[field] = {
        status: resolved.status,
        value: resolved.value,
        fieldRead: {
          status: resolved.status,
          resolvedRefKey: resolved.resolvedRefKey,
          resolvedRef: resolved.resolvedRef,
          selector: resolved.selector,
          source: resolved.source,
          ...(resolved.unavailableReason ? { unavailableReason: resolved.unavailableReason } : {}),
        },
      };
    }
    const data = {
      requestRef,
      requestedFields: fields,
      fields: resolvedFields,
    };
    const outputData = options.valuesOnly ? valuesOnlyInspectData(resolvedFields) : data;
    await recordInspectTelemetry(ctx.projectRoot, requestFile, requestRef, outputData, resolvedFields);
    return ok("inspect", ctx.projectRoot, outputData, fields.length === 1 ? "Field inspected." : "Fields inspected.");
  };
}

function valuesOnlyInspectData(
  resolvedFields: Record<string, {
    status: "resolved" | "not_available";
    value: unknown;
  }>,
): { fields: Record<string, unknown> } {
  return {
    fields: Object.fromEntries(
      Object.entries(resolvedFields).map(([field, result]) => [field, result.value]),
    ),
  };
}

function parseFields(value: string | undefined): string[] {
  const fields = (value ?? "")
    .split(",")
    .map((field) => field.trim())
    .filter((field) => field.length > 0);
  const unique = [...new Set(fields)];
  if (unique.length === 0) {
    throw invalidArgument("inspect --field must include at least one non-empty field path.");
  }
  return unique;
}

async function resolveRequestFieldWithRecovery(
  projectRoot: string,
  requestRef: string,
  request: Record<string, unknown>,
  requestedFields: string[],
  field: string,
): Promise<{
  value: unknown;
  status: "resolved" | "not_available";
  unavailableReason?: string;
  resolvedRefKey: string | null;
  resolvedRef: string | null;
  selector: string;
  source: "request_root" | "request_manifest_ref";
}> {
  try {
    return await resolveRequestField(projectRoot, request, field);
  } catch (error) {
    if (error instanceof LoomError && error.code === "INVALID_ARGUMENT") {
      throw invalidArgument(error.message, {
        ...errorDetailsObject(error.details),
        inspectRecovery: await buildInspectRecovery(projectRoot, requestRef, request, requestedFields),
      });
    }
    throw error;
  }
}

async function resolveRequestField(
  projectRoot: string,
  request: Record<string, unknown>,
  field: string,
): Promise<{
  value: unknown;
  status: "resolved" | "not_available";
  unavailableReason?: string;
  resolvedRefKey: string | null;
  resolvedRef: string | null;
  selector: string;
  source: "request_root" | "request_manifest_ref";
}> {
  const parts = field.split(".").filter(Boolean);
  if (parts.length === 0) {
    throw invalidArgument("inspect --field must be a non-empty field path.");
  }

  const contextField = await resolveRequestContextRefField(projectRoot, request, field);
  if (contextField) {
    return contextField;
  }

  const manifestRefs = requestManifestRefs(request);
  const rootKey = parts[0];
  if (rootKey === "agentAction" && rootKey in request) {
    const normalizedRequest = {
      ...request,
      agentAction: normalizeAgentActionForRequest(request.agentAction, request),
    };
    return {
      status: "resolved",
      value: selectValue(normalizedRequest, parts),
      resolvedRefKey: null,
      resolvedRef: null,
      selector: `.${parts.join(".")}`,
      source: "request_root",
    };
  }
  if (rootKey === "contextRefs" && parts.length >= 2) {
    const contextRefs = request.contextRefs;
    const refKey = parts[1];
    if (isRecord(contextRefs) && !(refKey in contextRefs)) {
      return {
        status: "not_available",
        unavailableReason: "contextRef is not present on this request.",
        value: null,
        resolvedRefKey: null,
        resolvedRef: null,
        selector: `.${parts.join(".")}`,
        source: "request_root",
      };
    }
  }
  const refInfo = manifestRefs[rootKey];
  if (refInfo?.ref) {
    const refFile = resolveProjectFile(projectRoot, refInfo.ref);
    const refValue = await readJsonFile(refFile);
    const normalizedRefValue = rootKey === "agentAction"
      ? normalizeAgentActionForRequest(refValue, request)
      : refValue;
    const selectorParts = parts.slice(1);
    const value = rootKey === "rules" && selectorParts.join(".") === "requirementSemanticGrounding.compactRules"
      ? selectCompactRequirementSemanticRules(normalizedRefValue)
      : selectorParts.length === 0 ? normalizedRefValue : selectValue(normalizedRefValue, selectorParts);
    return {
      status: "resolved",
      value,
      resolvedRefKey: rootKey,
      resolvedRef: refInfo.ref,
      selector: selectorParts.length === 0 ? "$" : `.${selectorParts.join(".")}`,
      source: "request_manifest_ref",
    };
  }

  const normalizedRequest = rootKey === "agentAction"
    ? {
      ...request,
      agentAction: normalizeAgentActionForRequest(request.agentAction, request),
    }
    : request;
  const rootValue = parts.join(".") === "rules.requirementSemanticGrounding.compactRules"
    ? selectCompactRequirementSemanticRules(normalizedRequest.rules)
    : selectValue(normalizedRequest, parts);
  return {
    status: "resolved",
    value: rootValue,
    resolvedRefKey: null,
    resolvedRef: null,
    selector: `.${parts.join(".")}`,
    source: "request_root",
  };
}

async function resolveRequestContextRefField(
  projectRoot: string,
  request: Record<string, unknown>,
  field: string,
): Promise<{
  value: unknown;
  status: "resolved" | "not_available";
  unavailableReason?: string;
  resolvedRefKey: string | null;
  resolvedRef: string | null;
  selector: string;
  source: "request_root" | "request_manifest_ref";
} | null> {
  const exactContextRefFields: Record<string, {
    refField: string;
    selectorParts: string[];
    text?: boolean;
  }> = {
    "requirementContext.normalizedText": {
      refField: "normalizedRequirementTextRef",
      selectorParts: [],
      text: true,
    },
  };
  const contextRefAliases: Record<string, {
    refField: string;
    selectorParts: string[];
  }> = {
    requirementContext: {
      refField: "requirementContextRef",
      selectorParts: [],
    },
    originalRequirementContext: {
      refField: "originalRequirementContextRef",
      selectorParts: [],
    },
    keywordHints: {
      refField: "keywordHintsRef",
      selectorParts: [],
    },
    deliveryContext: {
      refField: "deliveryContextRef",
      selectorParts: [],
    },
    latestRepositoryContext: {
      refField: "latestRepositoryContextRef",
      selectorParts: [],
    },
    latestConfirmedRequirementDecision: {
      refField: "latestConfirmedRequirementDecisionRef",
      selectorParts: [],
    },
    confirmedRequirementDecisionsIndex: {
      refField: "confirmedRequirementDecisionsIndexRef",
      selectorParts: [],
    },
    deliveryConceptGlossary: {
      refField: "deliveryConceptGlossaryRef",
      selectorParts: [],
    },
    phaseConceptGrounding: {
      refField: "phaseConceptGroundingRef",
      selectorParts: [],
    },
    currentFrontendExperience: {
      refField: "currentFrontendExperienceRef",
      selectorParts: [],
    },
  };

  const exactSpec = exactContextRefFields[field];
  const alias = exactSpec
    ? null
    : Object.keys(contextRefAliases)
      .sort((left, right) => right.length - left.length)
      .find((candidate) => field === candidate || field.startsWith(`${candidate}.`)) ?? null;
  const aliasSpec = alias ? contextRefAliases[alias] : null;
  const spec = exactSpec ?? aliasSpec;
  if (!spec) {
    return null;
  }
  const selectorParts = exactSpec
    ? exactSpec.selectorParts
    : [
        ...spec.selectorParts,
        ...field.slice(alias?.length ?? 0).split(".").filter(Boolean),
      ];

  const contextRefs = request.contextRefs;
  const contextRefValue = isRecord(contextRefs) ? contextRefs[spec.refField] : null;
  const ref = typeof contextRefValue === "string" ? contextRefValue : null;
  if (!ref) {
    return {
      status: "not_available",
      unavailableReason: "contextRef is not present on this request.",
      value: null,
      resolvedRefKey: `contextRefs.${spec.refField}`,
      resolvedRef: null,
      selector: selectorParts.length === 0 ? "$" : `.${selectorParts.join(".")}`,
      source: "request_root",
    };
  }

  const refFile = resolveProjectFile(projectRoot, ref);
  const refValue = exactSpec?.text
    ? await fs.readFile(refFile, "utf8")
    : await readJsonFile(refFile);
  const value = alias === "keywordHints" && selectorParts[0] === "compact"
    ? selectCompactKeywordHints(refValue, selectorParts.slice(1))
    : selectorParts.length === 0 ? refValue : selectValue(refValue, selectorParts);
  return {
    status: "resolved",
    value,
    resolvedRefKey: `contextRefs.${spec.refField}`,
    resolvedRef: ref,
    selector: selectorParts.length === 0 ? "$" : `.${selectorParts.join(".")}`,
    source: "request_root",
  };
}

function selectCompactKeywordHints(value: unknown, selectorParts: string[]): unknown {
  const compact = compactKeywordHints(value);
  return selectorParts.length === 0 ? compact : selectValue(compact, selectorParts);
}

function compactKeywordHints(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) {
    return {
      usage: "advisory_only",
      status: "empty",
      languageHints: [],
      topKeywords: [],
      sectionKeywords: [],
      rules: keywordHintCompactRules(),
    };
  }
  if (isRecord(value.compact)) {
    return value.compact;
  }
  return {
    usage: value.usage === "advisory_only" ? "advisory_only" : "advisory_only",
    status: value.status === "completed" ? "completed" : "empty",
    languageHints: Array.isArray(value.languageHints)
      ? value.languageHints.filter((item): item is string => typeof item === "string").slice(0, 5)
      : [],
    topKeywords: Array.isArray(value.globalKeywords)
      ? value.globalKeywords.filter(isRecord).slice(0, 16).map((hint) => ({
        keyword: typeof hint.keyword === "string" ? hint.keyword : "",
        occurrences: typeof hint.occurrences === "number" ? hint.occurrences : 0,
        sourceItemIds: Array.isArray(hint.sourceItemIds)
          ? hint.sourceItemIds.filter((item): item is string => typeof item === "string").slice(0, 3)
          : [],
      })).filter((hint) => hint.keyword.length > 0)
      : [],
    sectionKeywords: Array.isArray(value.sectionKeywords)
      ? value.sectionKeywords.filter(isRecord).slice(0, 6).map((section) => ({
        sectionId: typeof section.sectionId === "string" ? section.sectionId : "",
        sourceItemId: typeof section.sourceItemId === "string" ? section.sourceItemId : "",
        ...(typeof section.title === "string" ? { title: section.title } : {}),
        keywords: Array.isArray(section.keywords)
          ? section.keywords.filter(isRecord).slice(0, 6).map((hint) => typeof hint.keyword === "string" ? hint.keyword : "").filter(Boolean)
          : [],
      })).filter((section) => section.sectionId.length > 0 || section.keywords.length > 0)
      : [],
    rules: keywordHintCompactRules(),
  };
}

function keywordHintCompactRules(): Record<string, true> {
  return {
    advisoryOnly: true,
    mustNotTreatAsScope: true,
    mustNotTreatAsAcceptance: true,
    ignoreWhenIrrelevant: true,
  };
}

function selectCompactRequirementSemanticRules(value: unknown): unknown {
  if (!isRecord(value)) {
    return [];
  }
  const requirementSemanticGrounding = value.requirementSemanticGrounding;
  if (!isRecord(requirementSemanticGrounding)) {
    return [];
  }
  if (Array.isArray(requirementSemanticGrounding.compactRules)) {
    return requirementSemanticGrounding.compactRules.filter((item): item is string => typeof item === "string");
  }
  if (!Array.isArray(requirementSemanticGrounding.rules)) {
    return [];
  }
  return requirementSemanticGrounding.rules
    .filter((item): item is string => typeof item === "string")
    .slice(0, 7);
}

function requestManifestRefs(request: Record<string, unknown>): Record<string, { ref?: string }> {
  const manifest = request.requestManifest;
  if (!isRecord(manifest) || !isRecord(manifest.refs)) {
    return {};
  }
  const output: Record<string, { ref?: string }> = {};
  for (const [key, value] of Object.entries(manifest.refs)) {
    if (!isRecord(value)) continue;
    const ref = typeof value.ref === "string" ? value.ref : undefined;
    output[key] = { ref };
  }
  return output;
}

async function buildInspectRecovery(
  projectRoot: string,
  requestRef: string,
  request: Record<string, unknown>,
  requestedFields: string[],
): Promise<Record<string, unknown>> {
  const readPlan = await resolveRequestReadPlan(projectRoot, requestRef, request);
  const requiredGroup = readPlan.availableFieldGroups.find((group) => group.required)
    ?? readPlan.availableFieldGroups[0];
  return {
    status: "field_not_found_use_request_read_plan",
    requestedFields,
    requestRef,
    readPlanAuthority: "requestReadPlan.groups",
    readPlanSource: readPlan.source,
    ...(readPlan.readError ? { requestReadPlanReadError: readPlan.readError } : {}),
    availableFieldGroups: readPlan.availableFieldGroups,
    recommendedNextRead: requiredGroup
      ? {
        reason: "The requested inspect field is not part of this request contract. Read the next required fieldGroup instead of guessing legacy root fields.",
        groupId: requiredGroup.groupId,
        commandInvocation: requiredGroup.commandInvocation,
      }
      : {
        reason: "No requestReadPlan.groups were found. Read requestManifest refs for the required root keys before falling back to the request file.",
        commandInvocation: {
          name: "inspect",
          argv: ["inspect", "--request", requestRef, "--field", "requestManifest"],
          projectRootRequired: true,
          preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
        },
      },
    fallbackRule: "If the recommended inspect command fails, read the listed fieldGroup fields through requestManifest refs and targeted selectors. If the read plan is missing or unreadable, read requestRef and requestManifest refs directly as a correctness fallback while keeping chat output compact.",
    doNot: [
      "Do not guess old wrapper fields such as phaseScopePrompt, data, contract, objective, scope, or outputContract when they are not listed in requestReadPlan.groups.",
      "Do not run broad searches over $HOME/.loom, .codex, node_modules, unrelated test directories, or the whole project to discover request fields.",
      "Do not print full .loom request, TaskPlan, run, result, or ref JSON into chat.",
    ],
    availableTopLevelFields: Object.keys(request),
    requestManifestRefKeys: Object.keys(requestManifestRefs(request)),
  };
}

async function resolveRequestReadPlan(
  projectRoot: string,
  requestRef: string,
  request: Record<string, unknown>,
): Promise<{
  source: "request_read_plan" | "request_root" | "request_manifest_ref" | "missing";
  ref: string | null;
  readError?: string;
  availableFieldGroups: InspectRecoveryReadGroup[];
}> {
  const requestReadPlanGroups = fieldGroupsFromRequestReadPlan(request.requestReadPlan, requestRef);
  if (requestReadPlanGroups.length > 0) {
    return {
      source: "request_read_plan",
      ref: null,
      availableFieldGroups: requestReadPlanGroups,
    };
  }

  const rootGroups = fieldGroupsFromAgentAction(request.agentAction, requestRef, request);
  if (rootGroups.length > 0) {
    return {
      source: "request_root",
      ref: null,
      availableFieldGroups: rootGroups,
    };
  }

  const agentActionRef = requestManifestRefs(request).agentAction?.ref;
  if (agentActionRef) {
    try {
      const agentAction = await readJsonFile(resolveProjectFile(projectRoot, agentActionRef));
      return {
        source: "request_manifest_ref",
        ref: agentActionRef,
        availableFieldGroups: fieldGroupsFromAgentAction(agentAction, requestRef, request),
      };
    } catch (error) {
      return {
        source: "request_manifest_ref",
        ref: agentActionRef,
        readError: error instanceof Error ? error.message : String(error),
        availableFieldGroups: [],
      };
    }
  }

  return {
    source: "missing",
    ref: null,
    availableFieldGroups: [],
  };
}

type InspectRecoveryReadGroup = {
  groupId: string;
  required: boolean;
  purpose: string;
  whenToRead: string;
  fields: string[];
  readCommand: {
    name: "inspect";
    argv: string[];
  };
  commandInvocation: {
    name: "inspect";
    argv: string[];
    projectRootRequired: true;
    preserveEnv: string[];
  };
  fallbackRule: string;
};

function fieldGroupsFromRequestReadPlan(value: unknown, requestRef: string): InspectRecoveryReadGroup[] {
  if (!isRecord(value) || !Array.isArray(value.groups)) {
    return [];
  }
  return value.groups
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const fields = Array.isArray(group.fields)
        ? [...new Set(group.fields.filter((field): field is string => typeof field === "string" && field.trim().length > 0).map((field) => field.trim()))]
        : [];
      if (fields.length === 0) {
        return null;
      }
      const readArgv = isRecord(group.readCommand) && Array.isArray(group.readCommand.argv)
        ? group.readCommand.argv.map((part) => String(part) === "{requestRef}" ? requestRef : String(part))
        : ["inspect", "--request", requestRef, "--field", fields.join(",")];
      return {
        groupId: typeof group.groupId === "string" && group.groupId.trim().length > 0 ? group.groupId : "request_fields",
        required: typeof group.required === "boolean" ? group.required : true,
        purpose: typeof group.purpose === "string" ? group.purpose : "Request fields required by the current loom action.",
        whenToRead: typeof group.whenToRead === "string" ? group.whenToRead : "Before acting on the current loom request.",
        fields,
        readCommand: {
          name: "inspect" as const,
          argv: readArgv,
        },
        commandInvocation: {
          name: "inspect" as const,
          argv: readArgv,
          projectRootRequired: true as const,
          preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
        },
        fallbackRule: typeof group.fallbackRule === "string" && group.fallbackRule.trim().length > 0
          ? group.fallbackRule
          : "If this grouped inspect read fails, read each listed field through requestManifest refs as a targeted fallback.",
      };
    })
    .filter((group): group is InspectRecoveryReadGroup => group !== null);
}

function fieldGroupsFromAgentAction(value: unknown, requestRef: string, request: Record<string, unknown>): InspectRecoveryReadGroup[] {
  const normalized = normalizeAgentActionForRequest(value, request);
  if (!isRecord(normalized) || !isRecord(normalized.read) || !Array.isArray(normalized.read.fieldGroups)) {
    return [];
  }
  return normalized.read.fieldGroups
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const fields = Array.isArray(group.fields)
        ? [...new Set(group.fields.filter((field): field is string => typeof field === "string" && field.trim().length > 0).map((field) => field.trim()))]
        : [];
      if (fields.length === 0) {
        return null;
      }
      return {
        groupId: typeof group.groupId === "string" && group.groupId.trim().length > 0 ? group.groupId : "request_fields",
        required: typeof group.required === "boolean" ? group.required : true,
        purpose: typeof group.purpose === "string" ? group.purpose : "Request fields required by the current loom action.",
        whenToRead: typeof group.whenToRead === "string" ? group.whenToRead : "Before acting on the current loom request.",
        fields,
        readCommand: {
          name: "inspect" as const,
          argv: ["inspect", "--request", requestRef, "--field", fields.join(",")],
        },
        commandInvocation: {
          name: "inspect" as const,
          argv: ["inspect", "--request", requestRef, "--field", fields.join(",")],
          projectRootRequired: true as const,
          preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
        },
        fallbackRule: typeof group.fallbackRule === "string" && group.fallbackRule.trim().length > 0
          ? group.fallbackRule
          : "If this grouped inspect read fails, read each listed field through requestManifest refs as a targeted fallback.",
      };
    })
    .filter((group): group is InspectRecoveryReadGroup => group !== null);
}

function errorDetailsObject(details: unknown): Record<string, unknown> {
  return isRecord(details) ? details : {};
}

function selectValue(value: unknown, pathParts: string[]): unknown {
  let current = value;
  for (const part of pathParts) {
    if (Array.isArray(current)) {
      const index = Number(part);
      if (!Number.isInteger(index) || index < 0 || index >= current.length) {
        throw invalidArgument("inspect field array index is invalid or out of range.", {
          path: pathParts.join("."),
          segment: part,
        });
      }
      current = current[index];
      continue;
    }
    if (!isRecord(current) || !(part in current)) {
      throw invalidArgument("inspect field was not found.", {
        path: pathParts.join("."),
        missingSegment: part,
        availableKeys: isRecord(current) ? Object.keys(current) : [],
      });
    }
    current = current[part];
  }
  return current;
}

function resolveProjectFile(projectRoot: string, fileRef: string): string {
  return path.isAbsolute(fileRef)
    ? fileRef
    : path.resolve(projectRoot, fileRef);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function recordInspectTelemetry(
  projectRoot: string,
  requestFile: string,
  requestRef: string,
  data: Record<string, unknown>,
  resolvedFields: Record<string, {
    status: "resolved" | "not_available";
    value: unknown;
    fieldRead: {
      status: "resolved" | "not_available";
      resolvedRefKey: string | null;
      resolvedRef: string | null;
      selector: string;
      source: "request_root" | "request_manifest_ref";
      unavailableReason?: string;
    };
  }>,
): Promise<void> {
  try {
    await recordTokenSavingEvent({
      projectRoot,
      source: "inspect_selectors",
      command: "inspect",
      artifactRef: requestRef,
      fullBytes: prettyJsonByteLength(await hydrateRequestManifest(projectRoot, requestFile)),
      compactBytes: prettyJsonByteLength(data),
      metadata: {
        fieldCount: Object.keys(resolvedFields).length,
        fields: Object.keys(resolvedFields),
        sources: [...new Set(Object.values(resolvedFields).map((field) => field.fieldRead.source))],
        resolvedRefKeys: [
          ...new Set(
            Object.values(resolvedFields)
              .map((field) => field.fieldRead.resolvedRefKey)
              .filter((refKey): refKey is string => typeof refKey === "string" && refKey.length > 0),
          ),
        ],
      },
    });
  } catch {
    // Telemetry must never block inspect reads.
  }
}
