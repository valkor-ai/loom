# Go Module And Package Structure

## When To Use

Use this reference only when the task owns module/workspace files, package/dependency boundaries, command entry points, `internal`, build tags, generated/embed assets, configuration composition, or structure migration.

## Implementation Focus

### Preserve Existing Shape

Inspect module/workspace roots, packages, commands, internal/public consumers, generated code, build tags, tools, tests, and release pipeline before moving files.

Do not impose a textbook `cmd/internal/pkg` layout on a coherent repository. Structure follows actual binaries, reusable modules, and ownership boundaries.

Move in buildable slices and update imports, generated commands, tests, embeds, build scripts, docs/config, and consumers together. Remove temporary old/new ownership after cutover.

### Modules And Workspaces

Keep module path stable for published consumers unless a breaking migration is owned. `go.mod` `go`/toolchain directives, replaces/excludes/retracts, and dependency versions affect CI/consumers/reproducibility.

Use `go.work` for local multi-module development according to repository policy; do not commit machine-local `replace` paths or make production builds depend on an accidental workspace.

Run `go mod tidy` only after real import/build-tag changes and review `go.mod`/`go.sum` deltas. Do not churn indirect dependencies unrelated to the task.

Check licenses/vulnerabilities/maintenance and avoid importing `internal` packages across forbidden module trees.

### Packages And Dependency Direction

Name packages by cohesive capability/domain, short lowercase and not generic `utils/common/helpers/models/services` dumping grounds.

Keep import direction acyclic and ownership clear. Break cycles by moving an abstraction/value to the consumer/lower boundary, not by global registries or duplicated types.

Avoid one package per type and huge packages combining transport/domain/data/runtime. Package public API should reflect what other packages need.

Use `internal` to enforce application-private boundaries and public module packages only for supported consumers. `pkg` has no special compiler meaning and is not mandatory.

### Commands And Composition

Use `cmd/<binary>` for multiple/clear executable entry points when local convention supports it. Keep `main` focused on config, dependency construction, lifecycle, signals, and running application commands.

Business/domain logic stays in importable/testable packages. Avoid importing one command's internals from another.

Each server/worker/CLI has explicit startup validation, graceful shutdown, exit codes, and version/build metadata according to delivery contract.

### Build Tags And Platform Files

Use `//go:build` expressions plus filename suffixes for real platform/integration/tool/generated alternatives. Keep a buildable implementation for every supported tag/OS/arch combination.

Tagged files can hide compile/test failures; document/run commands in CI and avoid mutually overlapping duplicate definitions.

Do not use tags as runtime feature flags or to conceal unfinished code.

### Generated Code And Tools

Keep `go:generate`/tool commands deterministic/pinned, exact source inputs, output ownership, and checked-in policy. Do not hand-edit generated files.

Tool dependencies belong in the repository's tools pattern and must not leak into production binaries.

Use `go:embed` with compile-time-valid paths, bounded assets, clear package ownership, and production licensing/security. Embedded config/secrets cannot vary at runtime and remain in binaries.

### Configuration And Boundaries

Keep environment/file/flags loading in composition packages; domain packages receive validated typed config/dependencies.

Avoid package `init` side effects, global mutable registries, hidden environment reads, network calls, or goroutines that make import/start/test order unpredictable.

### Public Modules And Compatibility

For public packages, preserve import paths, exported identifiers, interface method sets, behavior, errors, and semantic version/module major rules. Use `internal` implementation packages to limit surface.

## Verification Focus

- Run `go list`, focused tests, and builds for all affected modules/commands/tag/platform combinations.
- Verify no import cycles, forbidden internal imports, stale generated output, or accidental workspace/local replace dependency.
- Review `go.mod`/`go.sum` changes and minimum Go/toolchain/consumer impact.
- Build/run entry points for config/start/shutdown/exit behavior when composition moves.
- Test embeds/generated/public consumers and clean checkout reproducibility.

## Evidence Focus

Name module/package/command boundary, import visibility, tag/generated/config decision, and affected module/command build proof. A tidy folder tree or default-tag test does not establish all consumers/platforms.

## Unsafe Defaults

- Template directory structure imposed without repository need.
- `utils/common/pkg` used as dependency dumping ground.
- Machine-local replace/workspace committed or required for CI.
- Build-tag path left uncompiled/unverified.
- Generated files edited manually or tools unpinned.
- init/global state hides config, network, goroutine, or registration ownership.
- Public import path/method set changed without compatibility plan.
