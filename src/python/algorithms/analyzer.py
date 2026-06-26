from __future__ import annotations

import re
from collections.abc import Iterable

import jieba

_LATIN_RE = re.compile(r"[a-z0-9_]+", re.IGNORECASE)
_CJK_RE = re.compile(r"[\u3400-\u9fff]+")
_CJK_ONLY_RE = re.compile(r"^[\u3400-\u9fff]+$")
_LATIN_STOPWORDS = {
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "in",
    "into",
    "is",
    "of",
    "on",
    "or",
    "that",
    "the",
    "this",
    "to",
    "with",
}
_CJK_STOPWORDS = {
    "一个",
    "一种",
    "不同",
    "以及",
    "以后",
    "但是",
    "中文",
    "关系",
    "内容",
    "例如",
    "可以",
    "如果",
    "对于",
    "并且",
    "应该",
    "时候",
    "是否",
    "有关",
    "文档",
    "根据",
    "检查",
    "每个",
    "然后",
    "由于",
    "确认",
    "范围",
    "阶段",
    "顺序",
    "的",
    "了",
    "和",
    "在",
    "对",
    "将",
    "当",
    "或",
    "是",
    "有",
    "与",
    "及",
    "由",
    "给",
    "让",
    "把",
    "这",
    "那",
    "该",
    "其",
    "以",
    "为",
    "系统",
    "功能",
    "模块",
    "实现",
    "提供",
    "需要",
    "需求",
    "要求",
    "通过",
    "进行",
    "包括",
    "显示",
    "整理",
    "阅读",
    "仔细",
    "按照",
    "能力",
}


def analyze(text: str) -> list[str]:
    """Tokenize mixed Chinese/Latin text with stable fallback n-grams."""
    normalized = text.casefold()
    tokens: list[str] = []

    tokens.extend(token for token in _latin_tokens(normalized) if keyword_allowed(token))
    for segment in _CJK_RE.findall(normalized):
        cjk_tokens = [token for token in _jieba_tokens(segment) if keyword_allowed(token)]
        tokens.extend(cjk_tokens)
        tokens.extend(_cjk_compounds(cjk_tokens, 2))
        tokens.extend(_cjk_compounds(cjk_tokens, 3))

    return [token for token in tokens if token.strip()]


def _latin_tokens(text: str) -> Iterable[str]:
    return (match.group(0) for match in _LATIN_RE.finditer(text))


def _jieba_tokens(segment: str) -> Iterable[str]:
    return (token.strip() for token in jieba.lcut(segment) if token.strip())


def _cjk_compounds(tokens: list[str], size: int) -> Iterable[str]:
    if len(tokens) < size:
        return []
    compounds: list[str] = []
    for index in range(0, len(tokens) - size + 1):
        parts = tokens[index : index + size]
        compound = "".join(parts)
        if 3 <= len(compound) <= 12 and keyword_allowed(compound):
            compounds.append(compound)
    return compounds


def keyword_allowed(value: str) -> bool:
    token = value.strip().casefold()
    if not token:
        return False
    if token.isdigit():
        return False
    if _CJK_ONLY_RE.match(token):
        if len(token) < 2:
            return False
        return token not in _CJK_STOPWORDS
    if len(token) < 2:
        return False
    return token not in _LATIN_STOPWORDS
