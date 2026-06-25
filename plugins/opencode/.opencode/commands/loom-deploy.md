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

The current MCP deploy result remains the authority. Optional deploy references are installed under `../references/loom-deploy/`; load none by default. Load one file for the matching deploy action: repair, compose, dockerfile, environment, workspace, bootstrap, provider, external-reference review, or the detected runtime family.
