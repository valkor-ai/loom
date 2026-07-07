# C++ Build Quality

## When To Use

- The task changes CMake, build scripts, compiler flags, dependency managers, test targets, sanitizer targets, static analysis, install/export rules, or C++ project layout.
- Use this when build configuration affects portability, correctness, dependency resolution, or verification.
- If source changes build under the existing configuration, do not churn build files.

## Implementation Focus

- Prefer target-scoped CMake configuration: `target_sources`, `target_include_directories`, `target_compile_features`, `target_compile_options`, and `target_link_libraries`. Avoid global flags when a target-level setting is enough.
- Keep the declared C++ standard aligned with code features. Do not add C++20/23 syntax while targets still compile as C++17 or older.
- Preserve existing package manager conventions: Conan, vcpkg, FetchContent, system packages, submodules, or vendored dependencies. Do not introduce a second dependency system casually.
- Keep public and private include paths separate. Public headers should not require consumers to know internal source directories.
- Wire test targets into the repository's existing test runner (`ctest`, Catch2, GoogleTest, custom scripts) so CI can discover them.
- Add sanitizer configurations only as explicit debug/verification targets or options. Do not force ASan/UBSan/TSan flags into normal release builds.
- Keep static analysis (`clang-tidy`, `cppcheck`, include-what-you-use) tied to compile commands and the project's current warning policy. Do not turn on huge rule sets without scope control.
- Preserve cross-platform behavior. Avoid Unix-only flags, paths, or commands in shared CMake unless guarded for platform/compiler.
- Update install/export/package metadata only when the task changes public library packaging.
- Do not rewrite build layout to match a template if the existing project has a coherent convention.

## Verification Focus

- Run configure and build commands that cover changed targets, such as `cmake -B build` and `cmake --build build`.
- Run `ctest` or the repository test command when test wiring changed.
- Run sanitizer or static-analysis targets when the task changed those paths or when memory/concurrency risk requires them.
- Confirm dependency and lock/manifest changes are required by the task and do not introduce duplicate package-manager ownership.

## Evidence Focus

- In the evidence summary, name the build decision: target-scoped config, standard selection, dependency manager, test wiring, sanitizer target, static analysis, platform guard, or install/export rule.
