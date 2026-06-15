---
name: loom-deploy
description: Use when the user explicitly invokes @loom deploy or asks loom to prepare, run, inspect, log, stop, or repair a local deployment. This skill owns loom's deployment workflow, Dockerfile/Compose generation strategy, provider selection, validation, and future deploy repair guidance.
---

# loom deploy

This skill owns loom's explicit deploy mode. Keep the main `loom` skill focused on routing; keep deployment-specific workflow, provider choices, and repair guidance here.

This Codex plugin is the Codex adapter for loom's agent-neutral protocol. Run deploy workflow commands through `$HOME/.loom/bin/loom-cli` with `LOOM_AGENT_PROFILE=codex`; use `LOOM_COMPACT_OUTPUT=1` for normal JSON envelopes. Do not run bare `loom` or depend on shell `PATH`. If a deploy command returns `AGENT_PROFILE_REQUIRED`, rerun the exact same command with `LOOM_AGENT_PROFILE=codex` and preserve all original arguments.
When this skill mentions a command such as `loom deploy up`, treat it as a logical subcommand name. Execute it through `$HOME/.loom/bin/loom-cli` or the returned `commandInvocation`, never through a bare `loom` executable.
When a deploy repair or deploy-sourced execution repair returns a `requestRef`, read that request's `agentAction.read.fieldGroups` and run the listed inspect readCommands before editing files. Use `data.fields[<field>].value` for returned values. If inspect fails, use the group fields with requestManifest refs and targeted selectors; direct full artifact reads are a correctness fallback only and must not be printed into chat.

## Current Capability

Implemented now:

- Local Docker Compose deployment preview.
- `deploy run`: initialize when needed, prepare deployment assets, build/start with Docker Compose, validate health, report status, and include the latest repair request when the run cannot complete.
- `deploy run`: when an accepted RuntimeDeliveryContract exists, consume it as the source of truth for build/start/preview shape. A deployment is successful only when the declared preview URL/path is verified with HTTP 2xx/3xx; a running container alone is not success.
- `deploy prepare`: initialize `.loom/`, scan Node/Python/Go/Java/.NET/PHP/Ruby/static projects, reuse existing Compose/Dockerfile assets when present, otherwise generate Dockerfile, Compose, Dockerfile-specific ignore file, and deployment spec.
- Provider policy controls for prepare/run/up, including explicit `--provider`, `--force-generate`, and `--reuse-existing false`.
- Workspace/monorepo detection for common roots such as `pnpm-workspace.yaml`, `package.json workspaces`, `turbo.json`, `nx.json`, and `lerna.json`; root invocations select a deployable app subdirectory when the root itself is not directly deployable, and `--app-path` can explicitly select one app.
- Existing Compose analysis selects a likely app service from service names, build blocks, published ports, dependencies, and known infrastructure services so logs/status/health target the app rather than the first database/cache service.
- Environment diagnostics for `.env.example`/local `.env` variable names, source-code env references, generated runtime/dependency env, missing required env, and local-only placeholder secrets for common frameworks.
- Bootstrap diagnostics for common migration systems such as Prisma, Django, Rails, Laravel, Flyway, and Liquibase. These are advisory only; loom does not run migrations automatically.
- `deploy bootstrap`: show detected bootstrap commands by default; run them inside the active app service only when the user explicitly passes `--confirm`.
- Healthcheck candidate probing across common paths such as `/`, `/health`, `/healthz`, `/api/health`, framework health endpoints, and successful-path persistence in the deployment spec.
- User-configurable healthcheck options for prepare/run/up, including `--healthcheck-path`, `--healthcheck-candidate`, `--healthcheck-disabled`, retry count, interval, timeout, and expected status max.
- Dependency service scanning for Postgres, Redis, MySQL, MongoDB, RabbitMQ, Elasticsearch, and MinIO in generated Compose deployments.
- `deploy up`: validate Compose config, run `docker compose up -d --build`, inspect startup logs for fatal errors, run HTTP healthcheck probing when applicable, verify the preview path with HTTP 2xx/3xx, and write runtime state.
- `deploy status`: inspect the deployment container and report URL/container id/health.
- `deploy validate`: run Compose config validation and, when a deployment is running, refresh health status.
- `deploy inspect`: summarize the prepared spec, selected provider/policy, workspace, app service, runtime, missing env, bootstrap tasks, state, and latest repair request; `--refresh` updates running state and health first.
- `deploy logs`: return Docker Compose logs.
- `deploy down`: run `docker compose down` and update state.
- Failure classification for Compose config, build/start, and log failures.
- Failure diagnostics for common runtime/build blockers such as missing native optional Node packages, missing modules, port conflicts, localhost-only binding, dependency connection/auth failures, missing tables or pending migrations, missing env, and permissions.
- Repair request artifact written to `.loom/deployment/state/repair-request.json` when validation fails.
- `deploy repair`: expose the latest bounded repair request as structured JSON, including provider candidates, environment/bootstrap diagnostics, failure diagnostics, compact `errorWindow`, `fullLogRef`, and suggested repair actions for a coding agent.
- Skill-level repair execution: a coding agent may edit only returned `editableFiles`, then rerun `deploy up`.
- Provider strategy metadata in `.loom/deployment/specs/local.json`, including selected/skipped/available Dockerfile and Compose provider candidates.

