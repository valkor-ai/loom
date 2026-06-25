# Workspace / Monorepo Deploy Guidance

Use this reference when `loom deploy prepare/run --project-root` is pointed at a monorepo root rather than a single application directory.

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

## Explicit App Path

`--app-path <relative-path>` overrides automatic workspace selection. It must stay inside `--project-root` and point to an existing directory.

Use explicit app paths when a repo has multiple deployable targets, such as `apps/web`, `apps/admin`, and `services/api`. loom still stores one current local deployment under the root `.loom`; selecting a different app rewrites the current generated deployment spec/assets.

## Build Context

For reused app-local Dockerfiles and Compose files, keep the build context at the selected app path. User-authored Dockerfiles usually assume their own directory as context.

For generated Node workspace Dockerfiles, prefer the workspace root as build context so root lockfiles and workspace manifests remain available to npm/pnpm/yarn/bun. Set `detectedStack.workingDirectory` to the selected app path and make the Dockerfile switch to that directory before running app build/start scripts.

For generated non-Node stacks, keep context at the selected app path until stack-specific workspace support is implemented.

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
