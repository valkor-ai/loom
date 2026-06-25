---
name: loom-deploy
description: Use when the user invokes /loom deploy or /loom-deploy to prepare, run, inspect, validate, stop, bootstrap, or repair deployment through Loom MCP.
---

# loom-deploy

Deployment is controlled by Loom MCP deploy tools. Use the matching `loom.deploy*` tool and follow its structured action result.

During `active_operation`, call only the observation tools named by the result. During asset repair, edit only the returned generated deployment assets. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not infer stack topology, generated file paths, preview URLs, ports, or repair scope outside the current MCP result.

## Optional References

The current MCP deploy result remains the authority. Load no reference by default; load references only when the current deploy action matches the trigger.

- `references/repair.md`: executing a returned deploy repair request.
- `references/compose.md`: editing Compose files, service wiring, build contexts, ports, or environment blocks.
- `references/dockerfile.md`: editing Dockerfiles or Docker ignore files.
- `references/environment.md`: missing env, secrets, framework config, or runtime configuration.
- `references/workspaces.md`: monorepo/workspace roots, app paths, or generated context paths.
- `references/bootstrap.md`: database schema setup, migrations, seed/bootstrap tasks, or missing tables.
- `references/providers.md`: provider selection or provider reuse guardrails.
- `references/external-references.md`: comparing external Docker/agent skill guidance before absorbing it.
- Runtime family files: load exactly the detected family file when stack-specific behavior is needed: `node.md`, `python.md`, `go.md`, `java.md`, `dotnet.md`, `php.md`, `ruby.md`, or `static.md`.