Not implemented yet:

- CLI-native model execution for deployment repair.
- Remote or cloud deployment.

## Workflow

For a plain `@loom deploy` request, run the full local workflow:

```bash
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root /abs/project
```

## Active Deploy Operation

While `deploy run`, `deploy up`, `deploy prepare`, `deploy down`, or `deploy bootstrap --confirm` is running, do not start another mutating deploy command. Observation is limited to `deploy status`, `deploy inspect`, and `deploy logs`.

Do not run raw `docker compose`, `docker build`, `docker run`, or manual container inspection as a substitute for Loom deploy observation. Do not kill, `pkill`, or stop deploy, Docker Compose, Docker build, or Loom-managed deployment processes.

When the original `deploy run` tool call is still active, prefer waiting on that same tool session. Enter quiet observation mode: after one short "deploy is running" update, do not send progress chatter for repeated polls. For the first 120 seconds, do not run extra `deploy status`, `deploy inspect`, or `deploy logs` unless the original command returns, the user asks for status, or the CLI returns a blocker. After that initial quiet window, observe no more often than once every 60 seconds. Prefer `deploy status`; use `deploy logs` only after repeated unchanged status, explicit user request, or a returned repair/blocker instruction that requires logs.

If a returned instruction has `mode: observe_active_deploy_operation`, obey `instruction.observationPolicy`. `operationActive=true` is not a completion point: do not send a final done/stuck/failed deploy response while it remains true. User-visible updates during active observation are allowed only for terminal result, phase change, blocker/repair request, explicit user status request, or the long-running threshold in `observationPolicy`.

If the CLI returns `DEPLOY_OPERATION_ACTIVE`, report the active operation's `command`, `phase`, `elapsedMs`, and `logRef`. Then wait for user instruction or observe with `deploy status`, `deploy inspect`, or `deploy logs`; do not take over the running operation.

If `deploy run` returns `completed: true`, report the URL plus preview verification result and health/status. Do not run raw `docker compose`, `docker build`, `docker run`, or manually recreate loom containers after a successful deploy command. Use `loom deploy status`, `loom deploy inspect`, `loom deploy validate`, `loom deploy logs`, or `loom deploy down` for follow-up checks.

If `deploy run` returns `completed: false`, inspect `data.repair`:

