# Docker Compose Deployment Reference

Use this reference when implementing or repairing generated Compose files or wrappers around existing Dockerfiles.

## Generation Rules

- Generate services from the DeploymentSpec source model, not from a hard-coded one-service assumption.
- Use the source model service ids for app services. For a single service, `app` is acceptable. For frontend/backend shapes, keep separate frontend and backend service ids and make the public entry service own the preview URL.
- Set each service `build.context` to the DeploymentSpec build context for that service/provider and `build.dockerfile` to the Dockerfile path relative to that context. Do not point a Dockerfile at a context that omits its package/build files.
- Publish only public runtime ports from `DeploymentSpec.runtime.ports` where `internalOnly=false`.
- Dependency services should use Compose internal networking and `expose`, not host `ports`, to avoid local conflicts.
- Use named volumes for stateful dependencies such as Postgres, MySQL, MongoDB, Redis, MinIO, RabbitMQ, and Elasticsearch.
- Generate environment variables only for local development defaults. Do not generate real secrets.
- Real local `.env` values must not be copied into generated Compose. Use environment diagnostics to record variable names only.
- Safe local placeholders are acceptable for common framework boot secrets such as Laravel `APP_KEY`, Rails `SECRET_KEY_BASE`, Django `SECRET_KEY`, or NextAuth `NEXTAUTH_SECRET`; they are not production credentials.
- Prefer map-style `environment` values so repairs are easy to read and patch.
- Use `depends_on` for dependency ordering. Add health conditions only for services that define a healthcheck and where the local Docker Compose version supports them.
- Use `restart: unless-stopped` for generated long-running services.

## Source Model Shapes

Single-service projects:

- One app service owns build/start/runtime config.
- Publish the resolved host port from `runtime.ports`.
- If the project has no HTTP server, omit preview URL expectations instead of faking one.

Frontend plus backend projects:

- Prefer a public frontend/static service and an internal backend API service.
- Route API paths through the public entry service only when the generated stack includes a proxy configuration that is proven before the SPA fallback.
- Backend ports stay internal unless the runtime contract explicitly marks them public.
- Frontend runtime env should point to the proxy path when a proxy exists, not to a host-only backend URL that will fail inside the browser after deployment.

Existing Dockerfile wrapper:

- Wrap the existing Dockerfile with Compose, but keep the Dockerfile protected.
- Match `build.context` to the directory assumptions of the Dockerfile. App-local Dockerfiles usually expect their own directory; generated workspace Dockerfiles may need the workspace root.
- Do not add services into a user Compose file just because the wrapper would be easier.

## Port Plan

Treat `DeploymentSpec.runtime.ports` as the source of truth for host/container port publication:

- `hostPort` is the real available local port chosen by Loom. Use it in Compose `ports` and final preview details.
- `preferredHostPort` is only the starting preference. Do not hard-code it when `hostPort` differs.
- `containerPort` must match the app process inside the container and the Dockerfile `EXPOSE`.
- `purpose` tells whether the port is preview, api, service, or dependency.
- `internalOnly=true` means no host publication. Use service DNS names and `expose` for internal communication.

When multiple public app ports exist, publish each explicit public runtime port once. Do not publish dependency ports to solve application connectivity; fix the internal service URL instead.

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
- Build context failures usually show `file not found`, missing lockfile, missing wrapper script, missing build file, or missing source directory. Compare Compose `build.context`, `build.dockerfile`, `DeploymentSpec.files`, and `sourceModel.services[].root` before changing Dockerfile commands.
