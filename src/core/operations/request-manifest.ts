import path from "node:path";
import { ensureDir, readJsonFile, writeJsonAtomic } from "../state/fs";
import { toProjectRelative } from "../state/paths";
import { normalizeAgentActionForRequest } from "./agent-action";
import { prettyJsonByteLength, recordTokenSavingEvent } from "./token-saving-telemetry";

const DEFAULT_REF_KEYS = [
  "agentAction",
  "referencedArtifactReadGuide",
  "generationProtocol",
  "generationRules",
  "fieldAccessHints",
  "requestOptimization",
  "validatorRulesSummary",
  "validatorPolicy",
  "executionRules",
  "reviewRules",
  "enumRefs",
  "allowedRefs",
  "rules",
  "sourceRefs",
  "contextProjection",
  "sourceContracts",
  "sourceContext",
  "executionArtifacts",
  "changeSet",
  "reviewScope",
  "task",
  "taskConceptGrounding",
  "outputContract",
  "blockedOutput",
] as const;

export type RequestManifestOptions = {
  refKeys?: readonly string[];
};

export async function writeRequestManifestAtomic<T extends Record<string, unknown>>(
  projectRoot: string,
  requestFile: string,
  request: T,
  options: RequestManifestOptions = {},
): Promise<T> {
  await ensureRequestOutputParentDirs(projectRoot, request);
  const manifest = await buildRequestManifest(projectRoot, requestFile, request, options);
  await writeJsonAtomic(requestFile, manifest);
  return request;
}

export async function hydrateRequestManifest(projectRoot: string, requestFile: string): Promise<unknown> {
  const request = await readJsonFile(requestFile);
  if (!isRecord(request)) {
    return request;
  }
  return hydrateRequestManifestValue(projectRoot, request, requestFile);
}

export async function hydrateRequestManifestValue(projectRoot: string, request: Record<string, unknown>, requestFile?: string): Promise<Record<string, unknown>> {
  const hydrated: Record<string, unknown> = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || value.length === 0) {
      continue;
    }
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) {
      continue;
    }
    hydrated[targetKey] = await readJsonFile(path.resolve(projectRoot, value));
  }
  const manifestRefs = isRecord(request.requestManifest) && isRecord(request.requestManifest.refs)
    ? request.requestManifest.refs
    : {};
  for (const [targetKey, entry] of Object.entries(manifestRefs)) {
    if (targetKey in hydrated || !isRecord(entry) || typeof entry.ref !== "string" || entry.ref.length === 0) {
      continue;
    }
    hydrated[targetKey] = await readJsonFile(path.resolve(projectRoot, entry.ref));
  }
  if (isRecord(hydrated.agentAction)) {
    hydrated.agentAction = normalizeAgentActionForRequest(hydrated.agentAction, hydrated);
  }
  const requestReadPlan = isRecord(hydrated.requestReadPlan)
    ? hydrated.requestReadPlan
    : requestFile
      ? buildInlineRequestReadPlan(projectRoot, requestFile, hydrated.agentAction)
      : null;
  if (requestReadPlan) {
    hydrated.requestReadPlan = requestReadPlan;
    if (isRecord(hydrated.agentAction)) {
      hydrated.agentAction = compactAgentActionForExecutionRef(hydrated.agentAction);
    }
  }
  return hydrated;
}

