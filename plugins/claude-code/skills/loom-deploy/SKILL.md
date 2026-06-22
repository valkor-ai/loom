---
name: loom-deploy
description: Use when the user explicitly invokes /loom deploy or asks loom to prepare, run, inspect, log, stop, validate, bootstrap, or repair a local deployment. This skill owns loom's deployment workflow and repair boundaries for Claude Code.
argument-hint: "deploy [prepare|run|up|status|inspect|validate|logs|bootstrap|down|repair]"
allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]
---

# loom deploy

This skill owns loom's explicit deploy mode for Claude Code. Keep the main `loom` skill focused on routing; keep deployment-specific workflow, provider choices, diagnostics, and repair boundaries here.

Every deploy workflow command must set:

```bash
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy ...
```

Use the launcher at `$HOME/.loom/bin/loom-cli`. Do not run bare `loom` or depend on shell `PATH`.
When this skill mentions a command such as `loom deploy up`, treat it as a logical subcommand name. Execute it through `$HOME/.loom/bin/loom-cli` or the returned `commandInvocation`, never through a bare `loom` executable.

If a deploy command returns `AGENT_PROFILE_REQUIRED`, rerun the exact same command immediately with `LOOM_AGENT_PROFILE=claude` and all original arguments.

Use Claude Code's native file tools normally for source files, project docs, and small text artifacts. For `.loom` deployment JSON artifacts, repair request files, state files, logs, and generated candidates, use root `requestReadPlan.groups` inspect readCommands and read values from `data.fields[<field>].value`. Only older requests without `requestReadPlan` may use returned `agentAction.read.fieldGroups` as compatibility fallback. If inspect is missing or fails, fall back to requestManifest refs and targeted selectors; direct full artifact reads are a correctness fallback only and must not be printed into chat. If a file-read tool call fails because of tool arguments, retry the same read with a valid native file-tool call or a short field selector. When an execution repair request contains `executionRules.sourceEditPreparationContract`, follow that contract before any `Write`, `Edit`, `MultiEdit`, or quiet programmatic file write; if a native file-write tool rejects missing or invalid arguments before writing, return to that contract's write-plan sequence and do not repeat the malformed tool call. Avoid heavyweight read-only shell scripts; prefer loom `inspect` readCommands, short, single-purpose shell selectors, or native file reads that do not print full artifacts into chat.

Do not use Claude Code Plan Mode, `ExitPlanMode`, or `.claude/plans/*` for any `/loom deploy` workflow. Deploy commands and returned repair requests are executable protocol steps, not Claude-internal plan approvals. If Claude Code itself is already forced into Plan Mode and blocks Bash/Edit/Write, do not write a plan file and do not call `ExitPlanMode`; report that Claude Code must leave Plan Mode before `/loom deploy` can execute.

Claude Code's internal task/todo tools and subagents may be used as implementation aids for source inspection, deployment repair, verification, and local reasoning. They must not replace Loom deploy state, decide whether deploy or repair is complete, route to the next Loom node, or justify stopping before the returned instruction is complete. Loom deploy state, the CLI JSON envelope, and returned `instruction` / `actionRequired` fields are the only task source of truth. If Claude shows stale internal task reminders, ignore them for deploy routing and continue from the latest loom deploy command result or returned instruction. `TaskStop` is allowed only to stop a task-owned background Bash/runtime after readiness probing or cleanup; do not use it as Loom progress or todo state.

## Current Capability

Implemented now:

