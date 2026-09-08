# Contributing to Loom

Thanks for helping improve Loom.

## Issues

Before opening an issue, please check whether a similar issue already exists.

Use the issue templates when possible:

- Bug reports should include reproduction steps, environment details, and logs or screenshots.
- Feature requests should describe the problem, proposed solution, and expected workflow.

For questions, early ideas, or community discussion, join Discord:

https://discord.gg/Yr7UjwbYPC

## Pull Requests

Please keep pull requests focused and easy to review:

- Use a branch name such as `feature/my-change`, `fix/my-bug`, `docs/readme-update`, or `chore/tooling-update`.
- Use Conventional Commit messages such as `feat: add new capability`, `fix: correct broken behavior`, or `docs: update README`.
- Include a short summary and testing notes in the PR description.
- Keep unrelated refactors out of the PR unless they are required for the change.
- Resolve review conversations before merging.
- Squash focused changes into one clear commit when merging.

## Local Checks

For Rust changes, run the relevant checks before opening a PR:

```bash
cargo fmt --manifest-path src/rust/Cargo.toml --all --check
cargo check --manifest-path src/rust/Cargo.toml --workspace
cargo test --manifest-path src/rust/Cargo.toml --workspace --lib
cargo build --manifest-path src/rust/Cargo.toml -p mcp-server -p setup
```

For changes to the Python algorithm worker, install its local dependencies and run:

```bash
python3 -m pip install -e src/python/algorithms pytest
npm run python:test
```

For release-sensitive changes, also run `npm run rust:test`. For documentation-only changes, a visual review of the rendered Markdown is usually enough.

## Security

Please do not open public issues for sensitive security problems. See [SECURITY.md](SECURITY.md) for the reporting process.
