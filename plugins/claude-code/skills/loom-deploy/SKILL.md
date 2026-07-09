---
name: loom-deploy
description: Use when the user invokes /loom deploy or /loom-deploy to prepare, run, inspect, validate, stop, bootstrap, or repair deployment through Loom MCP.
---

# loom-deploy

Deployment is controlled by Loom MCP deploy tools. Use the matching `loom.deploy*` tool and follow its structured action result.

During `active_operation`, call only the observation tools named by the result, obey `observationPolicy`, obey `forbiddenActions`, and do not report completion while `finalResponsePolicy` forbids it. During asset repair, edit only the returned generated deployment assets, then call the returned `retryTool`; do not retry asset repair through `loom.deployRun`. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not infer stack topology, generated file paths, preview URLs, ports, or repair scope outside the current MCP result.

## Reference Loading

The current MCP deploy result remains the authority. Load no reference by default; load deploy references only when the MCP result supplies an exact reference load plan.

Protocol:
- After a deploy MCP result, look for `next.deployReferenceProfile.referenceLoadPlan` or `details.deployReferenceProfile.referenceLoadPlan`.
- Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`. Resolve `path` relative to this skill's `references/` directory.
- Read only the listed paths. Do not infer extra files from `refId`, stack names, failure text, or the `references/` directory.
- Use references as implementation guidance for generated or repaired deployment files; do not paste reference prose into deployment artifacts, repair results, or final chat output.
- If the current deploy action has no `deployReferenceProfile`, leave deploy references unread.
