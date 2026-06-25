---
name: loom-deploy
description: Use when the user invokes /loom deploy or /loom-deploy to prepare, run, inspect, validate, stop, bootstrap, or repair deployment through Loom MCP.
---

# loom-deploy

Deployment is controlled by Loom MCP deploy tools. Use the matching `loom.deploy*` tool and follow its structured action result.

During `active_operation`, call only the observation tools named by the result. During asset repair, edit only the returned generated deployment assets. During deploy execution repair, edit only the returned application/runtime files and submit through the returned repair submit tool.

Do not infer stack topology, generated file paths, preview URLs, ports, or repair scope outside the current MCP result.
