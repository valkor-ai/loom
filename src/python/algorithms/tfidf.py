from __future__ import annotations

from typing import Any

from sklearn.feature_extraction.text import TfidfVectorizer

from .analyzer import analyze


def extract_tfidf_keywords(documents: list[dict[str, Any]], limit: int = 20) -> list[dict[str, Any]]:
    texts = [str(document.get("text", "")) for document in documents]
    ids = [str(document.get("id", index)) for index, document in enumerate(documents)]
    if not texts or all(not text.strip() for text in texts):
        return []

    vectorizer = TfidfVectorizer(analyzer=analyze, lowercase=False)
    matrix = vectorizer.fit_transform(texts)
    terms = vectorizer.get_feature_names_out()

    scores = matrix.sum(axis=0).A1
    results: list[dict[str, Any]] = []
    for term_index, score in enumerate(scores):
        if score <= 0:
            continue
        column = matrix[:, term_index]
        document_ids = [ids[row] for row, value in zip(column.nonzero()[0], column.data) if value > 0]
        results.append(
            {
                "term": terms[term_index],
                "score": round(float(score), 6),
                "documentIds": sorted(set(document_ids)),
            }
        )

    results.sort(key=lambda item: (-item["score"], item["term"]))
    return results[:limit]
