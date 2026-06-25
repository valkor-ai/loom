import type {
  RequirementKeywordHint,
  RequirementKeywordHints,
  RequirementKeywordHintsCompact,
  RequirementSectionKeywordHints,
} from "./types";

type KeywordSourceText = {
  sourceItemId: string;
  title?: string;
  textRef: string;
  text: string;
};

type Chunk = {
  chunkId: string;
  sourceItemId: string;
  title?: string;
  text: string;
  terms: Map<string, number>;
};

const GLOBAL_LIMIT = 50;
const SECTION_LIMIT = 8;
const CHUNK_SIZE = 1800;
const CHUNK_OVERLAP = 120;

const EN_STOPWORDS = new Set([
  "the", "and", "for", "with", "that", "this", "from", "are", "was", "were", "will", "shall",
  "into", "onto", "their", "there", "about", "after", "before", "between", "within", "without",
  "when", "where", "what", "which", "while", "should", "could", "would", "have", "has", "had",
  "not", "can", "may", "use", "using", "used", "user", "system", "page", "data",
]);

const ZH_STOPWORDS = new Set([
  "一个", "一种", "一些", "以及", "进行", "通过", "需要", "可以", "应该", "如果", "然后", "这个",
  "那个", "用户", "系统", "功能", "模块", "页面", "数据", "信息", "相关", "支持", "实现", "根据",
]);

export function generateKeywordHints(input: {
  deliveryId: string;
  generatedAt: string;
  sources: KeywordSourceText[];
}): RequirementKeywordHints {
  const chunks = input.sources.flatMap((source) => chunkSource(source));
  const sourceTextRefs = input.sources.map((source) => source.textRef);
  if (chunks.length === 0) {
    return emptyHints(input.deliveryId, input.generatedAt, sourceTextRefs);
  }

  const documentFrequency = new Map<string, number>();
  for (const chunk of chunks) {
    for (const term of chunk.terms.keys()) {
      documentFrequency.set(term, (documentFrequency.get(term) ?? 0) + 1);
    }
  }

  const scored = new Map<string, RequirementKeywordHint>();
  for (const chunk of chunks) {
    const totalTerms = [...chunk.terms.values()].reduce((sum, count) => sum + count, 0) || 1;
    for (const [term, count] of chunk.terms) {
      const idf = Math.log((chunks.length + 1) / ((documentFrequency.get(term) ?? 0) + 1)) + 1;
      const score = (count / totalTerms) * idf;
      const existing = scored.get(term);
      const sample = sampleContext(chunk.text, term);
      if (existing) {
        existing.score += score;
        existing.occurrences += count;
        if (!existing.sourceItemIds.includes(chunk.sourceItemId)) {
          existing.sourceItemIds.push(chunk.sourceItemId);
        }
        if (sample && existing.sampleContexts.length < 3 && !existing.sampleContexts.includes(sample)) {
          existing.sampleContexts.push(sample);
        }
      } else {
        scored.set(term, {
          keyword: term,
          score,
          occurrences: count,
          sourceItemIds: [chunk.sourceItemId],
          sampleContexts: sample ? [sample] : [],
        });
      }
    }
  }

  const globalKeywords = sortHints([...scored.values()]).slice(0, GLOBAL_LIMIT);
  const sectionKeywords: RequirementSectionKeywordHints[] = input.sources
    .map((source) => {
      const sectionScored = new Map<string, RequirementKeywordHint>();
      for (const chunk of chunks.filter((item) => item.sourceItemId === source.sourceItemId)) {
        const totalTerms = [...chunk.terms.values()].reduce((sum, count) => sum + count, 0) || 1;
        for (const [term, count] of chunk.terms) {
          const idf = Math.log((chunks.length + 1) / ((documentFrequency.get(term) ?? 0) + 1)) + 1;
          const score = (count / totalTerms) * idf;
          const sample = sampleContext(chunk.text, term);
          const existing = sectionScored.get(term);
          if (existing) {
            existing.score += score;
            existing.occurrences += count;
            if (sample && existing.sampleContexts.length < 2 && !existing.sampleContexts.includes(sample)) {
              existing.sampleContexts.push(sample);
            }
          } else {
            sectionScored.set(term, {
              keyword: term,
              score,
              occurrences: count,
              sourceItemIds: [source.sourceItemId],
              sampleContexts: sample ? [sample] : [],
            });
          }
        }
      }
      return {
        sectionId: `section-${source.sourceItemId}`,
        sourceItemId: source.sourceItemId,
        ...(source.title ? { title: source.title } : {}),
        keywords: sortHints([...sectionScored.values()]).slice(0, SECTION_LIMIT),
      };
    })
    .filter((section) => section.keywords.length > 0);

  const roundedGlobalKeywords = globalKeywords.map(roundHint);
  const roundedSectionKeywords = sectionKeywords.map((section) => ({
    ...section,
    keywords: section.keywords.map(roundHint),
  }));
  const status = globalKeywords.length > 0 ? "completed" : "empty";
  const languageHints = detectLanguages(input.sources.map((source) => source.text).join("\n"));
  const compact = compactKeywordHints(status, languageHints, roundedGlobalKeywords, roundedSectionKeywords);

  return {
    schemaVersion: "1.0",
    deliveryId: input.deliveryId,
    source: "tfidf_keyword_hints",
    usage: "advisory_only",
    status,
    generatedAt: input.generatedAt,
    languageHints,
    sourceTextRefs,
    extraction: {
      method: "local_tfidf",
      chunkCount: chunks.length,
      globalLimit: GLOBAL_LIMIT,
      sectionLimit: SECTION_LIMIT,
    },
    globalKeywords: roundedGlobalKeywords,
    sectionKeywords: roundedSectionKeywords,
    compact,
    rules: {
      advisoryOnly: true,
      mustNotTreatAsScope: true,
      mustNotTreatAsAcceptance: true,
      mustNotTreatAsConfirmedConcept: true,
      ignoreWhenIrrelevant: true,
    },
  };
}

