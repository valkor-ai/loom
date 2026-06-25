---
name: loom-deploy
description: Use when the user explicitly invokes @loom deploy or asks Loom to prepare, run, inspect, validate, stop, bootstrap, or repair a deployment through MCP.
---

# loom-deploy

Deployment is a Loom MCP workflow. Route deploy requests to the registered `loom.deploy*` tools.

- `@loom deploy` -> `loom.deployRun`.
- `@loom deploy prepare` -> `loom.deployPrepare`.
- `@loom deploy up` -> `loom.deployUp`.
- `@loom deploy status` -> `loom.deployStatus`.
- `@loom deploy inspect` -> `loom.deployInspect`.
- `@loom deploy validate` -> `loom.deployValidate`.
- `@loom deploy logs` -> `loom.deployLogs`.
- `@loom deploy bootstrap` -> `loom.deployBootstrap`.
- `@loom deploy down` -> `loom.deployDown`.
- `@loom deploy repair` -> `loom.deployRepair`.

Follow the returned action result. During `active_operation`, only call the observation tools named by the result. During deployment asset repair, edit only the returned generated deployment assets. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not invent deployment files, stack choices, preview URLs, ports, or repair scopes outside the current MCP result.
