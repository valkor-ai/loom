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

## Scanner Signals To Deploy Facts

Translate Python scanner evidence into deploy facts before generating files:

- Dependency manifests become service root, package manager, lockfile, and install command facts.
- FastAPI/Flask/Django/Streamlit/stdlib HTTP signals become runtime framework and port facts.
- ASGI/WSGI symbols such as `app = FastAPI()`, `Flask(__name__)`, `application`, and Django settings become start command candidates.
- `manage.py` and Django settings become app root and framework env facts.
- Env examples and settings modules become required/generated environment facts.
- SQLAlchemy/Django database settings, Alembic, Redis/Celery, and driver dependencies become dependency service facts.
- Plain scripts without HTTP server signals become non-preview or command-style deploy facts instead of fake HTTP apps.

## Generated Asset Expectations

Generated Python assets should show:

- `python:3.12-slim` unless project metadata selects a more specific compatible Python version.
- Install step matching pip, uv, or Poetry facts.
- Runtime command using the detected ASGI/WSGI/stdlib entrypoint and binding to `0.0.0.0`.
- `PORT` set to the selected container port.
- Django local preview env includes safe local secret/allowed-host defaults only when Django is detected.
- Dependency URLs use Compose service DNS names and generated local credentials.
- Healthcheck candidates prefer explicit source/docs paths before generic `/`.

## Repair Boundary

Repair generated Python deploy assets when:

- Dockerfile installs from the wrong manifest or package manager.
- Entrypoint/module name is wrong but can be inferred from source evidence.
- Uvicorn/Gunicorn/Flask/Django command binds to localhost or wrong port.
- Generated env misses safe local framework defaults.
- Dependency service URL points at localhost.

Do not edit Python source, settings modules, migrations, or dependency manifests during deploy asset repair unless the MCP action routes to execution repair.
