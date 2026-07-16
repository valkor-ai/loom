# Swift Testing Quality

This file applies Swift testing guidance to task-owned changes.

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

## Decision Rules

- Select the narrowest proof: XCTest/Swift Testing for pure logic, async/actor tests for concurrency, UI tests for critical interactions, and performance tests for an owned hot path.
- Inject protocols, URL protocols, clocks, storage adapters, and fakes at boundaries so tests do not depend on real networks, device state, keychain, user defaults, or global stores.
- Test async success, failure, cancellation, timeout, actor isolation, and stream termination without arbitrary sleeps. Use expectations, test clocks, streams, or deterministic timeout helpers.
- Keep UI assertions on user-visible behavior and include error/empty/disabled states when they exist. Snapshot tests are supplementary unless the repository treats them as an established contract.
- Reset files, caches, stores, user defaults, keychain, and app state between tests. Name tests after behavior and condition rather than implementation details.
- Report changed-branch and platform evidence instead of imposing a universal coverage percentage; record simulator/device limitations when they prevent the required test.

## Verification Focus

- Run the targeted Swift test command and platform build required for the changed target.
- Cover success, invalid input, error mapping, cancellation/timeout, empty state, boundary values, and platform-specific branches touched by the task.
- For UI changes, verify at least the changed interaction path plus one error/empty/disabled state when those states exist.
- Record platform or simulator limitations if the local environment cannot run the required UI/device test.

## Evidence Focus

- In the evidence summary, name the proof type: XCTest/Swift Testing unit test, async test, actor concurrency test, UI test, snapshot test, performance metric, test double, or platform limitation.
