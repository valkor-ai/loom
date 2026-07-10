# Deploy Stability Matrix

Use this reference when Loom selects `deploy.matrix`. It defines the deployment matrix that generated specs, source models, topology, Compose, Dockerfiles, validation, and repair must agree on.

## Matrix Dimensions

Every `loom.deployPrepare` result should be explainable by these dimensions:

- Topology class: `single_service_app`, `api_only_single_service`, `static_site`, `backend_served_frontend_api`, `frontend_gateway_backend_api`, `multi_service`, `existing_compose`, or `existing_dockerfile_wrapper`.
- Runtime families: Node, Java, Python, Go, .NET, PHP, Ruby, Static, or Unknown.
- Repository layout: root app, same-root fullstack, split frontend/backend, workspace app, or existing assets.
- Provider strategy: generated assets, existing Compose, or existing Dockerfile wrapper.
- Port plan: public preview/API ports, internal app ports, and dependency ports.
- State and dependency model: no dependency, file database volume, SQL/Redis/etc dependency service, or protected external service.

Do not collapse the matrix into a single "one container serves everything" assumption unless the facts say the same runtime process really serves every required surface.

## Topology Expectations

`api_only_single_service`:

- One application service.
- API probe paths may be validated directly against the public service.
- No HTTP proxy route is required.
- Frontend/static preview assumptions must not be invented.

`single_service_app`:

- One public application service exposes an HTTP preview/root path.
- No extra API proxy or static gateway is generated.
- Compose, Dockerfile, healthcheck, and port plan all point to the same service/container port.

`static_site`:

- One public static service.
- No API route validation unless the source model contains a backend/API service.
- Use Nginx or an equivalent static server; do not run a frontend development server as the deployed runtime.

`backend_served_frontend_api`:

- One backend/runtime service owns both preview traffic and API paths.
- Static assets are built/copied into the backend artifact before packaging or runtime start.
- No frontend gateway proxy is required.

`frontend_gateway_backend_api`:

- Public entry service is the frontend/static gateway.
- Backend API service is internal.
- API paths must be proxied by the public entry before SPA fallback.
- Browser-facing API env points to the public proxy path, not `localhost:<backend-port>`.

`multi_service`:

- More than one deployable service exists, but not necessarily a frontend/API pair.
- Public and internal ports must be explicit in `DeploymentSpec.runtime.ports`.
- Compose service ids must match `sourceModel.services[].serviceId`.

`existing_compose`:

- User Compose is protected.
- Loom may inspect, validate, and report selected services.
- Loom must not rewrite the user Compose file during prepare or repair.

`existing_dockerfile_wrapper`:

- User Dockerfile is protected.
- Loom may generate a Compose wrapper.
- The wrapper build context must match the Dockerfile's assumptions.

## Port Matrix

Use `DeploymentSpec.runtime.ports` as the only host/container port plan.

- `hostPort` is the real available local port chosen by Loom.
- `preferredHostPort` is diagnostic only after allocation.
- `containerPort` must match the runtime process and Dockerfile `EXPOSE`.
- `internalOnly=true` services are not published to the host.
- Dependency services use Compose DNS names and `expose`; do not publish dependency ports to make app code work.
- Multiple public ports are allowed when the source model requires them, but each must have one clear purpose.

## Dependency Matrix

Generated dependency services are local deployment conveniences, not production infrastructure.

- File databases need a writable mounted container path such as `/app/data`.
- SQL/Redis/Mongo/etc services use stable Compose service names.
- Application connection URLs use service DNS names, not `localhost`.
- Real credentials are blockers unless Loom can supply safe local placeholders.

## Repair Boundary

Repair may patch generated assets when the generated files are inconsistent with the facts. Repair must not compensate for an incorrect matrix by inventing a different topology.

- Facts/source/topology mismatch: fix MCP generation logic or generated assets aligned with facts.
- Generated Dockerfile/Compose mismatch: repair generated assets only.
- Application build/start/runtime failure: route to deploy execution repair.
- Protected existing assets mismatch: report protected-asset blocker or generated fallback according to provider policy.
