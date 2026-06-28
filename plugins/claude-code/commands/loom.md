---
description: Route Loom delivery, knowledge, and deploy commands through MCP.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
allowed-tools: [Read, Glob, Grep, Edit, MultiEdit, Write]
---

You are executing `/loom $ARGUMENTS` now.

First call the matching Loom MCP tool for the current project directory. Do not answer in prose, inspect `.loom`, or enter Plan Mode before the first Loom MCP tool call.

Route `$ARGUMENTS` as follows:

- `status` -> `loom.status`.
- `continue`, `resume`, `proceed`, `next`, or empty -> `loom.continue`.
- `knowledge ...` -> the matching `loom.knowledge*` tool.
- `deploy` -> `loom.deployRun`.
- `deploy ...` -> the matching `loom.deploy*` tool.
- `plan <request>` or any other request text -> `loom.plan`.

After the MCP tool returns, load the installed Loom skill if needed and follow the action result. Continue immediately for `auto_runnable`; when `continuationPolicy.mustContinue=true` or `continuationPolicy.progressReportAllowed=false`, do not report progress or stop until a returned stop condition is reached. Ask only for `user_gate`, and stop only for `done`, `blocked`, or `failed`.
