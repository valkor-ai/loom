from __future__ import annotations

from typing import Any

from sklearn.feature_extraction.text import TfidfVectorizer

from .analyzer import analyze


def extract_tfidf_keywords(documents: list[dict[str, Any]], limit: int = 20) -> dict[str, Any]:
    texts = [str(document.get("text", "")) for document in documents]
    ids = [str(document.get("id", index)) for index, document in enumerate(documents)]
    if not texts or all(not text.strip() for text in texts):
        return {"keywords": [], "documentKeywords": []}

    vectorizer = TfidfVectorizer(analyzer=analyze, lowercase=False)
    matrix = vectorizer.fit_transform(texts)
    terms = vectorizer.get_feature_names_out()

    scores = matrix.sum(axis=0).A1
    results: list[dict[str, Any]] = []
    for term_index, score in enumerate(scores):
        if score <= 0:
            continue
        weighted_score = float(score) * _term_weight(str(terms[term_index]))
        column = matrix[:, term_index]
        document_ids = [ids[row] for row, value in zip(column.nonzero()[0], column.data) if value > 0]
        results.append(
            {
                "term": terms[term_index],
                "score": round(weighted_score, 6),
                "documentIds": sorted(set(document_ids)),
            }
        )

    results.sort(key=lambda item: (-item["score"], item["term"]))
    document_keywords: list[dict[str, Any]] = []
    for row_index, document_id in enumerate(ids):
        row = matrix.getrow(row_index)
        keywords = []
        for term_index, raw_score in zip(row.indices, row.data):
            score = float(raw_score) * _term_weight(str(terms[term_index]))
            if score <= 0:
                continue
            keywords.append({"term": terms[term_index], "score": round(score, 6)})
        keywords.sort(key=lambda item: (-item["score"], item["term"]))
        document_keywords.append({"documentId": document_id, "keywords": keywords[:limit]})

    return {"keywords": results[:limit], "documentKeywords": document_keywords}


def _term_weight(term: str) -> float:
    cjk_count = sum(1 for char in term if "\u3400" <= char <= "\u9fff")
    if cjk_count >= 4:
        return 2.25
    if cjk_count == 3:
        return 1.6
    return 1.0
