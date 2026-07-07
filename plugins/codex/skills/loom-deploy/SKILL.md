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

Follow the returned action result. During `active_operation`, only call the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it. During deployment asset repair, edit only the returned generated deployment assets, then call the returned `retryTool`; do not retry asset repair through `loom.deployRun`. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not invent deployment files, stack choices, preview URLs, ports, or repair scopes outside the current MCP result.

## Reference Loading

The current MCP deploy result remains the authority. Load no reference by default; load deploy references only when the MCP result selects them by id.

Protocol:
- After a deploy MCP result, look for `next.deployReferenceProfile.referenceIds` or `details.deployReferenceProfile.referenceIds`.
- Read only the selected reference ids. Do not infer extra files from stack names, failure text, or the `references/` directory.
- Use references as implementation guidance for generated or repaired deployment files; do not paste reference prose into deployment artifacts, repair results, or final chat output.
- If the current deploy action has no `deployReferenceProfile`, leave deploy references unread.

MCP-selected deploy references:
- `deploy.providers` -> `references/providers.md`.
- `deploy.compose` -> `references/compose.md`.
- `deploy.dockerfile` -> `references/dockerfile.md`.
- `deploy.environment` -> `references/environment.md`.
- `deploy.workspaces` -> `references/workspaces.md`.
- `deploy.bootstrap` -> `references/bootstrap.md`.
- `deploy.repair` -> `references/repair.md`.
- `deploy.stacks.node`, `deploy.stacks.python`, `deploy.stacks.go`, `deploy.stacks.java`, `deploy.stacks.dotnet`, `deploy.stacks.php`, `deploy.stacks.ruby`, `deploy.stacks.static` -> the matching runtime-family file under `references/`.
- `external-references.md` is maintainer research material only. Do not load it during normal deploy prepare, up, validate, inspect, or repair.
