# Docker Compose Deployment Reference

Use this reference when implementing or repairing generated Compose files or wrappers around existing Dockerfiles.

## Generation Rules

- Use a single app service name unless the project explicitly has multiple app processes. Prefer `app`.
- Set `build.context` to the project root and `build.dockerfile` to the selected Dockerfile path.
- Publish only the application service port to the host.
- Dependency services should use Compose internal networking and `expose`, not host `ports`, to avoid local conflicts.
- Use named volumes for stateful dependencies such as Postgres, MySQL, MongoDB, Redis, MinIO, RabbitMQ, and Elasticsearch.
- Generate environment variables only for local development defaults. Do not generate real secrets.
- Real local `.env` values must not be copied into generated Compose. Use environment diagnostics to record variable names only.
- Safe local placeholders are acceptable for common framework boot secrets such as Laravel `APP_KEY`, Rails `SECRET_KEY_BASE`, Django `SECRET_KEY`, or NextAuth `NEXTAUTH_SECRET`; they are not production credentials.
- Prefer map-style `environment` values so repairs are easy to read and patch.
- Use `depends_on` for dependency ordering. Add health conditions only for services that define a healthcheck and where the local Docker Compose version supports them.
- Use `restart: unless-stopped` for generated long-running services.

## Existing Assets

- Root-level `compose.yaml`, `compose.yml`, `docker-compose.yaml`, and `docker-compose.yml` are protected.
- Root-level `Dockerfile` is protected; generated Compose may wrap it, but must not edit it without approval.
- If a user-owned Compose file exists, validate and report it. Do not merge generated services into it automatically.
- Analyze existing Compose services before reporting status, logs, or health. Prefer app-like services named `app`, `web`, `api`, `server`, `backend`, `frontend`, `www`, `site`, or `gateway`, especially when they have `build`, published HTTP ports, and `depends_on`.
- Avoid selecting dependency-like services such as Postgres, MySQL, Redis, MongoDB, RabbitMQ, Elasticsearch, MinIO, Kafka, localstack, or mail services as the primary app even if they publish a host port.
- When a selected service has a published port, use that host/container port pair for the preview URL and healthcheck. If no published port exists, keep diagnostics explicit rather than guessing a reachable URL.

## Health And Logs

- App health probing belongs to loom validation unless the generated Compose file has an obvious HTTP endpoint.
- Probe common health candidates such as `/`, `/health`, `/healthz`, `/api/health`, `/ready`, `/readiness`, and framework-specific endpoints such as Spring Boot `/actuator/health` or Laravel/Rails `/up`.
- Respect user healthcheck overrides from `--healthcheck-path`, `--healthcheck-candidate`, `--healthcheck-disabled`, `--healthcheck-attempts`, `--healthcheck-interval-ms`, `--healthcheck-timeout-ms`, and `--healthcheck-expected-status-max`.
- When a candidate succeeds, persist that path back into `DeploymentSpec.runtime.healthcheck`.
- Log parsing should target the selected app service for existing Compose and identify fatal startup failures before reporting a preview URL.
- If the app has no HTTP server, Compose can still build/start it, but the deploy result should not invent an HTTP preview URL.

## Repair Clues

- `docker compose config` failures usually involve invalid YAML, wrong env shape, missing files, unsupported health condition syntax, or wrong build paths.
- Startup failures often involve wrong container command, missing dependency env, a dependency service that needs more startup time, or port mismatch.
- Port publish failures usually mean the selected host port is already in use; repair generated Compose, not app source.
