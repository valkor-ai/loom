---
description: Route Loom deployment commands through MCP.
argument-hint: "[prepare|up|status|inspect|validate|logs|bootstrap|down|repair]"
---

You are executing `/loom-deploy $ARGUMENTS` now.

Call the matching Loom MCP deploy tool for the current project directory:

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

Follow the returned MCP action result and do not create deployment assets or repair scopes outside that result. During asset repair, edit only returned generated deployment assets and then call the returned `retryTool`; do not use `loom.deployRun` as a repair retry.
