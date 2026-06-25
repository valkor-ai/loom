# Python Deployment Reference

Use this reference when implementing or repairing loom deploy support for Python projects.

## Scanner Signals

- Python project files: `requirements.txt`, `pyproject.toml`, `Pipfile`, `uv.lock`, `poetry.lock`, `server.py`, `main.py`, `app.py`, `manage.py`.
- Also inspect package source files and local docs such as `README.md` and `HTTP_API.md` for entrypoints, routes, `--port` examples, and health endpoints.
- Package manager:
  - `uv.lock` or `[tool.uv]` -> uv
  - `poetry.lock` or `[tool.poetry]` -> poetry
  - otherwise -> pip
- Framework hints:
  - `fastapi` or `uvicorn` -> FastAPI, default port 8000.
  - `flask` or `gunicorn` -> Flask, default port 8000.
  - `django` or `manage.py` -> Django, default port 8000.
  - `streamlit` -> Streamlit, default port 8501.
  - `ThreadingHTTPServer`, `BaseHTTPRequestHandler`, `http.server`, or a local `run_http_server` helper -> standard-library HTTP server, default port 8000.
- If a standard-library HTTP server is detected, prefer `server.py`, then `main.py`, then `app.py` as the entrypoint when that file contains HTTP server signals.
- For stdlib HTTP entrypoints with `--host`/`--port`, generate a container-safe start command such as `python server.py --host 0.0.0.0 --port 8000`.
- Detect health paths from source or docs. Prefer explicit routes such as `/health`, `/healthz`, `/ready`, `/readiness`, `/api/health`, or `/up` over the generic `/`.

## Dependency Parser Boundary

- External Python packaging libraries can be useful later for dependency and version parsing, but they should not be required for the first deploy scanner path.
- Entrypoint, host binding, port, and health route inference should remain deterministic and explainable from local files, because this is deployment behavior rather than package resolution.

## Template Rules

- Use `python:3.12-slim`.
- Set `PYTHONDONTWRITEBYTECODE=1` and `PYTHONUNBUFFERED=1`.
- Set `PORT` to the detected container port.
- For pip, install `requirements.txt` when present.
- For poetry, install Poetry and run `poetry install --only main` with virtualenv creation disabled.
- For uv, install uv and prefer `uv pip install --system`.

## Repair Notes

- Most Python startup failures are wrong module names (`main:app` vs `app:app`), missing runtime dependencies, or binding to `127.0.0.1`.
- Keep fixes in generated Dockerfile/Compose unless the user approves source changes.
