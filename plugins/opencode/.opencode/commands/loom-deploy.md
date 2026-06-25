---
description: Route Loom deployment commands through MCP.
argument-hint: "[prepare|up|status|inspect|validate|logs|bootstrap|down|repair]"
---

# loom-deploy

Call the matching Loom MCP deploy tool for the current project directory.

- empty -> `loom.deployRun`
- `prepare` -> `loom.deployPrepare`
- `up` -> `loom.deployUp`
- `status` -> `loom.deployStatus`
- `inspect` -> `loom.deployInspect`
- `validate` -> `loom.deployValidate`
- `logs` -> `loom.deployLogs`
- `bootstrap` -> `loom.deployBootstrap`
- `down` -> `loom.deployDown`
- `repair` -> `loom.deployRepair`

Follow the returned action result. Do not invent deployment assets, topology, repair scope, preview URLs, or ports outside that MCP result.
