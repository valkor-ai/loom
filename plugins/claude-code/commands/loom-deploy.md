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

Follow the returned MCP action result and do not create deployment assets or repair scopes outside that result. For `user_gate` with `preResponseContract`, execute the contract before any visible response: call `loom.inspectRequest`, then call `loom.readFieldGroup` for only required `requestReadPlan.groups`. During `active_operation`, call only the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it. During asset repair, edit only returned generated deployment assets or `modelRepairRef`; never edit generated source-model/topology/facts snapshots directly. Then call the returned `retryTool`; do not use `loom.deployRun` as a repair retry.

If the MCP result includes `deployReferenceProfile.referenceLoadPlan`, load the installed `loom-deploy` skill and follow its `Reference Loading` section. This command must not maintain a separate deploy reference path map.
