from algorithms import rank_bm25


def test_rank_bm25_prefers_matching_document() -> None:
    matches = rank_bm25(
        "证券账户开户",
        [
            {"id": "securities", "text": "证券账户 开户 资格 校验"},
            {"id": "funding", "text": "资金账户 存款 取款"},
            {"id": "market", "text": "行情 发布 交易 管理"},
        ],
    )

    assert matches[0]["documentId"] == "securities"
    assert matches[0]["score"] > 0
