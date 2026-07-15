# Deploy Topology Reference

Use this reference when Loom selects `deploy.topology`. It explains public entry, proxy routes, validation paths, and gateway behavior.

## Authority

`DeploymentTopology` is generated from Loom deploy facts. Compose and Nginx must implement it; agents should not replace it with an invented topology during repair.

Fields:

- `publicEntryServiceId`: service that owns the preview URL and published host port.
- `routes`: public routing rules such as static SPA serving or HTTP proxy paths.
- `validation.previewPaths`: paths probed against the public entry URL.
- `validation.apiProbes`: safe `GET`/`HEAD` probes derived only from read-safe interfaces in the accepted API contract. Route contracts still include every declared method; write interfaces are never executed as deploy probes.

## Frontend Gateway + Backend API

When topology class is `frontend_gateway_backend_api`:

- Public entry service is frontend/static gateway.
- Backend service remains internal.
- Compose publishes only the frontend gateway host port unless another public port is explicitly present.
- Nginx or equivalent gateway must define API proxy locations before SPA fallback.
- `location /api/` and an exact `/api` route are both needed when the base path is `/api`.
- `proxy_pass` must target the backend Compose service name and container port.
- SPA fallback belongs after API proxy locations.

## Backend-Served Frontend + API

When topology class is `backend_served_frontend_api`:

- One service owns both preview and API validation.
- No HTTP proxy route is necessary.
- Frontend assets must be copied into the backend's static output before packaging or runtime start.
- API validation paths are probed directly against the backend public port.

## API-Only Single Service

When topology class is `api_only_single_service`:

- API paths are direct validation paths.
- Missing proxy routes are not an error.
- Preview URL may be the health/root path if the app exposes HTTP.

## Single Service App

When topology class is `single_service_app`:

- One app service owns the public preview URL.
- No HTTP proxy route is required.
- Validation probes the preview/health path directly against the app public port.

## Static Site

When topology class is `static_site`:

- Public entry serves static content.
- API paths must be empty.
- SPA fallback is added only when client-side routing is signaled by framework/source evidence.

## Validation Rules

Generated assets should fail preflight when:

- Topology references a service id not present in source model.
- Frontend gateway topology lacks an HTTP proxy route.
- Nginx proxy route appears after SPA fallback.
- API validation path returns HTML fallback.
- `DeploymentSpec.runtime.ports` has no public port for the public entry service.

Repair should fix generated gateway files when they contradict topology. If the topology itself contradicts source facts, the MCP generator is wrong and should be fixed rather than hidden by asset edits.

## API Contract Boundary

Deploy consumes the project-level current API contract referenced by the accepted Architecture artifact. It does not infer a public API prefix from a string such as `/api`, and it does not let a generated frontend environment variable redefine an interface path. Before generated assets are written, Loom checks that every declared interface path fits the public exposure base and derives safe read probes separately. An unresolved or conflicting binding blocks generated deployment assets with the source files and contract reference in the diagnostic.
