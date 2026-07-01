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

During `active_operation`, call only the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it.

For `DeployRepairAssetsNext`, edit only the returned generated deployment asset files and retry through the returned deploy tool. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

When a deploy repair or deploy execution repair result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Read only the field groups needed for the current repair action.

## Reference Loading

The current MCP deploy result remains the authority. Optional deploy references are installed under `../references/loom-deploy/`; load none by default. Load one file for the matching deploy action: repair, compose, dockerfile, environment, workspace, bootstrap, provider, external-reference review, or the detected runtime family.

Protocol:
- Read only references that match the current deploy action, repair request, or detected runtime family from the MCP result.
- Prefer the exact runtime family reference selected by the deploy result; do not scan the whole `../references/loom-deploy` directory.
- Use references as implementation guidance for generated or repaired deployment files; do not paste reference prose into deployment artifacts, repair results, or final chat output.
- If the current deploy action does not require a reference file, leave deploy references unread.

Do not copy deployment stack rules, repair contracts, runtime-family rules, or TaskResult contracts into this command. They belong to the current MCP deploy result or repair request.
