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
    const request = await readJsonFile(requestFile);
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
    await recordInspectTelemetry(ctx.projectRoot, requestFile, requestRef, data, resolvedFields);
    return ok("inspect", ctx.projectRoot, data, fields.length === 1 ? "Field inspected." : "Fields inspected.");
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
    const value = selectorParts.length === 0 ? normalizedRefValue : selectValue(normalizedRefValue, selectorParts);
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
  const rootValue = selectValue(normalizedRequest, parts);
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
  const value = selectorParts.length === 0 ? refValue : selectValue(refValue, selectorParts);
  return {
    status: "resolved",
    value,
    resolvedRefKey: `contextRefs.${spec.refField}`,
    resolvedRef: ref,
    selector: selectorParts.length === 0 ? "$" : `.${selectorParts.join(".")}`,
    source: "request_root",
  };
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
  const readPlan = await resolveAgentActionReadPlan(projectRoot, requestRef, request);
  const requiredGroup = readPlan.availableFieldGroups.find((group) => group.required)
    ?? readPlan.availableFieldGroups[0];
  return {
    status: "field_not_found_use_request_read_plan",
    requestedFields,
    requestRef,
    readPlanAuthority: "agentAction.read.fieldGroups",
    agentActionSource: readPlan.source,
    ...(readPlan.ref ? { agentActionRef: readPlan.ref } : {}),
    ...(readPlan.readError ? { agentActionReadError: readPlan.readError } : {}),
    availableFieldGroups: readPlan.availableFieldGroups,
    recommendedNextRead: requiredGroup
      ? {
        reason: "The requested inspect field is not part of this request contract. Read the next required fieldGroup instead of guessing legacy root fields.",
        groupId: requiredGroup.groupId,
        commandInvocation: requiredGroup.commandInvocation,
      }
      : {
        reason: "No agentAction.read.fieldGroups were found. Read requestManifest refs for the required root keys before falling back to the request file.",
        commandInvocation: {
          name: "inspect",
          argv: ["inspect", "--request", requestRef, "--field", "requestManifest"],
          projectRootRequired: true,
          preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
        },
      },
    fallbackRule: "If the recommended inspect command fails, read the listed fieldGroup fields through requestManifest refs and targeted selectors. If the read plan is missing or unreadable, read requestRef and requestManifest refs directly as a correctness fallback while keeping chat output compact.",
    doNot: [
      "Do not guess old wrapper fields such as phaseScopePrompt, data, contract, objective, scope, or outputContract when they are not listed in agentAction.read.fieldGroups.",
      "Do not run broad searches over $HOME/.loom, .codex, node_modules, unrelated test directories, or the whole project to discover request fields.",
      "Do not print full .loom request, TaskPlan, run, result, or ref JSON into chat.",
    ],
    availableTopLevelFields: Object.keys(request),
    requestManifestRefKeys: Object.keys(requestManifestRefs(request)),
  };
}

async function resolveAgentActionReadPlan(
  projectRoot: string,
  requestRef: string,
  request: Record<string, unknown>,
): Promise<{
  source: "request_root" | "request_manifest_ref" | "missing";
  ref: string | null;
  readError?: string;
  availableFieldGroups: InspectRecoveryReadGroup[];
}> {
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
