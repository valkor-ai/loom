export const KNOWLEDGE_SEMANTIC_ANCHOR_RULES = [
  "Every semantic label or semanticFocus text must be self-contained: when read without neighboring labels, focus entries, or surrounding chat, it must still identify the business object, relationship, operation, rule, state, field, page, or flow.",
  "For operation labels and operation focus entries, include the business object, relationship, lifecycle, or workflow qualifier in the operation text itself whenever that qualifier is present in the chunk, heading, current scope item, or page-operation path. Do not rely on a separate object focus entry to qualify a bare operation word.",
  "For rule and state labels or focus entries, use the complete business rule or state-change phrase supported by the chunk or current scope, including its object, condition, or result when present; do not use context-dependent names such as condition, blocker, status, validation, or result by themselves.",
];

export const KNOWLEDGE_SEMANTIC_ANCHOR_RULE_TEXT = KNOWLEDGE_SEMANTIC_ANCHOR_RULES.join(" ");