- If `data.repair.repairRoute` is `execution_repair` or the envelope contains `instruction.command.argv = ["repair", "request", "--type", "execution", "--source", "deploy", ...]`, run that command immediately and follow the returned synthetic `execute_task` request. This means deploy classified the failure as application code/runtime delivery, not a deployment asset repair. Do not run `loom deploy repair` as a substitute and do not edit Dockerfile/Compose from deploy context.
- When the synthetic execution repair request includes `executionRules.sourceEditPreparationContract`, follow that contract before application code/script writes and before writing the repair resultFile.
- If top-level `instruction.mode` is `deploy_repair_assets`, repair only `instruction.editableFiles`, then run `instruction.retryCommand`; if it fails, run `loom deploy repair --project-root /abs/project` and immediately follow the returned instruction.
- If `nextAction` is `edit-and-rerun-up`, repair only `data.repair.editableFiles`, then run `loom deploy up --project-root /abs/project`.
- If the repair succeeds, run `loom deploy validate --project-root /abs/project` and `loom deploy status --project-root /abs/project`.
- If the repair fails again, call `loom deploy repair --project-root /abs/project` and continue the repair workflow until attempts are exhausted.
- If `nextAction` is `fix-docker`, ask the user to start Docker, fix permissions, or pull the blocked base image/registry dependency.
- If `nextAction` is `request-user-approval`, explain protected files and ask before editing them.

For explicit step-by-step requests, run the requested command only:

```bash
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy prepare --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy prepare --project-root /abs/project --app-path apps/web
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy prepare --project-root /abs/project --healthcheck-path /ready
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy prepare --project-root /abs/project --provider dockerfile-template
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy up --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy validate --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy status --project-root /abs/project
```

For status/log/down/repair requests, call only the matching command:

```bash
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy status --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy inspect --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy inspect --project-root /abs/project --refresh
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy validate --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy logs --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy bootstrap --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy bootstrap --project-root /abs/project --kind prisma --confirm
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy down --project-root /abs/project
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy repair --project-root /abs/project
```

Use the CLI JSON envelope as the source of truth. If Docker is unavailable, Docker Compose fails, or the CLI returns `DEPLOY_NOT_PREPARED` / `DEPLOY_NOT_RUNNING`, report that error code and summary directly.

Do not manually create Dockerfiles, rewrite Compose files, start alternate local servers, edit application code, change package scripts, run raw Docker/Compose commands, or invent preview URLs unless the returned deploy repair request explicitly allows that action. If validation fails and the user asks to repair, run `loom deploy repair --project-root /abs/project` and follow the returned bounded repair request.

If diagnostics point to migrations/bootstrap, run `loom deploy bootstrap --project-root /abs/project` first to show commands. Only run `--confirm` after the user explicitly approves execution against the active local Compose deployment.

loom v1 uses Dockerfile/Compose only. It does not switch to Railpack, Buildpacks, or other external builders; if deployment cannot be repaired with Dockerfile/Compose, explain the blocker clearly.

## Structured Deploy Blockers

Some deploy prepare/run failures are source-of-truth blockers, not repair loops.

- `DEPLOY_SOURCE_INSUFFICIENT`: the CLI could not derive a complete deploy source model from TechnicalBaseline, code evidence, and existing deployment assets. Read `error.details.evidenceRef`, `missingFacts`, `ambiguous`, and `nextAction`; report the missing facts or ask for the requested decision. Do not rerun blindly, invent dependency services, or generate Dockerfile/Compose assets from memory.
- `DEPLOY_CONFLICT`: TechnicalBaseline expectations conflict with code evidence or existing deploy assets. Read `error.details.evidenceRef` and `conflicts`; ask the user or follow the returned `nextAction`. Do not silently switch stacks, change dependency services, or overwrite generated assets to make the conflict disappear.

TechnicalBaseline is expectation context. The CLI's deploy code evidence, generated spec, and blocker envelope decide deployment behavior. Do not override a structured blocker with skill text, prior chat memory, or manual Docker commands.

## Knowledge Layout

Keep `SKILL.md` as the workflow router. Put deploy knowledge in focused reference files so new stacks can be added without mixing unrelated runtime assumptions.

When implementing or repairing:

