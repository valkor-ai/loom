# Deployment Repair Reference

Use this reference when the current Loom MCP deploy action asks for deployment repair.

## Failure Kinds

- `compose_config`: fix Compose syntax, build context paths, service names, port mappings, env shape, or file references.
- `image_build`: fix generated Dockerfile install/build commands, package manager handling, ignored files, or build context.
- `container_start`: fix runtime command, exposed port, host binding, missing build artifacts, or production server command.
- `healthcheck`: fix HTTP healthcheck path/candidates, app listen address, startup timing, exposed port, or app command.
- Missing environment diagnostics live in `environment.missing`; check them before assuming a Dockerfile, port, or healthcheck problem.
- Bootstrap diagnostics live in `bootstrap.tasks`; treat Prisma, Django, Rails, Laravel, Flyway, and Liquibase migration commands as advisory unless the user explicitly approves running them.
- If the user approves bootstrap execution, use `loom deploy bootstrap --project-root /abs/project --kind <kind> --confirm` instead of hand-running migration commands.
- Failure diagnostics live in `diagnostics`; use diagnostic `code`, `evidence`, and `suggestedAction` to prioritize the repair. These diagnostics may show missing native packages, missing modules, port conflicts, dependency connection/auth failures, pending migrations, missing env, or permissions.
- `logs`: verify the Compose project/service still exists before editing files.
- `docker_unavailable`: do not edit files. Ask the user to start Docker Desktop/the Docker daemon, verify `docker version` works from the same terminal/session, and enable full local access or Docker command permission if Docker works outside the agent chat but not inside it.
- `registry_network`: do not edit files; Docker could not reach or authenticate with the image registry. Ask the user to retry, pre-pull the blocked image, configure Docker registry mirrors/proxy, or fix registry credentials/network access.
- `build_command_failed`, `start_command_failed`, `http_probe_failed`, `preview_not_verified`: if the current MCP action reports `repairRoute=execution_repair`, do not edit deploy assets. Execute the returned repair action, write its result, submit with the returned submit tool, then retry through the returned deploy action.
- `unknown`: classify from stdout/stderr before editing.

## Repair Decision Tree

Follow this order for every deploy repair:

1. Check whether the failure is outside generated assets.
   - Docker daemon unavailable, registry/network/auth failure, missing real credentials, and protected user asset mismatches are blockers or user-action items.
   - Do not edit Dockerfile/Compose for these failures.
2. Check facts/source/topology consistency.
   - Compare `factsRef`, `sourceModelRef`, `topologyRef`, and `DeploymentSpec.runtime.ports`.
   - If topology says `frontend_gateway_backend_api`, generated assets must include a public gateway and API proxy route.
   - If topology says `backend_served_frontend_api`, generated assets must package frontend output into the backend and must not invent a proxy requirement.
   - If the spec facts contradict each other, the MCP generator is wrong. Do not hide it by inventing a different topology in repair.
3. Check generated asset closure.
   - Compose service ids must match `sourceModel.services[].serviceId`.
   - Compose build context, Dockerfile path, Dockerfile `WORKDIR`, and `COPY` sources must be a valid path closure.
   - Runtime ports, `EXPOSE`, host bindings, and healthcheck paths must match the port plan.
   - Environment values must come from environment/dependency facts.
4. Classify the failing phase.
   - Compose config failure: patch Compose structure, paths, env shape, or unsupported syntax.
   - Image build failure: patch generated Dockerfile, ignore file, dependency install, or build context.
   - Container start failure: patch generated command, env, dependency URL, bind host, or runtime artifact selection.
   - Healthcheck/proxy failure: patch healthcheck candidates, public gateway config, route order, bind host, startup timing, or topology-consistent port wiring.
5. Escalate to execution repair when logs prove application code/build scripts are the failing surface and the MCP action routes to `execution_repair`.
   - In that route, do not edit deploy assets.
   - Execute the returned synthetic task, submit the result, then retry the returned deploy step.

This tree prevents blind Dockerfile edits. A Dockerfile repair is correct only after the source model and Compose build context have been checked.

## Platform-Specific Native Dependency Failures

- If logs mention `@next/swc-linux-*`, `@tailwindcss/oxide-linux-*`, `tailwindcss-oxide.linux-*.node`, `lightningcss.linux-*.node`, `sharp`, `esbuild`, `rollup-*`, or similar native optional packages, treat OS/libc/CPU as part of the repair.
- Prefer glibc images such as `node:22-slim` or the project-detected `node:<major>-slim` for Next.js/Tailwind apps unless the project is known to work on Alpine.
- If a package lockfile was generated on macOS and only includes `darwin-*` optional packages, prefer adding the needed Linux optional dependency to the project lockfile with user approval. When the repair scope is limited to generated assets, patch the generated Dockerfile install step so the Linux package is installed inside the image.
- Do not solve native module failures by bind-mounting host `node_modules` into a Linux container unless the user explicitly wants a dev-only deployment. Host `node_modules` is usually platform-specific.

## Java Build Failures

