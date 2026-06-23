from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from algorithms import analyze, extract_tfidf_keywords, rank_bm25


FORBIDDEN_KEYS = {"projectRoot", "project_root", "loomRoot", "loom_root", "directoryRoot", "directory_root"}


def handle(request: dict[str, Any]) -> dict[str, Any]:
    forbidden = sorted(FORBIDDEN_KEYS.intersection(request.keys()))
    if forbidden:
        raise ValueError(f"worker request contains forbidden workflow fields: {', '.join(forbidden)}")

    operation = request.get("operation")
    if operation == "tokenize":
        return {"ok": True, "tokens": analyze(str(request.get("text", "")))}
    if operation == "tfidf":
        return {
            "ok": True,
            "keywords": extract_tfidf_keywords(
                list(request.get("documents", [])),
                int(request.get("limit", 20)),
            ),
        }
    if operation == "bm25":
        return {
            "ok": True,
            "matches": rank_bm25(
                str(request.get("query", "")),
                list(request.get("documents", [])),
                int(request.get("limit", 10)),
            ),
        }
    if operation == "file_grant_echo":
        return _file_grant_echo(request)
    raise ValueError(f"unsupported operation: {operation}")


def _file_grant_echo(request: dict[str, Any]) -> dict[str, Any]:
    read_grant = dict(request.get("readGrant") or {})
    output_grant = dict(request.get("outputGrant") or {})
    read_path = Path(str(read_grant.get("path", ""))).expanduser()
    if not read_path.is_file():
        raise ValueError("readGrant.path must be a file")

    data = read_path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    expected_digest = read_grant.get("sha256")
    if expected_digest and expected_digest != digest:
        raise ValueError("readGrant.sha256 does not match file content")

    result = {
        "ok": True,
        "path": str(read_path),
        "sizeBytes": len(data),
        "sha256": digest,
        "preview": data[:80].decode("utf-8", errors="replace"),
    }

    output_path_value = output_grant.get("path")
    if output_path_value:
        output_path = Path(str(output_path_value)).expanduser()
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(result, ensure_ascii=False, sort_keys=True), encoding="utf-8")
        result["outputPath"] = str(output_path)

    return result


def main() -> int:
    try:
        line = sys.stdin.readline()
        request = json.loads(line)
        response = handle(request)
    except Exception as error:  # noqa: BLE001 - worker boundary returns structured errors.
        response = {"ok": False, "error": str(error)}
    sys.stdout.write(json.dumps(response, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
