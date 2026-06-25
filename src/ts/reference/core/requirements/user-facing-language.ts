import type { LoomConfigV1, RequirementInput, UserFacingLanguageConstraint } from "../schemas";

type ConfiguredLanguage = LoomConfigV1["defaults"]["language"];

const APPLIES_TO = [
  "navigation labels",
  "page titles and headings",
  "form labels and placeholders",
  "button and action labels",
  "table/list/search labels",
  "success, validation, error, and business-blocking messages",
  "visible status/result text",
];

const DOES_NOT_APPLY_TO = [
  "source code identifiers",
  "API paths and payload field names",
  "database tables, columns, and enum values",
  "package names and framework conventions",
  "internal artifact names or technical ids",
];

export function inferUserFacingLanguage(input: {
  requirementInput: RequirementInput;
  configuredLanguage: ConfiguredLanguage;
}): UserFacingLanguageConstraint {
  if (input.configuredLanguage === "zh") {
    return languageConstraint("zh-CN", "project_default");
  }
  if (input.configuredLanguage === "en") {
    return languageConstraint("en", "project_default");
  }

  const locale = inferLocaleFromRequirement(input.requirementInput);
  return languageConstraint(locale, locale === "und" ? "fallback" : "requirement_primary_language");
}

export function inferUserFacingLanguageFromText(text: string): UserFacingLanguageConstraint {
  const locale = inferLocaleFromText(text);
  return languageConstraint(locale, locale === "und" ? "fallback" : "requirement_primary_language");
}

export function userFacingLanguageRule(language: UserFacingLanguageConstraint | null | undefined): string {
  return ruleForLocale(language?.defaultLocale ?? "und");
}

function ruleForLocale(defaultLocale: UserFacingLanguageConstraint["defaultLocale"]): string {
  if (defaultLocale === "und") {
    return "No explicit user-facing language was inferred. Keep user-visible copy aligned with the confirmed requirement wording or existing product baseline; do not translate technical identifiers.";
  }
  const localeName = defaultLocale === "zh-CN" ? "Chinese (Simplified)" : "English";
  return `User-facing UI copy must default to ${localeName}. Apply this to labels, navigation, form text, buttons, table/search labels, visible status text, success messages, validation errors, and business-blocking feedback. Do not translate code identifiers, API paths, database fields, enum values, package/framework names, or internal artifact ids.`;
}

function languageConstraint(
  defaultLocale: UserFacingLanguageConstraint["defaultLocale"],
  source: UserFacingLanguageConstraint["source"],
): UserFacingLanguageConstraint {
  return {
    defaultLocale,
    source,
    appliesTo: APPLIES_TO,
    doesNotApplyTo: DOES_NOT_APPLY_TO,
    rule: ruleForLocale(defaultLocale),
  };
}

function inferLocaleFromRequirement(input: RequirementInput): UserFacingLanguageConstraint["defaultLocale"] {
  const requestText = [
    input.primaryRequest,
    ...input.requestSources.map((source) => source.content),
  ].join("\n");
  return inferLocaleFromText(requestText);
}

function inferLocaleFromText(text: string): UserFacingLanguageConstraint["defaultLocale"] {
  const counts = countLanguageSignals(text);
  if (counts.han > 0 && (counts.han >= 20 || counts.han * 3 >= counts.latin)) {
    return "zh-CN";
  }
  if (counts.latin > 0 && counts.han === 0) {
    return "en";
  }
  if (counts.han > 0 && counts.latin === 0) {
    return "zh-CN";
  }
  return "und";
}

function countLanguageSignals(text: string): { han: number; latin: number } {
  return {
    han: text.match(/[\p{Script=Han}]/gu)?.length ?? 0,
    latin: text.match(/[A-Za-z]/g)?.length ?? 0,
  };
}