- If Maven or Gradle wrapper scripts are missing or not executable, use the builder image's installed `mvn` or `gradle` command before editing application code.
- If no runnable jar is found, inspect `target` or `build/libs` and avoid selecting `*-plain.jar`, `*-sources.jar`, or `*-javadoc.jar`.
- If Spring Boot starts on the wrong port, verify generated `PORT`, `SERVER_PORT`, and any project `server.port` setting.

## .NET Build Failures

- If `dotnet publish` succeeds but runtime cannot find the DLL, inspect the `.csproj` assembly name and published output, then update only the generated start command.
- If restore fails for private NuGet feeds, ask for credentials or a safe `NuGet.Config`; do not bake secrets into generated deployment files.
- If ASP.NET Core starts but healthcheck fails, verify `ASPNETCORE_URLS`, HTTPS redirection, whether the app is listening on the generated container port, and whether a framework-specific health path should be added.

## PHP Build Failures

- If Composer install fails because an extension is missing, update the generated Dockerfile extension install block before editing app code.
- If Laravel returns a 500 after boot, inspect logs and `environment.missing` for missing `APP_KEY`, storage/cache permissions, database connection errors, or pending migrations.
- If a Laravel project is detected as Node because of frontend assets, treat `composer.json` and `artisan` as higher-priority stack signals than `package.json`.

## Ruby Build Failures

- If Bundler fails on native extensions, update generated OS package installs before editing app code.
- If Rails returns a 500 after boot, inspect logs and `environment.missing` for missing `SECRET_KEY_BASE`, storage permissions, database connection errors, or pending migrations.
- If a Rails project is detected as Node because of frontend assets, treat `Gemfile` and Rails config as higher-priority stack signals than `package.json`.

## Editing Rules

- Edit only files listed in `editableFiles`.
- If `editableFiles` is empty because the failure is routed to deploy-sourced execution repair, the allowed edit boundary comes from the synthetic execution request, not from deploy repair.
- Treat `protectedFiles` as read-only unless the user explicitly approves editing them.
- Do not edit app source, package scripts, or environment files unless the user approves and the current repair action cannot be solved in deployment files.
- Do not run migration/bootstrap commands automatically. If diagnostics point to missing tables or pending migrations, explain the command from `bootstrap.tasks` and ask for approval.
- Do not read, print, or bake real local `.env` values into generated deployment files. Use variable names and safe local placeholders only.
- Preserve generated file locations under `.loom/deployment/specs/generated/`.
- Do not use repair to rewrite the provider strategy. If the selected provider is generated, repair generated assets. If an unforced existing provider was unsuitable, Loom should have fallen back to generated before repair. If the user forced an existing provider, report the protected asset issue clearly.

## Generation-First Repair Posture

Repair is a bounded fallback, not the primary template engine. Before patching, compare the failure with:

- `sourceModelRef` for service roots, runtime kinds, working directories, and package managers
- `topologyRef` for public entry service, API proxy paths, and validation paths
- `generatedFileRefs` for the actual files the agent may edit
- `DeploymentSpec.runtime.ports` for real host/container port assignments

If the generated template was structurally wrong, repair the generated asset in the smallest durable way and keep the fix aligned with the source model. Avoid scenario-specific fixes such as hard-coding one framework's port, one folder name, or one database unless the source model or diagnostics proves that exact stack.

Ask the user only when the next action requires real credentials, destructive bootstrap/migration execution, changing protected user-owned Compose/Dockerfile assets, or decisions that cannot be inferred from repository evidence.

## Protected Asset Boundary

Existing Compose and Dockerfile assets are user-owned unless the user forced a generated provider or explicitly approved editing them.

- For `compose-existing`, inspect and report the user Compose topology. Do not inject generated proxy, dependency, or healthcheck services into it.
- For `dockerfile-existing`, generate or repair the Compose wrapper and environment around the protected Dockerfile. Do not change the Dockerfile when its assumptions are merely inconvenient.
- For generated providers, generated files under `.loom/deployment/specs/generated/` are editable and should be repaired when they contradict facts.
- If a protected asset cannot satisfy the accepted runtime contract, report the protected-asset blocker and the exact required change. Loom may fall back to generated assets only when provider policy allows it.

## Retry Rules

- For a fresh deploy request, use the MCP deploy run action; it prepares missing specs, builds, starts, validates, reports status, and returns the next repair action when the full flow cannot complete.
- After each repair edit, call the returned `retryTool`. For asset repair this is `loom.deployUp`.
- `loom.deployUp` retries with the current `.loom/deployment/specs/local.json` and generated assets. It must not regenerate Dockerfile, Compose, nginx, or dockerignore files.
- Do not call `loom.deployRun` as a repair retry. `deployRun` is a high-level entry point, while repair retry is an execution step against the current spec.
- If it succeeds, run `loom deploy status --project-root /abs/project`.
- If it fails, run `loom deploy repair --project-root /abs/project` again and use the new request.
- Default `maxAttempts` is 10.
- Default Docker Compose build/start timeout is 10 minutes because first-time dependency installation can be slow on real projects.
- Stop when `attempts >= maxAttempts` or when the next repair requires protected files.