function emptyHints(deliveryId: string, generatedAt: string, sourceTextRefs: string[]): RequirementKeywordHints {
  const compact = compactKeywordHints("empty", [], [], []);
  return {
    schemaVersion: "1.0",
    deliveryId,
    source: "tfidf_keyword_hints",
    usage: "advisory_only",
    status: "empty",
    generatedAt,
    languageHints: [],
    sourceTextRefs,
    extraction: {
      method: "local_tfidf",
      chunkCount: 0,
      globalLimit: GLOBAL_LIMIT,
      sectionLimit: SECTION_LIMIT,
    },
    globalKeywords: [],
    sectionKeywords: [],
    compact,
    rules: {
      advisoryOnly: true,
      mustNotTreatAsScope: true,
      mustNotTreatAsAcceptance: true,
      mustNotTreatAsConfirmedConcept: true,
      ignoreWhenIrrelevant: true,
    },
  };
}

function chunkSource(source: KeywordSourceText): Chunk[] {
  const normalized = source.text.replace(/\r/g, "").replace(/[ \t]+/g, " ").trim();
  if (!normalized) {
    return [];
  }
  const chunks: Chunk[] = [];
  for (let start = 0; start < normalized.length; start += CHUNK_SIZE - CHUNK_OVERLAP) {
    const text = normalized.slice(start, start + CHUNK_SIZE).trim();
    if (!text) {
      continue;
    }
    const terms = extractTerms(text);
    if (terms.size === 0) {
      continue;
    }
    chunks.push({
      chunkId: `${source.sourceItemId}-${chunks.length + 1}`,
      sourceItemId: source.sourceItemId,
      title: source.title,
      text,
      terms,
    });
  }
  return chunks;
}

function extractTerms(text: string): Map<string, number> {
  const terms = new Map<string, number>();
  for (const term of [...englishTerms(text), ...chineseTerms(text)]) {
    if (term.length < 2 || EN_STOPWORDS.has(term) || ZH_STOPWORDS.has(term)) {
      continue;
    }
    terms.set(term, (terms.get(term) ?? 0) + 1);
  }
  return terms;
}