async function buildRequestManifest<T extends Record<string, unknown>>(
  projectRoot: string,
  requestFile: string,
  request: T,
  options: RequestManifestOptions,
): Promise<T> {
  const manifest = JSON.parse(JSON.stringify(request)) as Record<string, unknown>;
  if (isRecord(manifest.agentAction)) {
    manifest.agentAction = normalizeAgentActionForRequest(manifest.agentAction, manifest);
  }
  const requestReadPlan = buildInlineRequestReadPlan(projectRoot, requestFile, manifest.agentAction);
  const fullRequestBytes = prettyJsonByteLength(manifest);
  const refKeys = options.refKeys ?? DEFAULT_REF_KEYS;
  const refsDir = requestRefsDir(requestFile);
  const refs: Record<string, {
    refKey: string;
    ref: string;
    purpose?: string;
    requiredSelectors?: string[];
    rule?: string;
  }> = {};
  for (const key of refKeys) {
    if (!(key in manifest)) {
      continue;
    }
    const value = key === "agentAction" && requestReadPlan
      ? compactAgentActionForExecutionRef(manifest[key])
      : manifest[key];
    if (value === undefined || value === null) {
      continue;
    }
    const refFile = path.join(refsDir, `${kebabCase(key)}.json`);
    await writeJsonAtomic(refFile, value);
    delete manifest[key];
    const refKey = `${key}Ref`;
    const ref = toProjectRelative(projectRoot, refFile);
    manifest[refKey] = ref;
    refs[key] = {
      refKey,
      ref,
      ...refManifestMetadata(key),
    };
  }
  manifest.requestManifest = {
    schemaVersion: "1.0",
    refFirst: true,
    protocolAuthority: "request_manifest_refs",
    refs,
    rule: "Use requestReadPlan.groups as the request field read contract. Run its loom inspect commands for grouped field values. requestManifest refs are storage refs for inspect and targeted recovery only; do not read them to discover a second read plan. Do not invent or probe unlisted sidecar files under the .refs directory.",
  };
  if (requestReadPlan) {
    manifest.requestReadPlan = requestReadPlan;
  }
  await recordTokenSavingEvent({
    projectRoot,
    source: "request_manifest_refs",
    artifactRef: toProjectRelative(projectRoot, requestFile),
    fullBytes: fullRequestBytes,
    compactBytes: prettyJsonByteLength(manifest),
    metadata: {
      refCount: Object.keys(refs).length,
      refKeys: Object.keys(refs),
    },
  });
  return manifest as T;
}

function compactAgentActionForExecutionRef(value: unknown): unknown {
  if (!isRecord(value)) {
    return value;
  }
  const { read: _read, ...executionAction } = value;
  return {
    ...executionAction,
  };
}

function buildInlineRequestReadPlan(
  projectRoot: string,
  requestFile: string,
  agentAction: unknown,
): Record<string, unknown> | null {
  if (!isRecord(agentAction) || !isRecord(agentAction.read) || !Array.isArray(agentAction.read.fieldGroups)) {
    return null;
  }
  const requestRef = toProjectRelative(projectRoot, requestFile);
  const groups = agentAction.read.fieldGroups
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const fields = Array.isArray(group.fields)
        ? [...new Set(group.fields.filter((field): field is string => typeof field === "string" && field.trim().length > 0))]
        : [];
      if (fields.length === 0) {
        return null;
      }
      const argv = ["inspect", "--request", requestRef, "--field", fields.join(",")];
      return {
        groupId: typeof group.groupId === "string" ? group.groupId : "request_fields",
        required: typeof group.required === "boolean" ? group.required : true,
        purpose: typeof group.purpose === "string" ? group.purpose : "Request fields required by the current loom action.",
        whenToRead: typeof group.whenToRead === "string" ? group.whenToRead : "Before acting on the current loom request.",
        fields,
        readCommand: {
          name: "inspect",
          argv,
        },
        fallbackRule: "If this inspect read fails, read only these exact fields through requestManifest refs with short targeted selectors. Do not print full .loom artifacts.",
      };
    })
    .filter((group) => group !== null);
  if (groups.length === 0) {
    return null;
  }
  return {
    schemaVersion: "1.0",
    authority: "requestReadPlan.groups",
    primaryMethod: "loom inspect",
    requestRef,
    groups,
    rules: [
      "Use this requestReadPlan as the only detailed read contract for this request.",
      "Run required groups with loom inspect and use data.fields[field].value as the field value.",
      "Do not use shell selectors or full-file reads on .loom refs during the normal path.",
      "If an inspect command fails or a required field is missing, fall back only to the fields listed in that same group with targeted selectors.",
      "Full .loom artifact reads are a last-resort correctness fallback and must not be printed into chat.",
    ],
  };
}