- Load [references/providers.md](references/providers.md) for provider choice and reuse guardrails.
- Load [references/workspaces.md](references/workspaces.md) when a project root is a monorepo/workspace, explicit `--app-path` is involved, or generated Compose context paths look wrong.
- Load [references/environment.md](references/environment.md) when missing env, secrets, framework config, or Compose environment injection is involved.
- Load [references/bootstrap.md](references/bootstrap.md) when missing tables, pending migrations, Prisma/Django/Rails/Laravel/Flyway/Liquibase bootstrap tasks, or `deploy bootstrap` is involved.
- Load [references/dockerfile.md](references/dockerfile.md) when editing generated Dockerfiles or Docker ignore files.
- Load [references/compose.md](references/compose.md) when editing generated Compose files or Compose wrappers.
- Load exactly one stack reference when stack-specific behavior is needed: [references/node.md](references/node.md), [references/python.md](references/python.md), [references/go.md](references/go.md), [references/java.md](references/java.md), [references/dotnet.md](references/dotnet.md), [references/php.md](references/php.md), [references/ruby.md](references/ruby.md), or [references/static.md](references/static.md).
- Load [references/repair.md](references/repair.md) only when executing a returned repair request.
- Load [references/external-references.md](references/external-references.md) only when evaluating whether to absorb ideas from external Docker/agent skills.

Future stack support should usually be added as `references/<stack>.md` plus scanner/template changes, not by expanding this file.

## Repair Workflow

`loom deploy repair` does not edit files by itself. The skill is the repair executor. The command returns:

- `failureKind`
- failed command, compact `errorWindow`, `fullLogRef`, and output tails
- `providerCandidates`
- `suggestedActions`
- `editableFiles`
- `protectedFiles`
- a repair instruction
- `attempts` and `maxAttempts`
- `nextAction`

Rules:

- Read `errorWindow` before reading full logs. Use `fullLogRef` only when compact diagnostics are insufficient for correctness.
- If top-level `instruction.mode` is `deploy_repair_assets`, edit only `instruction.editableFiles`, then run `instruction.retryCommand`; if it fails, run `loom deploy repair --project-root /abs/project` and immediately follow the returned instruction.
- If `nextAction` is `edit-and-rerun`, edit only `editableFiles`, then run `loom deploy up --project-root /abs/project` again.
- If `nextAction` is `execution-repair`, do not edit deploy assets. Run `loom repair request --type execution --source deploy --failure-ref <failureRef>` and execute the returned synthetic request. Submit it with `loom repair submit --type execution --source deploy --repair-id <repairId> --result-file <resultFile>`, then follow the returned `deploy run` retry instruction.
- For any returned deploy repair/execution repair request, use `agentAction.read.fieldGroups` inspect readCommands as the default request-reading path; fall back to requestManifest refs only when inspect is missing or fails.
- For deploy-sourced execution repair, source and result writes are governed by `executionRules.sourceEditPreparationContract` when present; do not repeat malformed file-write/edit operations with missing path/content/edit arguments.
- If `editableFiles` is empty for a RuntimeDeliveryContract, build command, start command, or preview probe mismatch, do not edit application code from deploy repair. Report that the delivery/runtime contract must be repaired through the normal loom delivery repair or manual review path.
- If `editableFiles` is empty and `protectedFiles` is non-empty, ask the user before editing protected reused assets such as an existing `compose.yaml` or `Dockerfile`.
- If the failure is `docker_unavailable`, do not edit deployment files; ask the user to start Docker or fix permissions.
- Do not modify application code, package scripts, tests, or RuntimeDeliveryContract from deploy repair. Deploy repair owns only deployment assets returned in `editableFiles`.
- Stop after `maxAttempts` repair attempts. The default is 10 attempts; summarize the remaining failure and ask the user how aggressive the next repair should be.

Execution loop:

