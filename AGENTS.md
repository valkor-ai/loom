# Repository Guidelines

## Project Structure & Module Organization

- `src/rust/`: workspace crates. `mcp-server` exposes tools; domain crates such as `planning`, `execution`, `deploy`, and `knowledge` own workflow behavior; `contracts`, `core`, and `state` provide shared protocols.
- `src/python/algorithms/`: bundled BM25, TF-IDF, tokenization, and worker code.
- `tests/rust/` and `tests/python/algorithms/`: integration and algorithm tests, organized by product domain.
- `plugins/{codex,claude-code,opencode}/`: agent adapters. Shared Loom and deploy guidance lives under `plugins/shared/`; avoid duplicating shared rules in adapter files.
- `docs/`, `assets/`, and `scripts/`: user documentation, README media, and local install helpers.

## Build, Test, and Development Commands

Run commands from the repository root:

```bash
npm run rust:test
npm run python:test
cargo fmt --manifest-path src/rust/Cargo.toml --all --check
cargo build --manifest-path src/rust/Cargo.toml -p mcp-server -p setup
```

Use targeted Rust tests while iterating, for example:

```bash
cargo test --manifest-path src/rust/Cargo.toml -p deploy --test deploy_workflow
```

Refresh a local integration after runtime or plugin changes with `./install.sh --agent codex --local-build`; Claude Code and OpenCode helpers are in `scripts/`.

## Coding Style & Naming Conventions

Use Rust 2021 conventions and keep code `rustfmt`-clean. Name modules, functions, and files with `snake_case`; types and traits with `UpperCamelCase`; constants with `SCREAMING_SNAKE_CASE`. Python follows four-space indentation, `snake_case`, and `test_*.py` naming. Keep changes within existing domain boundaries and prefer structured Serde models over ad hoc JSON manipulation.

## Testing Guidelines

Add focused regression coverage for behavioral changes. Rust integration tests belong in the matching `tests/rust/<domain>/` suite; local unit tests may remain beside implementation code. Python tests use `pytest`. Run the affected package or test target first, then both product test lanes before release-impacting changes. Fixes should reproduce the prior failure.

## Reference and Contract Design

- Select references from structured repository facts and task ownership; never globally default a focus or group.
- Audit existing contracts before adding fields or references; remove superseded and duplicate guidance in the same change.
- Do not ask agents to author fields that MCP derives.
- Test both selection and non-selection for every reference route.

## Commit & Pull Request Guidelines

Branches must match CI patterns such as `feature/name`, `fix/name`, or `docs/name`. Commit subjects use Conventional Commits, including an optional lowercase scope: `fix(deploy): validate source roots`. Keep subjects under 200 characters.

PRs should stay focused, explain the user-visible or contract impact, list verification commands, and link relevant issues. Include screenshots for documentation or UI-visible changes. Keep unrelated formatting and refactors out of the diff; never commit generated `.loom/` runtime state.