function refManifestMetadata(key: string): {
  purpose?: string;
  requiredSelectors?: string[];
  rule?: string;
} {
  const metadata: Record<string, { purpose: string; requiredSelectors?: string[]; rule?: string }> = {
    agentAction: {
      purpose: "Execution action map: what to write, how to submit, schema authority, and stop conditions. It is not a request read plan.",
      requiredSelectors: [".actionKind", ".instruction", ".write", ".submit", ".schema", ".stopConditions"],
      rule: "Use root requestReadPlan.groups for request field reads. agentAction is execution metadata, not a second read contract.",
    },
    outputContract: {
      purpose: "Complete output path and schema authority. For architecture section generation, section schemas and enums live here under .sectionOutputs[].schemaShape and .sectionOutputs[].enumRefs.",
      requiredSelectors: [".candidateFile", ".resultFile", ".outlineFile", ".groupFilePattern", ".sectionOutputs[].section", ".sectionOutputs[].candidateFile", ".sectionOutputs[].schemaShape", ".sectionOutputs[].enumRefs"],
      rule: "Do not look for separate section schema refs such as section-schemas.json. Prefer requestReadPlan/inspect fields over full ref reads.",
    },
    fieldAccessHints: {
      purpose: "Selector hints for reading this request and its refs without guessing old wrapper roots or ref names.",
      requiredSelectors: [".commonSelectors"],
      rule: "Prefer requestReadPlan/inspect groups. Use this ref only for targeted selector recovery; do not print the full sidecar.",
    },
    referencedArtifactReadGuide: {
      purpose: "Selector map for every external source/context ref.",
      requiredSelectors: [".[].refKey", ".[].refPath", ".[].requiredSelectors", ".[].doNotGuessAlternateRoots"],
    },
    sourceRefs: {
      purpose: "Authoritative source artifact paths for this request.",
      requiredSelectors: [".*Ref"],
    },
    contextProjection: {
      purpose: "Request-scoped projection of existing authority artifacts for the current operation. Read only the fields listed in root requestReadPlan.groups.",
      rule: "This is not a parallel authority model. Use requestReadPlan/inspect groups to read selected fields; do not print this full ref.",
    },
    enumRefs: {
      purpose: "Allowed enum values for generated candidate/result fields.",
      rule: "Read only the enum sets named by requestReadPlan/inspect fields or the output schema currently being written.",
    },
    allowedRefs: {
      purpose: "Allowed business/scope/acceptance refs for generated candidate fields.",
      requiredSelectors: [".scopeRefs", ".acceptanceRefs", ".deferredScopeRefs", ".excludedScopeRefs"],
    },
    generationProtocol: {
      purpose: "Execution boundaries, output policy, and submit rules for candidate generation.",
      requiredSelectors: [".readRequestBeforeActing", ".writeCandidateFileOnly", ".submitWithProvidedCommand", ".chatOutputPolicy"],
    },
    rules: {
      purpose: "Generation and validation rules that are too large for the routing instruction.",
      rule: "Read only the rule sections named by requestReadPlan/inspect fields. Do not use this ref as a full-file default read.",
    },
  };
  return metadata[key] ?? {};
}

function requestRefsDir(requestFile: string): string {
  const parsed = path.parse(requestFile);
  return path.join(parsed.dir, `${parsed.name}.refs`);
}

function kebabCase(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

const OUTPUT_FILE_KEYS = new Set([
  "candidateFile",
  "blockedFile",
  "resultFile",
  "outlineFile",
  "groupFilePattern",
  "targetCandidateFile",
]);

async function ensureRequestOutputParentDirs(projectRoot: string, request: Record<string, unknown>): Promise<void> {
  const refs = new Set<string>();
  collectOutputFileRefs(request, refs);
  for (const ref of refs) {
    await ensureDir(path.dirname(path.resolve(projectRoot, ref)));
  }
}

function collectOutputFileRefs(value: unknown, refs: Set<string>): void {
  if (Array.isArray(value)) {
    for (const item of value) {
      collectOutputFileRefs(item, refs);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    if (OUTPUT_FILE_KEYS.has(key) && typeof child === "string" && isConcreteOutputRef(child)) {
      refs.add(child);
      continue;
    }
    collectOutputFileRefs(child, refs);
  }
}

function isConcreteOutputRef(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.length > 0 && !trimmed.startsWith("{") && !trimmed.includes("://");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
