export type RequirementSourceItem =
  | {
      itemId: string;
      kind: "text";
      origin: "user_message" | "stdin" | "cli_option";
      title?: string;
      textRef: string;
      extractionStatus: "completed";
      digest: string;
      characterCount: number;
    }
  | {
      itemId: string;
      kind: "file";
      origin: "request_file" | "context_file";
      title?: string;
      path: string;
      mimeType: string;
      textRef?: string;
      extractedTextRef?: string;
      extractionStatus: "completed" | "unsupported" | "failed";
      extractionReason?: string;
      digest: string;
      textDigest?: string;
      characterCount?: number;
    };

export type RequirementContext = {
  schemaVersion: "1.0";
  deliveryId: string;
  createdAt: string;
  sourceItems: RequirementSourceItem[];
  normalizedTextRef: string | null;
  normalizedTextStatus: "completed" | "empty";
  normalizedTextReason?: string;
  keywordHintsRef: string | null;
  keywordHintsStatus: "completed" | "skipped" | "empty" | "failed";
  keywordHintsReason?: string;
};

export type RequirementKeywordHint = {
  keyword: string;
  score: number;
  occurrences: number;
  sourceItemIds: string[];
  sampleContexts: string[];
};

export type RequirementKeywordHintCompact = {
  keyword: string;
  occurrences: number;
  sourceItemIds: string[];
};

export type RequirementSectionKeywordHints = {
  sectionId: string;
  sourceItemId: string;
  title?: string;
  keywords: RequirementKeywordHint[];
};

export type RequirementKeywordHintsCompact = {
  usage: "advisory_only";
  status: "completed" | "empty";
  languageHints: string[];
  topKeywords: RequirementKeywordHintCompact[];
  sectionKeywords: Array<{
    sectionId: string;
    sourceItemId: string;
    title?: string;
    keywords: string[];
  }>;
  rules: {
    advisoryOnly: true;
    mustNotTreatAsScope: true;
    mustNotTreatAsAcceptance: true;
    ignoreWhenIrrelevant: true;
  };
};

export type RequirementKeywordHints = {
  schemaVersion: "1.0";
  deliveryId: string;
  source: "tfidf_keyword_hints";
  usage: "advisory_only";
  status: "completed" | "empty";
  generatedAt: string;
  languageHints: string[];
  sourceTextRefs: string[];
  extraction: {
    method: "local_tfidf";
    chunkCount: number;
    globalLimit: number;
    sectionLimit: number;
  };
  globalKeywords: RequirementKeywordHint[];
  sectionKeywords: RequirementSectionKeywordHints[];
  compact: RequirementKeywordHintsCompact;
  rules: {
    advisoryOnly: true;
    mustNotTreatAsScope: true;
    mustNotTreatAsAcceptance: true;
    mustNotTreatAsConfirmedConcept: true;
    ignoreWhenIrrelevant: true;
  };
};
