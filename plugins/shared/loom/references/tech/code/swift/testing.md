# Swift Testing Quality

Use this topic reference when `tech/code/swift/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies Swift testing guidance to task-owned changes.

## When To Use

- The task changes Swift behavior, view models, networking, persistence, async flows, actors, SwiftUI surfaces, package modules, or test files.
- Use this when XCTest, Swift Testing, UI tests, snapshot tests, performance tests, async tests, or test doubles are the right proof.
- If no Swift behavior is touched, do not add Swift tests just because this reference is selected.

## Implementation Focus

- Follow the repository's test framework and target layout: XCTest, Swift Testing, Xcode UI tests, package tests, snapshot library, naming, helpers, and fixtures.
- Use dependency injection through protocols, closures, clocks, URL protocols, storage adapters, or fakes to make behavior testable without real networks or device state.
- Test async code directly with async tests. Avoid arbitrary sleeps; use expectations, test clocks, streams, or timeout helpers that make completion deterministic.
- For actors and concurrent code, test concurrent access, cancellation, and failure paths, not just single-threaded success.
- Use UI tests for critical user flows and interaction contracts; use snapshot tests only when the repository already has stable snapshot tooling.
- Use performance tests for owned performance work and keep metrics focused on the changed hot path.
- Keep tests independent. Reset shared stores, keychain/user defaults, files, caches, and app state between tests using the repository's helpers.
- Name tests around behavior and condition, not implementation details. One test can contain multiple assertions when they prove one behavior.

## Verification Focus

- Run the targeted Swift test command and platform build required for the changed target.
- Cover success, invalid input, error mapping, cancellation/timeout, empty state, boundary values, and platform-specific branches touched by the task.
- For UI changes, verify at least the changed interaction path plus one error/empty/disabled state when those states exist.
- Record platform or simulator limitations if the local environment cannot run the required UI/device test.

## Evidence Notes

- Record `swift.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/swift/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the proof type: XCTest/Swift Testing unit test, async test, actor concurrency test, UI test, snapshot test, performance metric, test double, or platform limitation.
