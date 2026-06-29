from __future__ import annotations

from typing import Any

from rank_bm25 import BM25Okapi

from .analyzer import analyze


def rank_bm25(query: str, documents: list[dict[str, Any]], limit: int = 10) -> list[dict[str, Any]]:
    corpus = [analyze(str(document.get("text", ""))) for document in documents]
    query_tokens = analyze(query)
    if not corpus or not query_tokens:
        return []

    ids = [str(document.get("id", index)) for index, document in enumerate(documents)]
    bm25 = BM25Okapi(corpus)
    scores = bm25.get_scores(query_tokens)
    if all(score <= 0 for score in scores):
        query_set = set(query_tokens)
        scores = [
            len(query_set.intersection(tokens)) / max(len(query_set), 1)
            for tokens in corpus
        ]

    results = [
        {"documentId": document_id, "score": round(float(score), 6)}
        for document_id, score in zip(ids, scores)
        if score > 0
    ]
    results.sort(key=lambda item: (-item["score"], item["documentId"]))
    return results[:limit]
