# C++ Build, Dependency, And Toolchain Configuration

## When To Use

Use this reference only when the task owns CMake/build configuration, language standard/toolchain change, dependency management, generated code, test/analysis targets, install/export/package rules, or build migration.

## Implementation Focus

### Discover The Build Graph

Inspect the actual entry point, presets/toolchain files, generators, targets, subdirectories, options, package manager, generated sources, tests, install/export, CI matrix, and supported compilers/platforms before editing.

Do not replace a coherent CMake, Bazel, Meson, Make, IDE, package-manager, or embedded build with a template for one feature.

Keep source and binary directories separate and avoid committing local build trees/cache/generated machine paths.

### Target-Scoped CMake

Use target-level sources, include directories, compile features/definitions/options, link libraries/options, generated files, and properties. Global flags/directories leak behavior to dependencies/tests/consumers.

Express PUBLIC/PRIVATE/INTERFACE according to the consumer contract. A public header dependency/feature/definition must propagate; implementation-only details must not. Use `target_compile_features`, `target_include_directories`, and `target_link_libraries` with the matching visibility instead of equivalent directory-global state.

Prefer compile features for the language standard and keep extensions policy explicit. Do not mix global `CMAKE_CXX_STANDARD` assumptions with targets declaring another level.

Use generator expressions for configuration/compiler/platform-specific behavior rather than configure-time branches that collapse multi-config generators incorrectly.

### Presets And Toolchains

Keep developer/CI presets reproducible without local absolute paths. Toolchain files own compiler/sysroot/target/package-manager integration before project configuration.

Support Ninja/Make/Visual Studio/Xcode or cross compilation only according to repository matrix. Avoid Unix shell commands/paths in portable custom commands.

Multi-config builds use `--config`; single-config builds use `CMAKE_BUILD_TYPE`. Do not assume one in shared scripts.

### Dependencies

Preserve Conan/vcpkg/FetchContent/system/submodule/vendor ownership and lock/version policy. Do not introduce a second manager or download unpinned moving branches during configure.

Use imported targets rather than global include/link variables when packages provide them. Verify static/shared/runtime library and transitive dependency behavior per platform.

Check license, supported compiler/runtime, C++ ABI/runtime linkage, and offline/reproducible build requirements before adding a package.

### Generated Code And Custom Commands

Declare exact inputs/outputs/byproducts/dependencies, use target-aware commands, create directories, and avoid races between parallel targets. Generated headers need correct binary include paths and build-before-use dependencies.

Quote paths/list arguments and use `VERBATIM`. Do not run generators at configure time when build-time dependencies should trigger regeneration.

### Warnings, Sanitizers, And Analysis

Apply repository warning policy per owned target and compiler. Do not turn third-party headers into warnings-as-errors accidentally.

Sanitizers require compatible compile and link flags, debug info/frame pointers as needed, and dedicated presets/options. Do not force ASan/UBSan/TSan/MSan into release/distribution or combine incompatible sanitizers.

Generate `compile_commands.json` or equivalent for clang-tidy/IWYU where supported and scope checks to project targets/files.

### Tests And Benchmarks

Register tests through existing CTest/Catch2/GoogleTest/custom conventions with working directory, environment, labels, fixtures, resources, and timeouts.

Benchmarks remain separate from correctness tests and normal startup. Analysis/test dependencies should not leak into production interfaces.

### Install, Export, And Packaging

For consumable libraries, preserve build/install include interfaces, export namespace/targets, version/config files, component/runtime destinations, RPATH/runtime DLL handling, symbol visibility, and CPack/package-manager metadata.

Relocatable packages must not embed source/build-machine absolute paths. Test installation through a small external consumer when public packaging changes.

## Verification Focus

- Configure from a clean build directory using affected presets/generators/toolchains.
- Build exact changed targets and affected compiler/platform/configuration matrix.
- Run registered tests/analysis/sanitizer targets and verify discovery, not only manual executable invocation.
- Reconfigure after dependency/generated-input changes to prove reproducibility and parallel ordering.
- Test install/export/package with an external consumer when public delivery changes.

## Evidence Focus

Name the target/property visibility, toolchain/preset/dependency/generated/install decision, and clean configure/build/test proof. Incremental local build success does not prove reproducibility, propagation, packaging, or cross-platform behavior.

## Unsafe Defaults

- Global compile/link flags used for target-local behavior.
- Moving/unpinned dependency downloaded during configure.
- Machine-specific paths committed in presets/cache/export files.
- Generated output missing declared dependencies/byproducts.
- Sanitizers forced into ordinary release builds.
- Installed targets embedding source/build paths.
