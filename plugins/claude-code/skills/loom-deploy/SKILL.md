---
name: loom-deploy
description: Use when the user invokes /loom deploy or /loom-deploy to prepare, run, inspect, validate, stop, bootstrap, or repair deployment through Loom MCP.
---

# loom-deploy

Deployment is controlled by Loom MCP deploy tools. Use the matching `loom.deploy*` tool and follow its structured action result.

During `active_operation`, call only the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it. During asset repair, edit only the returned generated deployment assets, then call the returned `retryTool`; do not retry asset repair through `loom.deployRun`. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not infer stack topology, generated file paths, preview URLs, ports, or repair scope outside the current MCP result.

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
