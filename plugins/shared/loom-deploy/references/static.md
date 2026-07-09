# Static Deployment Reference

Use this reference when implementing or repairing static-site deployment support.

## Scanner Signals

- Static output directories: `dist`, `build`, `public`, `out`, `_site`.
- Plain static entry files: `index.html`, `404.html`, static assets without a server entrypoint.
- Node projects with Vite/Astro/Next export may still use the Node reference first when `package.json` has build/preview scripts.

## Template Rules

- For already-built static assets, use an Nginx runtime image and copy the detected output directory into the default web root.
- If a build step is detected, use the stack-specific build stage first, then copy the output directory into Nginx.
- Default container port is `80`.
- Add SPA fallback only when the framework or project signals client-side routing. Do not force fallback for plain static docs.

## Repair Notes

- Common failures are missing build output, wrong output directory, generated Nginx config not copied, or a project that needs a build command before static serving.
- If the project is a library/docs source without generated assets, report the missing build artifact instead of serving the source tree blindly.

## Scanner Signals To Deploy Facts

Translate static scanner evidence into deploy facts before generating files:

- Root `index.html` or detected output directory becomes static source/output fact.
- Framework/export signals decide whether a build step is required before static serving.
- Client-side routing signals decide whether SPA fallback is allowed.
- API/backend signals in the same repository should create a frontend gateway/backend API topology, not a static-only topology.
- Plain docs/library sources without built output become missing-build-artifact diagnostics rather than deployable static facts.

## Generated Asset Expectations

Generated static assets should show:

- Nginx or equivalent static runtime serving the detected output directory.
- Build stage only when scanner facts prove a build command/output directory.
- SPA fallback only when client-side routing is signaled.
- API proxy config only when topology contains an internal backend/API service.
- Container port `80`, with host publication controlled by `DeploymentSpec.runtime.ports`.

## Repair Boundary

Repair generated static deploy assets when:

- The copied output directory does not match scanner facts.
- Build stage output path is wrong.
- Nginx config is missing, misplaced, or has API proxy routes after SPA fallback.
- Static-only topology incorrectly includes API validation paths.

Do not create fake static output from source files just to make the container start. If no build output or build command exists, report the missing deployable artifact.
