---
name: loom-deploy
description: Use only for an explicit @loom deploy request or deployment operation; do not select this skill for a plain @loom software-delivery request.
---

# loom-deploy

Deployment is a Loom MCP workflow. Route deploy requests to the registered `loom.deploy*` tools.

This skill is not the route for a plain `@loom <request>`. Plain delivery requests must go through the `loom` skill and `loom.plan`; use this skill only when the user explicitly requests deployment.

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

Follow the returned action result. During `active_operation`, only call the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it. During deployment asset repair, edit only the returned generated deployment assets or the returned `modelRepairRef`, then call the returned `retryTool`; do not retry asset repair through `loom.deployRun`. Never edit generated source-model/topology/facts snapshots directly. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not invent deployment files, stack choices, preview URLs, ports, or repair scopes outside the current MCP result.

## Reference Loading

The current MCP deploy result remains the authority. Load no reference by default; load deploy references only when the MCP result supplies an exact reference load plan.

Protocol:
- After a deploy MCP result, look for `next.deployReferenceProfile.referenceLoadPlan` or `details.deployReferenceProfile.referenceLoadPlan`.
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. Resolve `path` relative to this skill's `references/` directory.
- Read only the listed paths. Do not infer extra files from `refId`, stack names, failure text, or the `references/` directory.
- Use references as implementation guidance for generated or repaired deployment files; do not paste reference prose into deployment artifacts, repair results, or final chat output.
- If the current deploy action has no `deployReferenceProfile`, leave deploy references unread.