- Local Docker Compose deployment preview.
- `deploy run`: initialize when needed, prepare deployment assets, build/start with Docker Compose, validate health, report status, and include the latest repair request when the run cannot complete.
- `deploy run`: when an accepted RuntimeDeliveryContract exists, consume it as the source of truth for build/start/preview shape. A deployment is successful only when the declared preview URL/path is verified with HTTP 2xx/3xx; a running container alone is not success.
- `deploy prepare`: scan common Node/Python/Go/Java/.NET/PHP/Ruby/static projects, reuse existing Compose/Dockerfile assets when present, otherwise generate bounded local deployment assets.
- Workspace/monorepo detection, app service selection, environment diagnostics, bootstrap diagnostics, healthcheck probing, Compose validation, Docker build/start, startup log parsing, preview verification, status/log/down/inspect/validate/bootstrap commands, and structured repair requests.
- `deploy repair`: expose bounded repair data, including provider candidates, environment/bootstrap diagnostics, failure diagnostics, compact `errorWindow`, `fullLogRef`, suggested repair actions, editable/protected files, attempts, and next action.

Not implemented:

- CLI-native model execution for deployment repair.
- Remote or cloud deployment.

## Workflow

For plain `/loom deploy`, run:

```bash
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root /abs/project
```

## Active Deploy Operation

While `deploy run`, `deploy up`, `deploy prepare`, `deploy down`, or `deploy bootstrap --confirm` is running, do not start another mutating deploy command. Observation is limited to `deploy status`, `deploy inspect`, and `deploy logs`.

Do not run raw `docker compose`, `docker build`, `docker run`, or manual container inspection as a substitute for Loom deploy observation. Do not kill, `pkill`, or stop deploy, Docker Compose, Docker build, or Loom-managed deployment processes.

When the original `deploy run` tool call is still active, prefer waiting on that same tool session. Enter quiet observation mode: after one short "deploy is running" update, do not send progress chatter for repeated polls. For the first 120 seconds, do not run extra `deploy status`, `deploy inspect`, or `deploy logs` unless the original command returns, the user asks for status, or the CLI returns a blocker. After that initial quiet window, observe no more often than once every 60 seconds. Prefer `deploy status`; use `deploy logs` only after repeated unchanged status, explicit user request, or a returned repair/blocker instruction that requires logs.

If a returned instruction has `mode: observe_active_deploy_operation`, obey `instruction.observationPolicy`. `operationActive=true` is not a completion point: do not send a final done/stuck/failed deploy response while it remains true. User-visible updates during active observation are allowed only for terminal result, phase change, blocker/repair request, explicit user status request, or the long-running threshold in `observationPolicy`.

If the CLI returns `DEPLOY_OPERATION_ACTIVE`, report the active operation's `command`, `phase`, `elapsedMs`, and `logRef`. Then wait for user instruction or observe with `deploy status`, `deploy inspect`, or `deploy logs`; do not take over the running operation.

For explicit subcommands, run only the requested command:

```bash
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy prepare --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy up --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy status --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy inspect --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy validate --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy logs --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy bootstrap --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy down --project-root /abs/project
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy repair --project-root /abs/project
```

If `deploy run` returns `completed: true`, report the URL plus preview verification result and health/status. Do not run raw `docker compose`, `docker build`, `docker run`, or manually recreate loom containers after a successful deploy command.

If `deploy run` returns `completed: false`, inspect `data.repair` and the returned instruction:

- If `data.repair.repairRoute` is `execution_repair` or the envelope contains `instruction.command.argv = ["repair", "request", "--type", "execution", "--source", "deploy", ...]`, run that command immediately and execute the synthetic `execute_task` request. This means deploy classified the failure as application code/runtime delivery, not deployment asset repair.
- When the synthetic execution repair request includes `executionRules.sourceEditPreparationContract`, follow that contract before application code/script writes and before writing the repair resultFile.
- If top-level `instruction.mode` is `deploy_repair_assets`, repair only `instruction.editableFiles`, then run `instruction.retryCommand`; if it fails, run `loom deploy repair --project-root /abs/project` and immediately follow the returned instruction.
- If `nextAction` is `edit-and-rerun-up`, repair only returned `editableFiles`, then run `loom deploy up --project-root /abs/project`.
- If `nextAction` is `fix-docker`, report it as an environment/session access blocker. Ask the user to start Docker Desktop/the Docker daemon, verify `docker version` works from the same terminal/session, enable full local access or Docker command permission if the agent chat is sandboxed, or fix registry/network access for blocked base images.
- If `nextAction` is `request-user-approval`, explain protected files and ask before editing them.

