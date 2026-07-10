# Deployment Provider Reference

Use this reference when extending or repairing loom deploy provider selection.

## Current Strategy

loom v1 uses Dockerfile and Docker Compose only. It does not invoke Railpack, Buildpacks, Nixpacks, or other external builders.

Provider order:

1. Reuse root-level Compose files without overwriting them.
2. Reuse root-level Dockerfiles and generate only a local Compose wrapper.
3. Generate deterministic Dockerfile/Compose files for known or unknown stacks.
4. Return a bounded MCP repair action when build, boot, log, or health validation fails.

`loom.deployRun` is the preferred high-level MCP tool for normal agent use. It composes prepare, build/start, validate, status, and repair action reporting without hiding provider choice or switching builders.

`providerCandidates` in `.loom/deployment/specs/local.json` should describe the Compose/Dockerfile providers that were selected, available, or skipped, plus the commands that validate/build them.

## Provider Policy

Provider policy gives explicit user control over strategy selection through `DeployToolInput.providerPolicy`:

- `provider: "compose-existing"`: require a root-level Compose file.
- `provider: "dockerfile-existing"`: require a root-level Dockerfile and generate only the Compose wrapper.
- `provider: "generated"`: generate Loom Dockerfile/Compose assets instead of selecting an existing provider.
- `forceGenerate: true`: force generated Dockerfile/Compose assets and skip existing user assets.
- `reuseExisting: false`: disable existing Dockerfile/Compose reuse while keeping normal template generation.

If an explicitly selected existing provider has no matching file, return `INVALID_ARGUMENT` with a clear reason. Do not silently fall back to another provider.

Provider candidates should explain policy skips so repair/inspect output can tell whether a provider was unavailable or intentionally bypassed.

## Fallback Policy

Existing assets are tried first when the user did not force a provider. If an unforced existing Compose or Dockerfile provider cannot even build/start because of protected asset shape, Loom may fall back to the generated provider instead of asking the user to edit their files. This fallback must:

- preserve the user-owned Compose file or Dockerfile unchanged
- switch `providerPolicy` to generated with existing reuse disabled for the retry
- record the selected generated provider in the new DeploymentSpec
- keep protected user assets out of `editableFiles`

When the user explicitly selected `compose-existing` or `dockerfile-existing`, fallback is not allowed. Return a repair/blocker that explains the existing asset mismatch and the available user choices.

## Provider Rules

- Existing Compose is protected and never overwritten during `loom.deployPrepare`.
- Existing Dockerfiles are protected and reused with a generated Compose wrapper.
- Generated files live under `.loom/deployment/specs/generated/`.
- Unknown projects still receive a deterministic placeholder Dockerfile so a coding agent can inspect, repair, or explain the blocker.
- Generated fallback is a provider strategy, not a repair loop. Repair should fix generated assets or application/runtime issues after the selected provider is known.

## Guardrails

- Do not automatically switch provider after a failure when the user explicitly forced a provider.
- Do not introduce external builders unless the product explicitly adds them as a future provider family.
- Do not overwrite existing `Dockerfile` or Compose files without explicit user approval.
- A failure should produce a clear MCP repair action for a coding agent, not a chain of hidden retry strategies.
