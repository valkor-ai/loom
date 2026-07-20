# Swift Core Quality

This file applies core Swift language quality to task-owned changes.

## When To Use

- The task changes Swift models, services, view models, controllers, package code, API clients, error handling, platform availability, or typed domain logic.
- Use this for value semantics, optionals, access control, API design guidelines, `throws`/`Result`, Codable, property wrappers, and platform checks.
- If the task only changes generated project metadata or non-Swift assets, do not expand into Swift refactoring.

## Implementation Focus

- Prefer structs and enums for values, state, and domain concepts. Use classes when identity, reference semantics, Objective-C interop, or framework lifecycle requires them.
- Make invalid states hard to represent with enums, non-optional properties, small value objects, and failable/throwing initializers where appropriate.
- Avoid force unwraps and implicitly unwrapped optionals except at framework-required boundaries with a local justification. Convert optional absence into explicit user/system behavior.
- Use `throws` for recoverable operation failures and `Result` when storing/passing a success-or-failure value is itself the API. Do not mix both without a reason.
- Follow Swift API naming: labels should make call sites read naturally, types should be nouns, mutating methods should make mutation obvious, and access control should be narrow.
- Keep Codable/API models separate from persistence or UI state when wire shape, domain rules, and view state differ.
- Use property wrappers for real cross-cutting behavior such as state, persistence, environment, or validation. Do not hide ordinary assignment behind a wrapper.
- Add availability checks or platform-specific compilation where APIs differ across iOS, macOS, watchOS, tvOS, server Swift, or package targets.
- Keep Objective-C patterns, singletons, notification sprawl, and global mutable state out of new Swift code unless the existing platform boundary requires them.

## Boundary Decisions

- Choose value or reference semantics from identity, shared mutation, lifecycle, and framework requirements. A `class` is not a default substitute for a model that can be a `struct` or enum.
- Keep optional absence explicit at API, persistence, UI, and platform boundaries. Do not turn a missing value into an empty string or force unwrap merely to satisfy a compiler branch.
- Separate Codable transport models, domain types, persistence records, and view state when their invariants or wire shapes differ. Do not expose storage or server fields through UI models by accident.
- Use `throws` for an operation's immediate failure contract and `Result` when success/failure must be stored, combined, or passed as data. Keep error mapping at the boundary that owns user/system output.
- Keep access control narrow and API names readable at call sites. Add `///` documentation where a public package or framework contract needs lifecycle, threading, availability, or failure expectations.
- Treat availability and platform compilation conditions as part of the supported target matrix. Verify the selected target rather than assuming an API available on the local host works on iOS, macOS, watchOS, tvOS, or server Swift.

## Verification Focus

- Run `swift build`, Xcode target build, or the repository's equivalent compile command.
- Run tests that cover success, optional absence, error paths, serialization/mapping, and platform availability touched by the task.
- Treat compiler warnings, deprecation warnings, and concurrency/sendability warnings as signals to fix or explicitly record.

## Evidence Focus

- In the evidence summary, name the Swift decision: value/reference semantics, optional handling, error API, access control, Codable separation, property wrapper, platform availability, or compile proof.
