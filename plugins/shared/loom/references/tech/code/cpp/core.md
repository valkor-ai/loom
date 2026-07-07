# C++ Core Quality

## When To Use

- The task changes C++ application, library, embedded, systems, CLI, service, or shared module code.
- Use this for baseline C++ correctness: standard version, ownership, RAII, const correctness, header boundaries, and error style.
- If the task only changes build metadata, generated files, or non-C++ code, do not expand scope because this reference is available.

## Implementation Focus

- Confirm the C++ standard and compiler support from build files before using modern language/library features. Do not assume C++20/23 when the target is older.
- Use RAII for every resource that must be released: file handles, sockets, locks, memory, threads, GPU handles, OS handles, and temporary state. Avoid manual cleanup paths scattered across call sites.
- Represent ownership explicitly. Use value types where practical, `std::unique_ptr` for exclusive ownership, `std::shared_ptr` only for real shared lifetime, and raw pointers/references for non-owning access.
- Avoid raw `new` and `delete` in application code. If a low-level allocator or placement-new path is required, isolate it and document ownership/destruction rules.
- Keep interfaces const-correct. Mark read-only member functions `const`, pass large values by `const&` or view/span where appropriate, and return mutable references only when mutation is part of the contract.
- Keep headers lean: include what the public type needs, forward declare where safe, avoid `using namespace` in headers, and keep implementation-only dependencies in `.cpp` files.
- Follow one error strategy per boundary. Do not mix exceptions, status codes, nullable returns, and assertions for the same class of recoverable failure.
- Use C++ casts (`static_cast`, `dynamic_cast`, `reinterpret_cast`, `const_cast`) intentionally and keep dangerous casts local. Avoid C-style casts.
- Treat undefined behavior as a defect, not an optimization. Bounds, lifetime, alignment, iterator invalidation, and signed overflow risks need explicit handling.
- Keep public APIs stable and documented when the library is consumed outside the target. Implementation helpers should stay in internal namespaces or translation units.

## Verification Focus

- Run the configured CMake/build target and test target for changed C++ code.
- Treat compiler warnings from changed files as defects unless the repository explicitly permits them.
- Add tests for ownership-sensitive paths, error branches, boundary checks, and resource cleanup touched by the task.
- Run sanitizers when memory lifetime, pointer arithmetic, undefined behavior, or low-level resource handling changed and the project supports sanitizer builds.

## Evidence Focus

- In the evidence summary, name the C++ decision made: standard compatibility, RAII owner, smart pointer choice, const-correct API, header hygiene, error strategy, cast boundary, or UB prevention.
