---
description: Route Loom delivery, knowledge, and deploy commands through MCP.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
---

You are executing `/loom $ARGUMENTS` now.

First call the matching Loom MCP tool for the current project directory. Do not answer in prose, inspect `.loom`, or enter Plan Mode before the first Loom MCP tool call.

Route `$ARGUMENTS` as follows:

- `status` -> `loom.status`.
- `continue`, `resume`, `proceed`, `next`, or empty -> `loom.continue`.
- `knowledge ...` -> the matching `loom.knowledge*` tool.
- `deploy` -> `loom.deployRun`.
- `deploy ...` -> the matching `loom.deploy*` tool.
- `plan <request>` -> `loom.plan` with `<request>`.
- Any other request text -> `loom.plan` with the full request text.

After the MCP tool returns, load the installed Loom skill if needed and follow the action result. Continue immediately for `auto_runnable`; do not report progress, mark a local plan complete, send a final answer, or stop while `stopAllowed=false`. Ask only for `user_gate`, and stop only for `done`, `blocked`, or `failed`.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting completion.
