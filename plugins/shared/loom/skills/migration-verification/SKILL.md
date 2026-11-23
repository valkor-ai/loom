---
name: migration-verification
description: Plan or review verification for schema and data migrations, API deprecations, and rolling upgrades where old and new versions must coexist.
---

# Migration Verification

Identify the supported upgrade paths, consumers, and rollout stages before choosing checks. For each stage, record the deployed versions, data or contract shape, evidence required to proceed, and available recovery action.

Resolve companion links from the directory of the actual `SKILL.md` you loaded.

- For schema and backfill changes, read [data migration](references/data-migration.md).
- For API, event, CLI, or configuration changes, read [consumer compatibility](references/consumer-compatibility.md).
- For staged rollout, rollback, or forward repair, read [rollout and recovery](references/rollout-and-recovery.md).
- Use the [verification matrix](templates/verification-matrix.md) to record relevant checks and gaps.

Use existing Loom task and Review procedures when working in a delivery. Rehearse with representative fixtures or a disposable environment. Production rollout, data mutation, and cleanup follow the project's existing authorization and deployment procedures.

Reference: [Addy Osmani's deprecation and migration guidance](https://github.com/addyosmani/agent-skills/tree/main/skills/deprecation-and-migration).