function englishTerms(text: string): string[] {
  const words = (text.toLowerCase().match(/[a-z][a-z0-9_-]{2,}/g) ?? [])
    .filter((word) => !EN_STOPWORDS.has(word));
  const terms = [...words];
  for (let n = 2; n <= 3; n += 1) {
    for (let index = 0; index <= words.length - n; index += 1) {
      const phrase = words.slice(index, index + n);
      if (phrase.some((word) => EN_STOPWORDS.has(word))) {
        continue;
      }
      terms.push(phrase.join(" "));
    }
  }
  return terms;
}

function chineseTerms(text: string): string[] {
  const segments = segmentChinese(text);
  const terms = segments.filter((segment) => segment.length >= 2 && !ZH_STOPWORDS.has(segment));
  for (let n = 2; n <= 3; n += 1) {
    for (let index = 0; index <= segments.length - n; index += 1) {
      const phrase = segments.slice(index, index + n).join("");
      if (phrase.length >= 2 && !ZH_STOPWORDS.has(phrase)) {
        terms.push(phrase);
      }
    }
  }
  return terms;
}

function segmentChinese(text: string): string[] {
  const zhOnly = text.match(/[\p{Script=Han}]+/gu)?.join(" ") ?? "";
  if (!zhOnly) {
    return [];
  }
  const Segmenter = Intl.Segmenter;
  if (Segmenter) {
    const segmenter = new Segmenter("zh", { granularity: "word" });
    return [...segmenter.segment(zhOnly)]
      .filter((segment) => segment.isWordLike)
      .map((segment) => segment.segment.trim())
      .filter(Boolean);
  }
  const compact = zhOnly.replace(/\s+/g, "");
  const terms: string[] = [];
  for (let n = 2; n <= 4; n += 1) {
    for (let index = 0; index <= compact.length - n; index += 1) {
      terms.push(compact.slice(index, index + n));
    }
  }
  return terms;
}

function sampleContext(text: string, term: string): string | null {
  const lower = text.toLowerCase();
  const index = lower.indexOf(term.toLowerCase());
  if (index < 0) {
    return null;
  }
  const start = Math.max(0, index - 60);
  const end = Math.min(text.length, index + term.length + 80);
  return text.slice(start, end).replace(/\s+/g, " ").trim().slice(0, 160);
}

function sortHints(hints: RequirementKeywordHint[]): RequirementKeywordHint[] {
  return hints.sort((a, b) => {
    const byScore = b.score - a.score;
    if (byScore !== 0) {
      return byScore;
    }
    const byOccurrences = b.occurrences - a.occurrences;
    if (byOccurrences !== 0) {
      return byOccurrences;
    }
    return a.keyword.localeCompare(b.keyword);
  });
}

function roundHint(hint: RequirementKeywordHint): RequirementKeywordHint {
  return {
    ...hint,
    score: Number(hint.score.toFixed(6)),
  };
}

function compactKeywordHints(
  status: RequirementKeywordHintsCompact["status"],
  languageHints: string[],
  globalKeywords: RequirementKeywordHint[],
  sectionKeywords: RequirementSectionKeywordHints[],
): RequirementKeywordHintsCompact {
  return {
    usage: "advisory_only",
    status,
    languageHints,
    topKeywords: globalKeywords.slice(0, 16).map((hint) => ({
      keyword: hint.keyword,
      occurrences: hint.occurrences,
      sourceItemIds: hint.sourceItemIds.slice(0, 3),
    })),
    sectionKeywords: sectionKeywords.slice(0, 6).map((section) => ({
      sectionId: section.sectionId,
      sourceItemId: section.sourceItemId,
      ...(section.title ? { title: section.title } : {}),
      keywords: section.keywords.slice(0, 6).map((hint) => hint.keyword),
    })),
    rules: {
      advisoryOnly: true,
      mustNotTreatAsScope: true,
      mustNotTreatAsAcceptance: true,
      ignoreWhenIrrelevant: true,
    },
  };
}

function detectLanguages(text: string): string[] {
  const languages: string[] = [];
  if (/[a-zA-Z]/.test(text)) {
    languages.push("en");
  }
  if (/[\p{Script=Han}]/u.test(text)) {
    languages.push("zh");
  }
  return languages;
}