1. Run `loom deploy repair --project-root /abs/project`.
2. If `hasRepairRequest` is false, say there is no repair request and suggest running `loom deploy up`.
3. If `nextAction` is `request-user-approval`, explain the protected files and ask before editing them.
4. If `nextAction` is `edit-and-rerun`, inspect the returned output tails and edit only `editableFiles`.
5. Run `loom deploy up --project-root /abs/project`.
6. If `deploy up` succeeds, run `loom deploy status --project-root /abs/project`.
7. If `deploy up` fails again, repeat from step 1 until attempts are exhausted.

## Target Architecture

loom deploy should evolve as:

```text
source workspace
  -> scanner
  -> DeploySpec
  -> strategy resolver
  -> materializer / builder
  -> validator
  -> runtime state
  -> repair loop
```

Scanner detects language, framework, package manager, lockfile, ports, build/start commands, static output, and dependency services. Node, Python, Go, Java, .NET, PHP, Ruby, and static detection are implemented.

DeploySpec records selected app path, build context path, stack, environment diagnostics, runtime, ports, healthcheck config, commands, services, provider, generated files, validation status, and repair attempts.
DeploySpec also records the selected provider policy so reruns and inspect output can explain why existing assets were reused, skipped, or generation was forced.
For existing Compose, DeploySpec also records analyzed services and the selected app service. For boot/database setup, DeploySpec records advisory bootstrap tasks.

Strategy resolver chooses:

- Existing Dockerfile/Compose: validate and use without overwriting user assets. This is implemented for root-level `compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`, and `Dockerfile`.
- Simple project: use deterministic templates and language/provider references.
- Complex or unknown project: generate deterministic Dockerfile/Compose files so a coding agent can inspect, repair, or explain why local deployment cannot be completed.
- Failed build or boot: classify failure, write a bounded repair request, then expose it through `deploy repair`.
- Failed logs/health: attach diagnostic codes and evidence so the repair executor can distinguish Dockerfile/Compose issues from missing env, dependency auth, platform native package, or migration/bootstrap problems.

Validator should run the relevant checks:

- `docker compose config --quiet`
- `docker build` or `docker compose up -d --build`
- startup log parsing
- healthcheck probes
- preview URL/path HTTP verification with expected 2xx/3xx
- explicitly confirmed bootstrap commands through `docker compose exec -T <app-service> sh -lc <command>`

## References

Read provider or language references only when implementing or repairing that surface:

- [references/dockerfile.md](references/dockerfile.md): Dockerfile generation, caching, runtime, and ignore-file guidance.
- [references/compose.md](references/compose.md): Compose service, network, volume, env, and healthcheck guidance.
- [references/node.md](references/node.md): Node/Vite/static deployment detection and template notes.
- [references/python.md](references/python.md): Python/FastAPI/Flask/Django/Streamlit detection and template notes.
- [references/go.md](references/go.md): Go service detection and template notes.
- [references/java.md](references/java.md): Java/Maven/Gradle/Spring Boot deployment detection and template notes.
- [references/dotnet.md](references/dotnet.md): .NET/ASP.NET Core deployment detection and template notes.
- [references/php.md](references/php.md): PHP/Composer/Laravel deployment detection and template notes.
- [references/ruby.md](references/ruby.md): Ruby/Bundler/Rails deployment detection and template notes.
- [references/static.md](references/static.md): Static site deployment template notes.
- [references/providers.md](references/providers.md): Dockerfile/Compose provider order and guardrails.
- [references/workspaces.md](references/workspaces.md): Monorepo/workspace app selection, build context, and package-manager lockfile guidance.
- [references/environment.md](references/environment.md): Env scanning, missing env diagnostics, local placeholders, and secret handling.
- [references/bootstrap.md](references/bootstrap.md): Bootstrap and migration diagnostics plus explicit execution rules.
- [references/repair.md](references/repair.md): Failure kinds, editing boundaries, and retry rules for repair execution.
- [references/external-references.md](references/external-references.md): External Docker skill and generator sources reviewed for ideas, with adoption boundaries.
