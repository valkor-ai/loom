---
description: Route Loom delivery, knowledge, and deploy commands through MCP.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
---

# loom

You are executing `/loom $ARGUMENTS` now.

Call the matching Loom MCP tool for the current project directory before doing any other work.

- `status` -> `loom.status`
- `continue`, `resume`, `proceed`, `next`, or empty -> `loom.continue`
- `knowledge ...` -> matching `loom.knowledge*` tool
- `deploy` -> `loom.deployRun`
- `deploy ...` -> matching `loom.deploy*` tool
- `plan <request>` or any other request text -> `loom.plan`

After the tool returns, follow `LoomMcpActionResult.state`: continue immediately for `auto_runnable`, ask only for `user_gate`, repair only returned targets for `repairable_error`, and stop only for `done`, `blocked`, or `failed`.

Use `requestReadPlan.groups` through `loom.inspectRequest` and `loom.readFieldGroup`. Write only to returned `writeTargets` and submit only through the returned MCP submit tool.

The current MCP request/result remains the authority. Optional references are installed under `../references/loom/`; load none by default. Load `uix/core.md` for user-visible frontend artifacts, then one focused UIX file for interaction, system, mobile, framework, content, data, or verification work. Load one delivery file for the matching action: planning, design, domain, testing, review, repair, or handoff.
