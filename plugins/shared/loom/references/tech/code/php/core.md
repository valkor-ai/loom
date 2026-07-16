# PHP Modern PHP Quality

This file turns modern PHP language guidance into task-level implementation rules.

## When To Use

- The task changes PHP domain code, DTOs, services, controllers, CLI commands, validation, dependency wiring, enums, value objects, or Composer-managed application code.
- Use this for PHP 8.x language choices such as strict types, readonly objects, attributes, enums, match expressions, first-class callables, exceptions, and typed APIs.
- If the task only edits SQL, templates, assets, or generated config with no PHP behavior, do not expand into PHP refactoring.

## Implementation Focus

- Add `declare(strict_types=1);` to new PHP source files when the repository uses strict typing. Do not create new untyped public APIs.
- Model closed business states with backed enums when the runtime supports them and persistence/API mapping is clear. Keep behavior such as labels or transition checks near the enum when it reduces string branching.
- Use readonly classes/properties for immutable DTOs and value objects. Do not make lifecycle-managed entities readonly if the framework or ORM mutates them.
- Prefer typed DTOs/value objects at boundaries where raw arrays would leak validation assumptions. Keep array shapes documented with PHPDoc only when a framework requires arrays.
- Use `match` for exhaustive branching over known states; keep a default branch only when unknown input is a real external possibility and is handled deliberately.
- Use attributes for metadata only when the framework or repository already uses attribute-driven routing, validation, serialization, or DI. Do not duplicate the same rule in attributes and config.
- Keep controllers and command handlers thin. Put business decisions in services, domain objects, policies, or handlers that can be tested without HTTP/CLI wiring.
- Use constructor dependency injection or the repository's established container pattern. Avoid hidden globals and static service access outside framework glue.
- Throw domain-specific exceptions or return explicit result objects for business failures. Do not return `false`, mixed arrays, or magic strings from new service APIs.
- Preserve Composer autoloading, namespace conventions, PSR-12 formatting, and local static-analysis annotations instead of inventing a parallel layout.

## Boundary Decisions

- Treat `declare(strict_types=1)` as a file-level boundary, not a substitute for validating external input. Normalize request, queue, and decoded JSON data before passing it to typed domain APIs.
- Use a backed enum when the state has a stable storage/API representation. Keep unknown external values on an explicit error or unmapped path instead of silently coercing them to a valid case.
- Use readonly DTOs/value objects for data that should not change after construction. Keep ORM entities, proxy-managed objects, and framework lifecycle objects mutable when their integration requires it.
- Use attributes only when the active framework consumes them. Choose one source of truth between attributes, configuration, serializer groups, and validation metadata.
- Keep `mixed`, array shapes, and PHPDoc generics at integration boundaries where PHP cannot express the contract. Do not let them leak through service APIs without a documented reason.
- Name the Composer namespace and autoload path from repository facts. Do not copy an `App\\` namespace or demo package layout into an existing project with a different convention.

## Verification Focus

- Run the repository's PHP test command: PHPUnit, Pest, or framework feature tests.
- Run PHPStan or Psalm when configured, especially after adding generics, array shapes, DTOs, enums, or service interfaces.
- Test invalid input, impossible state, enum mapping, exception/result behavior, and one successful business path touched by the task.
- If a framework route or command is changed, verify the real entry point, not only the service in isolation.

## Evidence Focus

- In the evidence summary, name the PHP decision: strict typing, DTO/value object, enum, readonly immutability, attribute metadata, DI boundary, domain exception, or static-analysis proof.
