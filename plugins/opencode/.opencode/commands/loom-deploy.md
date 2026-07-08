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

For `DeployRepairAssetsNext`, edit only the returned generated deployment asset files and retry through the returned `retryTool`; do not use `loom.deployRun` as a repair retry. For deploy execution repair, edit only the allowed application/runtime files and submit through the returned repair submit tool.

When a deploy repair or deploy execution repair result contains `requestRef`, use `loom.inspectRequest` and `loom.readFieldGroup`. `requestReadPlan.groups` is the only read contract. Read only the field groups needed for the current repair action.

## Reference Loading

The current MCP deploy result remains the authority. Optional deploy references are installed under `../references/loom-deploy/`; load none by default. Load deploy references only when the MCP result selects them by id.

Protocol:
- After a deploy MCP result, look for `next.deployReferenceProfile.referenceIds` or `details.deployReferenceProfile.referenceIds`.
- Read only the selected reference ids. Do not infer extra files from stack names, failure text, or the `../references/loom-deploy` directory.
- Use references as implementation guidance for generated or repaired deployment files; do not paste reference prose into deployment artifacts, repair results, or final chat output.
- If the current deploy action has no `deployReferenceProfile`, leave deploy references unread.

MCP-selected deploy references:
- `deploy.providers` -> `../references/loom-deploy/providers.md`.
- `deploy.compose` -> `../references/loom-deploy/compose.md`.
- `deploy.dockerfile` -> `../references/loom-deploy/dockerfile.md`.
- `deploy.environment` -> `../references/loom-deploy/environment.md`.
- `deploy.workspaces` -> `../references/loom-deploy/workspaces.md`.
- `deploy.bootstrap` -> `../references/loom-deploy/bootstrap.md`.
- `deploy.repair` -> `../references/loom-deploy/repair.md`.
- `deploy.stacks.node`, `deploy.stacks.python`, `deploy.stacks.go`, `deploy.stacks.java`, `deploy.stacks.dotnet`, `deploy.stacks.php`, `deploy.stacks.ruby`, `deploy.stacks.static` -> the matching runtime-family file under `../references/loom-deploy/`.
- `external-references.md` is maintainer research material only. Do not load it during normal deploy prepare, up, validate, inspect, or repair.

Do not copy deployment stack rules, repair contracts, runtime-family rules, or TaskResult contracts into this command. They belong to the current MCP deploy result or repair request.