## Structured Deploy Blockers

Some deploy prepare/run failures are source-of-truth blockers, not repair loops.

- `DEPLOY_SOURCE_INSUFFICIENT`: the CLI could not derive a complete deploy source model from TechnicalBaseline, code evidence, and existing deployment assets. Read `error.details.evidenceRef`, `missingFacts`, `ambiguous`, and `nextAction`; report the missing facts or ask for the requested decision. Do not rerun blindly, invent dependency services, or generate Dockerfile/Compose assets from memory.
- `DEPLOY_CONFLICT`: TechnicalBaseline expectations conflict with code evidence or existing deploy assets. Read `error.details.evidenceRef` and `conflicts`; ask the user or follow the returned `nextAction`. Do not silently switch stacks, change dependency services, or overwrite generated assets to make the conflict disappear.

TechnicalBaseline is expectation context. The CLI's deploy code evidence, generated spec, and blocker envelope decide deployment behavior. Do not override a structured blocker with skill text, prior chat memory, or manual Docker commands.

## Repair Boundaries

Use the CLI JSON envelope as the source of truth. Do not manually create Dockerfiles, rewrite Compose files, start alternate local servers, edit application code, change package scripts, run raw Docker/Compose commands, or invent preview URLs unless the returned deploy repair/execution-repair request explicitly allows that action.

`loom deploy repair` does not edit files by itself. Claude Code is the repair executor only inside the returned bounds.

Rules:

- Read `errorWindow` before reading full logs. Use `fullLogRef` only when compact diagnostics are insufficient.
- If top-level `instruction.mode` is `deploy_repair_assets`, edit only `instruction.editableFiles`, then run `instruction.retryCommand`; if it fails, run `loom deploy repair --project-root /abs/project` and immediately follow the returned instruction.
- If `nextAction` is `edit-and-rerun`, edit only `editableFiles`, then run `loom deploy up --project-root /abs/project`.
- If `nextAction` is `execution-repair`, do not edit deploy assets. Run the returned `loom repair request --type execution --source deploy ...`, execute the synthetic request, submit it, then follow the returned deploy retry instruction.
- When the returned deploy repair or execution repair includes `requestRef`, read that request's root `requestReadPlan.groups` and run the listed inspect readCommands before editing files. Use `data.fields[<field>].value` for returned values. If the request is older and has no `requestReadPlan`, use `agentAction.read.fieldGroups` as compatibility fallback. If inspect fails, use the group fields through requestManifest refs and targeted selectors as the correctness fallback.
- For deploy-sourced execution repair, source and result writes are governed by `executionRules.sourceEditPreparationContract` when present; do not repeat malformed file-write/edit operations with missing path/content/edit arguments.
- If `editableFiles` is empty for a RuntimeDeliveryContract, build command, start command, or preview probe mismatch, do not edit application code from deploy repair. Route through the normal loom delivery repair path.
- If `editableFiles` is empty and `protectedFiles` is non-empty, ask the user before editing protected reused assets such as an existing `compose.yaml` or `Dockerfile`.
- If the failure is `docker_unavailable`, do not edit deployment files. Ask the user to start Docker Desktop/the Docker daemon, verify `docker version` works from the same terminal/session, and enable full local access or Docker command permission if Docker works outside the agent chat but not inside it.
- Stop after `maxAttempts` repair attempts and summarize the remaining failure.

## Knowledge Layout

Keep `SKILL.md` as the workflow router. Deploy provider, stack, environment, bootstrap, and repair knowledge lives in this skill's `references/` directory and is part of the Claude deploy adapter package. Load only the focused reference needed for the returned deploy failure, selected stack, or repair action.

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
