from __future__ import annotations

import re
from collections.abc import Iterable

import jieba

_LATIN_RE = re.compile(r"[a-z0-9_]+", re.IGNORECASE)
_CJK_RE = re.compile(r"[\u3400-\u9fff]+")


def analyze(text: str) -> list[str]:
    """Tokenize mixed Chinese/Latin text with stable fallback n-grams."""
    normalized = text.casefold()
    tokens: list[str] = []

    tokens.extend(_latin_tokens(normalized))
    for segment in _CJK_RE.findall(normalized):
        tokens.extend(_jieba_tokens(segment))
        tokens.extend(_cjk_ngrams(segment, 2))
        tokens.extend(_cjk_ngrams(segment, 3))

    return [token for token in tokens if token.strip()]


def _latin_tokens(text: str) -> Iterable[str]:
    return (match.group(0) for match in _LATIN_RE.finditer(text))


def _jieba_tokens(segment: str) -> Iterable[str]:
    return (token.strip() for token in jieba.lcut(segment) if token.strip())


def _cjk_ngrams(segment: str, size: int) -> Iterable[str]:
    if len(segment) < size:
        return []
    return (segment[index : index + size] for index in range(0, len(segment) - size + 1))
