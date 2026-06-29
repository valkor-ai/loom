use algorithm_client::AlgorithmClient;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

fn client() -> AlgorithmClient {
    let root = repo_root();
    AlgorithmClient::new(
        "python3",
        root.join("src")
            .join("python")
            .join("algorithms")
            .join("worker.py"),
    )
}

#[test]
fn worker_tokenizes_text() {
    let response = client()
        .call(&json!({"operation": "tokenize", "text": "证券账户 account"}))
        .expect("tokenize response");
    let tokens = response["tokens"].as_array().expect("tokens array");
    assert!(tokens.iter().any(|token| token == "account"));
}

#[test]
fn worker_runs_tfidf_and_bm25() {
    let documents = json!([
        {"id": "securities", "text": "证券账户 开户 资格 校验"},
        {"id": "funding", "text": "资金账户 存款 取款"},
        {"id": "market", "text": "行情 发布 交易 管理"}
    ]);

    let tfidf = client()
        .call(&json!({"operation": "tfidf", "documents": documents, "limit": 5}))
        .expect("tfidf response");
    assert!(tfidf["keywords"].as_array().expect("keywords").len() > 0);

    let bm25 = client()
        .call(&json!({
            "operation": "bm25",
            "query": "证券账户开户",
            "documents": [
                {"id": "securities", "text": "证券账户 开户 资格 校验"},
                {"id": "funding", "text": "资金账户 存款 取款"},
                {"id": "market", "text": "行情 发布 交易 管理"}
            ]
        }))
        .expect("bm25 response");
    assert_eq!(bm25["matches"][0]["documentId"], "securities");
}

#[test]
fn worker_reads_authorized_file_grant() {
    let temp_dir =
        std::env::temp_dir().join(format!("loom-algorithm-client-{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let source = temp_dir.join("source.txt");
    fs::write(&source, "证券账户开户").expect("source file");
    let output = temp_dir.join("echo.json");

    let response = client()
        .call(&json!({
            "operation": "file_grant_echo",
            "readGrant": {"path": source},
            "outputGrant": {"path": output}
        }))
        .expect("file grant response");

    assert_eq!(response["ok"], true);
    assert!(output.is_file());
}
