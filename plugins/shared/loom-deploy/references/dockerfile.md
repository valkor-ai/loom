# Dockerfile Deployment Reference

Use this reference when implementing or repairing generated Dockerfiles or Dockerfile-specific ignore files.

## Generation Rules

- Prefer multi-stage builds when a stack has a build step or compiled artifact.
- Keep generated files under `.loom/deployment/specs/generated/`; do not overwrite a user-owned root `Dockerfile`.
- Generate Dockerfiles per deployable app service. Do not combine unrelated frontend/backend commands in one image unless the source model explicitly says one runtime process serves both.
- Align `COPY`, dependency install, build, and start commands with `sourceModel.services[].root`, `workingDirectory`, package manager, and build context. A Dockerfile generated beside `.loom/deployment/specs/generated/` still runs against the Compose build context.
- Use explicit, maintained base images already chosen by the stack reference, such as `node:22-slim`, a project-detected `node:<major>-slim`, `python:3.12-slim`, `golang:1.23-alpine`, Eclipse Temurin Java images, Microsoft .NET images, official PHP images, or official Ruby images.
- Choose the base image with platform in mind. Debian/Ubuntu/slim images use glibc; Alpine uses musl. Native optional dependencies must match the container OS/libc/CPU, not the host machine.
- Copy dependency manifests before source files so dependency install layers cache well.
- Use lockfile-aware installs where the stack supports them.
- Set a deterministic `WORKDIR`, usually `/app`.
- Bind runtime servers to `0.0.0.0`, never `127.0.0.1`.
- Expose only the detected container port. Compose owns host port publishing.
- Do not copy `.env`, local databases, caches, `node_modules`, virtualenvs, build outputs, or VCS metadata into images unless the project explicitly requires it.
- When serving a built frontend through a backend image, copy only the built static output into the backend's static resource directory and keep the backend start command as the single runtime process.

## Context And Workdir Checks

Before changing a Dockerfile, verify the path triangle:

- Compose `build.context`
- Compose `build.dockerfile`
- Dockerfile `WORKDIR` and every `COPY` source

The Docker build context controls which files can be copied. If a `COPY frontend/package.json` command runs with `build.context: ./service`, it will fail even if the file exists in the repository. Fix the context or the copy path; do not paper over it with broad `COPY . .` unless that is the intended context.

For generated workspace-aware Dockerfiles, copy root lockfiles and workspace manifests first, then service manifests, then source. For app-local generated Dockerfiles, keep paths relative to that app root.

## Ignore Files

- Prefer Dockerfile-specific ignore files beside generated Dockerfiles, for example `Dockerfile.dockerignore`.
- Include large and sensitive local state:
  - `.git`
  - `.loom`
  - `.env`
  - `.env.*`
  - `node_modules`
  - `.next`
  - `.turbo`
  - `.vercel`
  - `out`
  - `.venv`
  - `venv`
  - `__pycache__`
  - `dist`
  - `build`
  - `coverage`
  - `.DS_Store`
- Keep ignore rules conservative. Do not ignore lockfiles, dependency manifests, source directories, migrations, public assets, or framework config files by default.

## Runtime Rules

- Prefer the runtime command detected by the scanner, then the stack reference default.
- Generated Dockerfiles should be understandable to a coding agent and easy to patch after a build/start failure.
- Add a non-root runtime user only when it does not break common framework behavior or require project-specific file ownership changes.
- Avoid Dockerfile `HEALTHCHECK` unless the stack has a reliable HTTP endpoint. Compose validation can probe health externally.
- Do not bake secrets into `ARG`, `ENV`, or copied files.

## Repair Clues

- Install failures usually point to missing lockfile handling, missing system packages, wrong package manager, or build context ignores.
- Missing native modules such as `@next/swc-linux-*` or `lightningcss.linux-*.node` usually mean the lockfile or install step only included host-platform optional dependencies. Repair by installing the Linux glibc/musl package that matches the image, or by switching the image family.
- Start failures usually point to wrong command, missing build artifacts, wrong bind host, or wrong port.
- For compiled stacks, distinguish build-stage failures from runtime-stage missing binary/files.
- If the build cannot find project files that exist locally, inspect Compose build context and `.dockerignore` before changing package manager commands.
