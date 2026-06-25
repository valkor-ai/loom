# External Docker Skill References

Use this file only when evaluating whether loom deploy should absorb ideas from external Docker/agent skill projects. Do not vendor third-party skills or generators directly into loom unless the product explicitly adopts their license, update policy, and runtime contract.

## Reviewed Sources

- Docker Compose Generator Skill: `https://github.com/joocn619/agent-skills-pack/blob/main/skills/docker-compose-gen/SKILL.md`
- Docker Claude Skill Package: `https://github.com/OpenAEC-Foundation/Docker-Claude-Skill-Package`
- Docker GenAI / MCP guidance: `https://docs.docker.com/guides/genai-claude-code-mcp/claude-code-mcp-guide/`
- Docker agent skills reference: `https://docker.github.io/docker-agent/features/skills/`
- AI Dockerfile generator examples:
  - `https://github.com/kfirc/dockerfile-generator`
  - `https://github.com/MarkBenjaminKatamba/dockerfile-generator`

## Adopted Ideas

- Treat Dockerfile and Compose creation as a bounded skill with clear inputs, outputs, and validation steps.
- Keep reusable Docker guidance in small stack/environment references instead of one large prompt.
- Prefer deterministic generated artifacts plus validation logs over opaque generation.
- Keep agent instructions focused on inspect, patch, rerun, and explain.
- Separate local runtime tooling from knowledge references so future agents can use the same deployment contract.

## Not Adopted

- Do not make external skills runtime dependencies for `loom deploy`.
- Do not copy third-party `SKILL.md` text into loom.
- Do not call external Dockerfile generator projects as part of the local deploy path.
- Do not bind the deploy workflow to one agent vendor or one editor/plugin surface.
- Do not add MCP-only behavior to v1. MCP/Docker Desktop integrations can become optional adapters later.

## Evaluation Rules

When adding a new external idea, decide where it belongs:

- Stable Docker/Compose practice -> `references/dockerfile.md` or `references/compose.md`.
- Stack-specific deployment pattern -> `references/<stack>.md`.
- Provider selection behavior -> `references/providers.md`.
- Repair-loop behavior -> `references/repair.md`.
- Runtime code -> `src/core/deployment/`, with tests or smoke coverage.

If an idea cannot be validated through `deploy prepare`, `deploy up`, `deploy validate`, or `deploy repair`, keep it as reference guidance only.
