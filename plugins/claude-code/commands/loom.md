---
description: Route Loom delivery commands through the local Claude adapter.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]
---

You are executing the user command `/loom $ARGUMENTS` now.

Your first assistant action must be exactly one Bash tool call. Do not answer in prose, ask a question, inspect files, read `.loom/`, create a plan, or summarize before that Bash tool call.

Use the current project directory as `--project-root`.

Route `$ARGUMENTS` as follows:

- If `$ARGUMENTS` is `status`, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" status --project-root "$PWD"`
- If `$ARGUMENTS` is `continue`, `resume`, `proceed`, `next`, or empty, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" continue --project-root "$PWD"`
- If `$ARGUMENTS` is `knowledge`, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge --project-root "$PWD"`
- If `$ARGUMENTS` starts with `knowledge `, remove the leading `knowledge ` token and pass the remaining text as knowledge subcommand arguments:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <the knowledge subcommand arguments from $ARGUMENTS> --project-root "$PWD"`
- If `$ARGUMENTS` is `deploy`, run:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root "$PWD"`
- If `$ARGUMENTS` starts with `deploy `, remove the leading `deploy ` token and pass the remaining text as the deploy subcommand arguments:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy <the deploy subcommand arguments from $ARGUMENTS> --project-root "$PWD"`
- If `$ARGUMENTS` is a new delivery request and includes one or more local requirement files such as PDF, DOCX, XLSX, TXT, MD, CSV, or TSV paths, run the `plan` entrypoint with one `--requirement-file "<path>"` argument per requirement file, plus `--request "<remaining natural-language request>"` only when there is remaining non-file text. Do not pass requirement file paths as plain request text.
- Otherwise, treat `$ARGUMENTS` as a new Loom delivery request and run the `plan` entrypoint, which initializes an empty project before creating the Brainstorm request:
  `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" plan --project-root "$PWD" --request "$ARGUMENTS"`

If the first Bash command is a long-running deploy command, keep waiting on that same Bash session. After one short "deploy is running" update, stay quiet while polling the Bash session. Do not run extra `deploy status`, `deploy inspect`, or `deploy logs` during the first 120 seconds unless the original command returns, the user asks for status, or a blocker appears. After 120 seconds, use read-only observation no more often than once every 60 seconds; prefer `deploy status`, and use `deploy logs` only after repeated unchanged status or explicit user request. If any envelope returns `mode: observe_active_deploy_operation`, obey `instruction.observationPolicy`. Do not send a final done/stuck/failed deploy response while `operationActive=true`.

After the Bash command returns, parse the JSON envelope. If it returns `actionRequired.autoContinue`, `actionRequired.mustRunImmediately`, `instruction.autoContinue`, or `instruction.mustRunImmediately`, immediately follow that returned instruction. Stop only for a real user gate, done report, blocked report, or non-repairable command failure.

For `status`, report only the current Loom state and user guidance from the returned JSON. Do not start a new delivery from `status`.

For knowledge routes, parse the returned JSON envelope and follow any returned CLI instruction. Otherwise report the user-facing result compactly. Knowledge commands are source management, build, search, context, inspect, enable, disable, remove, and semantic commands; they are not delivery requests, so do not start `plan`, Brainstorm, candidate generation, or task execution for them.

For every other non-status route, after the first Bash command returns and before acting on a returned request, user gate, candidate generation, task execution, review, repair, or deploy instruction, read `~/.claude/skills/loom/skills/loom/SKILL.md` unless that exact skill body is already loaded in this session. Then continue with that installed Loom adapter protocol: use returned `requestRef`, `agentAction.read.fieldGroups[].readCommand`, exact output paths, submit commands, Brainstorm block order, TaskExecution completion barriers, review/repair routing, and deploy boundaries from that protocol. Do not replace Loom with Claude Plan Mode, internal task state, or prose-only progress summaries; internal task/todo/subagent tools may be used only as implementation aids after Loom routing is established.
