# Deployment Provider Reference

Use this reference when extending or repairing loom deploy provider selection.

## Current Strategy

loom v1 uses Dockerfile and Docker Compose only. It does not invoke Railpack, Buildpacks, Nixpacks, or other external builders.

Provider order:

1. Reuse root-level Compose files without overwriting them.
2. Reuse root-level Dockerfiles and generate only a local Compose wrapper.
3. Generate deterministic Dockerfile/Compose files for known or unknown stacks.
4. Write a bounded repair request when build, boot, log, or health validation fails.

`loom deploy run` is the preferred high-level command for normal agent use. It composes prepare, build/start, validate, status, and repair-request reporting without hiding provider choice or switching builders.

`providerCandidates` in `.loom/deployment/specs/local.json` should describe the Compose/Dockerfile providers that were selected, available, or skipped, plus the commands that validate/build them.

## Provider Policy

Provider policy gives explicit user control over strategy selection:

- `--provider compose-existing`: require a root-level Compose file.
- `--provider dockerfile-existing`: require a root-level Dockerfile and generate only the Compose wrapper.
- `--provider dockerfile-template`: generate loom Dockerfile/Compose assets even if user assets exist.
- `--force-generate`: force generated Dockerfile/Compose assets and skip existing user assets.
- `--reuse-existing false`: disable existing Dockerfile/Compose reuse while keeping normal template generation.

If an explicitly selected existing provider has no matching file, return `INVALID_ARGUMENT` with a clear reason. Do not silently fall back to another provider.

Provider candidates should explain policy skips so repair/inspect output can tell whether a provider was unavailable or intentionally bypassed.

## Provider Rules

- Existing Compose is protected and never overwritten during `deploy prepare`.
- Existing Dockerfiles are protected and reused with a generated Compose wrapper.
- Generated files live under `.loom/deployment/specs/generated/`.
- Unknown projects still receive a deterministic placeholder Dockerfile so a coding agent can inspect, repair, or explain the blocker.

## Guardrails

- Do not automatically switch provider after a failure.
- Do not introduce external builders unless the product explicitly adds them as a future provider family.
- Do not overwrite existing `Dockerfile` or Compose files without explicit user approval.
- A failure should produce a clear repair request for a coding agent, not a chain of hidden retry strategies.
