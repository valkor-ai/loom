from algorithms import analyze


def test_analyze_mixed_chinese_and_latin_text() -> None:
    tokens = analyze("证券账户 account_001 开户")
    assert "account_001" in tokens
    assert "证券" in tokens
    assert "开户" in tokens
