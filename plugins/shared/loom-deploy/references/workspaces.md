# Workspace / Monorepo Deploy Guidance

Use this reference when `loom.deployPrepare` or `loom.deployRun` receives a `projectRoot` that points at a monorepo root rather than a single application directory.

## Detection

Treat these as workspace root markers:

- `pnpm-workspace.yaml`
- `package.json` with `workspaces` or `workspaces.packages`
- `turbo.json`
- `nx.json`
- `lerna.json`
- `rush.json`

If the root already has a Compose file, Dockerfile, or directly deployable stack, use the root. Otherwise, search likely app directories such as `apps/*`, `packages/*`, `services/*`, `sites/*`, `web`, `frontend`, `backend`, and `api`.

Rank candidates by explicit deployment assets first, then runnable framework/start command signals, then common app directory names. Keep the selected path and candidate scores in `DeploymentSpec.workspace` so an agent can explain or repair the choice.

## App Path And Build Context Matrix

Use this matrix before generating Compose or Dockerfiles:

- Root app: app path `.`, source root `.`, build context `.`, generated Dockerfile paths root-relative.
- App-local subdirectory: app path such as `service`, `backend`, `web`, or `frontend`; source root is that directory; build context is the app path unless ancestor lockfiles/workspace manifests are needed.
- Split frontend/backend: app paths are separate source roots; generated Compose has separate services unless topology proves backend-served frontend.
- Same-root fullstack: one app path contains backend and frontend build inputs; build context stays at that root and Dockerfile stages separate frontend build from backend runtime.
- Workspace package: app path is under `apps/*`, `packages/*`, or `services/*`; build context is workspace root when root lockfiles/workspace manifests are required; Dockerfile `WORKDIR` changes to the package before build/start.
- Existing Dockerfile: build context follows the Dockerfile's own assumptions; wrapper Compose must not choose a context that makes existing `COPY` paths invalid.
- Existing Compose: Compose file owns service context choices; Loom reports them instead of replacing them during prepare.

The selected app path is not always the build context. The build context is the smallest directory that contains all files the generated Dockerfile must copy.

## Explicit App Path

`DeployToolInput.appPath` overrides automatic workspace selection. It must stay inside `projectRoot` and point to an existing directory.

Use explicit app paths when a repo has multiple deployable targets, such as `apps/web`, `apps/admin`, and `services/api`. loom still stores one current local deployment under the root `.loom`; selecting a different app rewrites the current generated deployment spec/assets.

## Build Context

For reused app-local Dockerfiles and Compose files, keep the build context at the selected app path. User-authored Dockerfiles usually assume their own directory as context.

For generated Node workspace Dockerfiles, prefer the workspace root as build context so root lockfiles and workspace manifests remain available to npm/pnpm/yarn/bun. Set `detectedStack.workingDirectory` to the selected app path and make the Dockerfile switch to that directory before running app build/start scripts.

For generated non-Node stacks, choose context from the source model:

- app-local service with all build files under one root -> app root context
- frontend/backend composition where one image copies frontend static assets into a backend -> repository or common ancestor context
- multi-service generated Compose -> each service may use a different Dockerfile and workdir, but every Dockerfile path must be valid from its Compose build context

## Package Managers

Package-manager detection can use lockfiles in ancestor directories when scanning a selected Node app. This is important for pnpm/npm/yarn/bun monorepos where the app does not carry its own lockfile.

For pnpm workspaces, copy `pnpm-workspace.yaml` with the root lockfile before install. Without it, `pnpm install --frozen-lockfile` may fail or install an incomplete workspace graph.

## Repair Notes

When a monorepo deployment fails, inspect these fields first:

- `workspace.appPath`
- `workspace.buildContextPath`
- `files.buildContextPath`
- `files.dockerfilePath`
- `detectedStack.workingDirectory`

Common fixes are correcting the Compose `build.context`, Dockerfile path relative to that context, or the Dockerfile `WORKDIR` used before install/build/start commands.

If a build command works locally only because it is run from a subdirectory, encode that subdirectory as `WORKDIR` or an explicit `cd` in the generated Dockerfile. Do not flatten the workspace into one root command unless the project already has root-level build scripts for that app.

## Source Root Repair Boundary

When a workspace deploy fails:

- Missing manifest from Docker build means build context or `COPY` path is wrong.
- Missing wrapper/build script means `WORKDIR` is wrong or the service root was misidentified.
- Missing sibling package/module means the context was too narrow for a workspace dependency graph.
- Wrong public service means topology/source model selection is wrong, not a Compose retry detail.

Repair generated assets to match the selected source model. If the source model selected the wrong app path, report that fact instead of compensating with broad repository copies.
