---
description: Route Loom deployment commands through the local Claude adapter.
argument-hint: "[prepare|run|up|status|inspect|validate|logs|bootstrap|down|repair]"
allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]
---

You are executing the user command `/loom-deploy $ARGUMENTS` now.

Your first assistant action must be exactly one Bash tool call. Do not answer in prose, ask a question, inspect files, read `.loom/`, create a plan, or summarize before that Bash tool call.

Use the current project directory as `--project-root`.

- If `$ARGUMENTS` is empty, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root "$PWD"`
- Otherwise, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy $ARGUMENTS --project-root "$PWD"`

If the first Bash command is a long-running `deploy run`, keep waiting on that same Bash session. After one short "deploy is running" update, stay quiet while polling the Bash session. Do not run extra `deploy status`, `deploy inspect`, or `deploy logs` during the first 120 seconds unless the original command returns, the user asks for status, or a blocker appears. After 120 seconds, use read-only observation no more often than once every 60 seconds; prefer `deploy status`, and use `deploy logs` only after repeated unchanged status or explicit user request. If any envelope returns `mode: observe_active_deploy_operation`, obey `instruction.observationPolicy`. Do not send a final done/stuck/failed deploy response while `operationActive=true`.

After the Bash command returns, parse the JSON envelope and follow any returned auto-runnable instruction immediately. Stop only for a real user gate, done report, blocked report, or non-repairable command failure.

After the first deploy command, before acting on a returned deploy repair, execution repair, generated asset, candidate, or retry instruction, read `~/.claude/skills/loom/skills/loom-deploy/SKILL.md` unless that exact deploy skill body is already loaded in this session. Then continue with that installed Loom deploy protocol: use returned request refs, repair boundaries, exact submit commands, generated deploy assets, and execution-repair instructions from that protocol. Do not invent Docker/Compose changes, alternate preview URLs, or manual deployment steps outside the returned Loom instruction.
