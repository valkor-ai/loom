# Swift Memory Quality

Use this topic reference when `tech/code/swift/memory.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies ARC, ownership, and performance guidance to Swift changes.

## When To Use

- The task changes closures that capture `self`, delegates, timers, observers, tasks, caches, image/data processing, large collections, performance-sensitive paths, or memory-warning behavior.
- Use this when ARC ownership, retain cycles, value-vs-reference semantics, collection cost, or profiling evidence affects correctness or responsiveness.
- If the change is not memory/performance-sensitive, do not add speculative micro-optimizations.

## Implementation Focus

- Use weak references for delegates and parent/back references that must not keep the target alive. Use `unowned` only when the lifetime guarantee is absolute.
- Review every escaping closure, callback, notification observer, timer, Combine/async subscription, and task for retain cycles. Use capture lists deliberately and cancel/remove observers when the owner deinitializes.
- Prefer value semantics for data models and state snapshots. Use reference types when identity, shared mutation, or framework requirements are real.
- Avoid copying large values repeatedly in hot paths. Reserve collection capacity when size is known and avoid string concatenation in tight loops.
- Keep caches bounded or clearable, and define whether cached data is memory-only, disk-backed, or lifecycle-scoped.
- Do not optimize based on guesswork. For user-visible performance work, use Instruments, XCTest metrics, or a representative measurement before and after the change.
- Handle iOS memory warnings or scene lifecycle events when the task owns memory-heavy resources.
- Use `autoreleasepool` only for known Objective-C/Foundation-heavy loops where measurement or local convention supports it.

## Verification Focus

- Build and run tests for the changed path. Add tests for deallocation or cancellation when the repository has patterns for that.
- For retain-cycle risks, inspect lifecycle manually or with existing leak tests/instruments and record the result when feasible.
- For performance claims, record measured timing/memory or explicitly state that the change is structural and not benchmarked.
- Verify observers, timers, tasks, and subscriptions are cancelled or released in owner teardown.

## Evidence Notes

- Record `swift.memory` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/swift/memory.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the memory decision: weak/unowned ownership, closure capture, observer/task cleanup, value semantics, collection allocation, cache boundary, memory warning, or profiling proof.
