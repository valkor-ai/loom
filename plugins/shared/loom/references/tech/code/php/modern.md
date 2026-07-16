# Modern PHP Feature Quality

## When To Use

- The task intentionally adopts or changes PHP language features such as strict typing, enums, readonly classes, attributes, first-class callables, `match`, `never`, fibers, or typed properties.
- Load this only for a task that owns the language-version or feature decision. Core PHP guidance remains the baseline for ordinary application code.

## Implementation Focus

- Confirm the repository's PHP version, Composer platform constraint, runtime images, and supported static-analysis version before using a feature. Do not infer support from the local interpreter alone.
- Add `declare(strict_types=1);` consistently to new PHP files when the repository uses it, and validate data crossing weakly typed boundaries before the strict API is called.
- Use backed enums for finite values that have stable storage or transport representations. Define unknown-value behavior and avoid serializing enum labels when the contract requires enum values.
- Use readonly classes or properties for immutable DTOs/value objects. Do not mark Doctrine/Eloquent entities or proxy-managed framework objects readonly without verifying hydration, mutation, and serialization behavior.
- Use attributes only where the active framework reads them. Keep routing, validation, serialization, and DI metadata in one authoritative representation.
- Use first-class callables and `match` when they make dispatch or exhaustive branching clearer. Keep exceptions and default/unknown cases explicit at external boundaries.
- Use `never` only for functions that truly cannot return, such as a typed terminator or exception boundary. Do not use it to hide an incomplete result path.
- Treat Fibers as a low-level primitive. Load the async reference for scheduling, I/O, cancellation, and lifecycle; this reference alone does not make code concurrent.

## Verification Focus

- Run the repository's configured PHP test and static-analysis commands for the changed module, plus the narrowest runtime check for the selected PHP version.
- Verify enum persistence/serialization, readonly hydration, attribute discovery, callable dispatch, exhaustive `match`, and unknown-input behavior when touched.
- Check Composer autoloading and the supported runtime/container path, not just syntax parsing with a newer local PHP binary.

## Evidence Focus

- In the evidence summary, name the feature decision, supported PHP/Composer constraint, integration boundary, and behavior tested.

## Failure Modes

- Do not copy PHP 8.3 syntax into a project whose Composer/runtime constraints are older.
- Do not use readonly, attributes, enums, or Fibers as decoration; each must solve an owned contract or lifecycle problem.
- Do not treat PHPStan/Psalm suppression, `mixed`, or a passing syntax check as proof of runtime compatibility.
