---
description: Route Loom delivery, knowledge, and deploy commands through MCP.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
---

You are executing `/loom $ARGUMENTS` now.

First call the matching Loom MCP tool for the current project directory. Do not answer in prose, inspect `.loom`, or enter Plan Mode before the first Loom MCP tool call.

Route `$ARGUMENTS` as follows:

- `status` -> `loom.status`.
- `continue`, `resume`, `proceed`, `next`, or empty -> `loom.continue`.
- `verify` -> `loom.verify` without a decision to show the V-SEFM onboarding gate.
- `knowledge ...` -> the matching `loom.knowledge*` tool.
- `deploy` -> `loom.deployRun`.
- `deploy ...` -> the matching `loom.deploy*` tool.
- `plan <request>` -> `loom.plan` with `<request>`.
- Any other request text -> `loom.plan` with the full request text.

When `loom.plan` returns a plan-conflict user gate, present its numbered choices and descriptions exactly. Choice 1 calls `loom.planConflictResolve` with `continue_current`; choice 2 calls it with `start_new`. Do not call `loom.plan` again to resolve the conflict.

After the MCP tool returns, load the installed Loom skill if needed and follow the action result. Treat `auto_runnable` as a required continuation checkpoint: continue immediately and do not report progress, mark a local plan complete, send a final answer, or stop while `stopAllowed=false`. If a shell, patch, test, or nested MCP call fails, inspect the exact failure and retry the smallest corrective step in the same turn. When MCP is called through a wrapper, parse the nested structured result and its `state`; the wrapper's status is not the Loom workflow state. For `user_gate` with `preResponseContract`, execute the contract before any visible response: inspect the request, read only required `requestReadPlan.groups`, and complete required Brainstorm knowledge steps. For a gate whose `gate.kind` is `vsefm_onboarding`, present the returned content, wait for `1` or `2`, then call `loom.verify` with `decision=required` for `1` or `decision=deferred` for `2` without waiting for an external result. For `repairable_error`, inspect the returned request, read every required repair group, then edit only the returned target and resubmit with the returned tool. Do not answer from the gate prompt alone or bypass it with `/loom continue`. Ask only after those steps, then wait for the accepted user response. Stop for `done`, `blocked`, or `failed`.

For `RunLoomToolNext`, inspect the requestRef, read only the returned readGroups, call the returned Loom MCP tool, then retry the returned retryTool before reporting completion.
