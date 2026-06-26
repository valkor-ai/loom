from algorithms import analyze


def test_analyze_mixed_chinese_and_latin_text() -> None:
    tokens = analyze("证券账户 account_001 开户")
    assert "account_001" in tokens
    assert "证券" in tokens
    assert "开户" in tokens


def test_analyze_preserves_negated_chinese_predicates() -> None:
    tokens = analyze("证券冻结不等于持仓减少")

    assert "不等于" in tokens
    assert "冻结不等于" in tokens
    assert "证券冻结等于" not in tokens
    assert "冻结等于" not in tokens
    assert "等于持仓减少" not in tokens
