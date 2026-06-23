from algorithms import extract_tfidf_keywords


def test_extract_tfidf_keywords_returns_stable_terms() -> None:
    keywords = extract_tfidf_keywords(
        [
            {"id": "doc_a", "text": "证券账户 开户 开户 校验"},
            {"id": "doc_b", "text": "资金账户 存款 取款"},
        ],
        limit=5,
    )

    terms = [keyword["term"] for keyword in keywords]
    assert "开户" in terms
    assert keywords == sorted(keywords, key=lambda item: (-item["score"], item["term"]))
