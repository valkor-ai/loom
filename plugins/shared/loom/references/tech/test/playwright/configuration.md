# Playwright Project Configuration

Configuration should make assigned checks reproducible locally and in CI while preserving the repository's existing scripts, servers, package manager, and test layout.

## Adapt Before Creating

Inspect the selected project runner facts first:

- package root and package manager;
- dependency and resolved version;
- existing Playwright config;
- test roots and scripts;
- app start/preview commands;
- monorepo workspace filters;
- current CI artifact conventions.

Extend an accepted config. Do not replace it with a generic full matrix, rename established projects, or move test roots solely to match an example.

## Minimal New Configuration

For a new browser suite, start with one Chromium project and the viewports required by the profile. Add Firefox, WebKit, devices, locales, or color schemes only when product/browser support requirements call for them.

```typescript
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI
    ? [['line'], ['html', { open: 'never' }]]
    : [['list'], ['html', { open: 'never' }]],
  outputDir: 'test-results',
  use: {
    baseURL: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:4173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'desktop-primary',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'mobile-primary',
      use: { ...devices['Pixel 7'] },
    },
  ],
  webServer: process.env.E2E_EXTERNAL_SERVER
    ? undefined
    : {
        command: 'npm run preview -- --host 127.0.0.1',
        url: 'http://127.0.0.1:4173',
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
});
```

Remove `mobile-primary` when responsive coverage is not assigned. Replace commands and ports with the project facts; examples are not defaults.

## Viewport Mapping

Map profile viewport refs to named projects or per-test `test.use()` values:

| Profile ref | Configuration intent |
| --- | --- |
| `desktop_primary` | supported desktop browser and production work-surface width |
| `mobile_primary` | supported narrow/touch viewport with no hover dependency |

Keep viewport names stable so TaskResult evidence identifies what ran. Do not emulate a branded device unless device-specific behavior matters; viewport and input mode are usually enough for responsive web checks.

## Servers And Base URLs

- Prefer the repository's production-like preview/start command over a development server when build output is part of the check.
- Use `webServer` for a single project-owned process that Playwright can start and stop reliably.
- For composed frontend/backend systems, use the project's orchestration command or start managed processes in fixtures; do not hide multiple unmanaged foreground servers inside one shell string.
- Bind test servers to loopback unless external access is required.
- Derive base URLs from environment/config and avoid hard-coded occupied ports.
- A readiness URL must prove the app is ready enough for the assigned check, not merely that a process opened a socket.

## Shared Browser Runtime

The project owns `@playwright/test`, config, scripts, and lockfile. MCP resolves the exact project version from installed dependencies or lockfiles, prepares only the selected browsers, and attaches the matching execution environment to the closure request.

- Host caches are isolated by OS/CPU and keyed by exact Playwright version plus browser set. Never construct or persist a cache path yourself.
- A `partial` runtime matrix is runnable: select the environment matching the current project runner and mark only checks tied to an unavailable target as blocked. Do not treat an unrelated workspace version as a global browser outage.
- For a host backend, apply the returned `browserEnvironment` to the project-local runner.
- When host launch fails, Loom may return a managed-container backend with an exact-version image, command prefix, mount path, and browser environment. Run through that descriptor; do not invent another image or run `playwright install --with-deps` in the project.
- When `projectRunner` is absent and the closure is authorized to establish Playwright, add the first project-owned `@playwright/test` dependency at the exact `resolvedVersion` supplied by the runtime, create the minimal config/test root, and update the project lockfile. Do not choose a different range or latest version after runtime preparation.
- If the preview/API runs on the host, replace only the loopback hostname with the returned `hostGateway` and preserve the discovered port. Services running in an existing container network require that network's supported address instead.
- Keep the project runner version aligned with the prepared browser revision; restore project dependencies when the execution request identifies stale package facts.
- Do not commit the shared cache path, downloaded browser binaries, or machine-specific absolute paths.
- Do not copy Loom's shared runner `node_modules` into the repository. The shared runner is a preparation/doctor asset; project tests still use the project-owned package and config.

## Artifacts And Reporters

- Keep `outputDir`, HTML report, JUnit, and blob report paths predictable and ignored unless the repository intentionally stores baselines.
- Use trace on first retry or retain on failure; always-on traces are expensive and can expose sensitive data.
- Retain screenshots/video on failure according to project policy.
- CI upload should run even when tests fail and should use bounded retention.
- Do not emit credentials, authorization headers, or full confidential payloads into reports.

## Timeouts And Retries

- Keep action/assertion timeouts close to expected UI latency.
- Give server startup a separate timeout; do not inflate all assertions because compilation is slow.
- Local retries should normally be zero so instability is visible.
- CI retries may capture diagnostics, but retry success remains visible evidence and repeated retries require repair.
- Do not set workers to one globally unless shared infrastructure truly cannot isolate state.

## Authentication Projects

Use a setup project and dependencies when several suites share role-specific storage state. Keep state files in ignored output and make role names explicit. A test for authentication itself must not depend on pre-authenticated state.

Refresh expired state through the setup project, keep credentials in environment/secret storage, and do not let an administrator state leak into lower-privilege projects.

## Monorepos

- Put config and tests at the package root that owns the browser app unless the repo has a central E2E workspace.
- Invoke through the package manager's workspace/filter syntax.
- Resolve web server cwd, build output, environment files, and artifact paths from that package root.
- Do not assume the repository root contains the frontend manifest.

## Configuration Verification

Before running the full check, verify that Playwright can list the selected tests/projects and that config resolution points at the intended base URL, test root, and output directory. A config that discovers zero tests is not a pass.

Also verify that the selected package script exits, artifact paths are writable/ignored, and a failed check retains the configured diagnostic artifact rather than opening an interactive report in automation.
