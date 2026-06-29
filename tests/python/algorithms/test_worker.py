import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
WORKER = ROOT / "src" / "python" / "algorithms" / "worker.py"


def call_worker(request: dict) -> dict:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(ROOT / "src" / "python")
    completed = subprocess.run(
        [sys.executable, str(WORKER)],
        input=json.dumps(request, ensure_ascii=False) + "\n",
        text=True,
        capture_output=True,
        check=True,
        env=env,
    )
    return json.loads(completed.stdout)


def test_worker_tokenize() -> None:
    response = call_worker({"operation": "tokenize", "text": "证券账户 account"})
    assert response["ok"] is True
    assert "account" in response["tokens"]


def test_worker_rejects_workflow_fields() -> None:
    response = call_worker({"operation": "tokenize", "text": "x", "projectRoot": str(ROOT)})
    assert response["ok"] is False
    assert "forbidden workflow fields" in response["error"]


def test_worker_file_grant_echo(tmp_path: Path) -> None:
    source = tmp_path / "source.txt"
    source.write_text("证券账户开户", encoding="utf-8")
    output = tmp_path / "out" / "echo.json"
    digest = hashlib.sha256(source.read_bytes()).hexdigest()

    response = call_worker(
        {
            "operation": "file_grant_echo",
            "readGrant": {"path": str(source), "sha256": digest},
            "outputGrant": {"path": str(output)},
        }
    )

    assert response["ok"] is True
    assert response["sha256"] == digest
    assert output.is_file()
