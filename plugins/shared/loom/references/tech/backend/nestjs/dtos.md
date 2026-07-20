# NestJS DTOs, Validation, And Serialization

DTOs define the HTTP transport shape. They validate and transform untrusted transport values into an application-call boundary and shape safe responses; they are not persistence entities or a substitute for domain invariants.

## Separate DTO Responsibilities

Use distinct DTOs when the contracts differ:

- create/input DTO for client-writable fields
- update DTO for patch/replace semantics
- query DTO for filters, ordering, search, and pagination
- response DTO for public fields and representation-specific values
- event or external-adapter DTO when a non-HTTP contract is separately owned

Do not reuse ORM entities as request or response DTOs. Persistence nullability, relations, decorators, generated columns, and internal metadata are not an API contract.

`PartialType`, `PickType`, `OmitType`, and `IntersectionType` are useful only when the derived validation and schema remain correct. A PATCH DTO must still reject immutable/server-owned fields, and PUT replacement semantics should not be modeled as an indiscriminate partial type.

## Validation Semantics

Express accepted field rules with `class-validator`: presence, string length, numeric range, enum membership, UUID/date/email format, array cardinality, and nested structure. Distinguish missing, `null`, empty string, and false/zero according to the interface contract.

For nested objects and arrays, combine `@ValidateNested` with `@Type(() => ChildDto)` and use `{ each: true }` where required. Validation decorators on a TypeScript interface do nothing; runtime validation requires classes and emitted metadata.

Keep database uniqueness, ownership, lifecycle transitions, and cross-record checks in the application/persistence boundary. Async custom validators that query storage can create hidden N+1 work and race conditions; use them only for transport-local lookups where their limitations are accepted.

## Transformation And Query Values

HTTP query and path values arrive as strings. Use explicit `@Type`, focused `@Transform`, or parse pipes for numbers, booleans, dates, enums, and arrays. Avoid broad implicit conversion when values such as `"false"`, empty strings, repeated query keys, or invalid dates could be misinterpreted.

Transforms must be deterministic, side-effect free, and ordered with validation intentionally. Do not normalize credentials, opaque identifiers, signatures, or case-sensitive values. Trim/case-normalize only fields whose contract declares that behavior.

Bound page size, offset, sort fields, and filter operators. Convert client ordering keys through an allowlist rather than passing arbitrary strings to an ORM.

## Global ValidationPipe Contract

Preserve the application's accepted `ValidationPipe` settings. A production-oriented baseline usually considers:

```typescript
new ValidationPipe({
  whitelist: true,
  forbidNonWhitelisted: true,
  transform: true,
  validationError: { target: false, value: false },
})
```

These settings change public behavior and must match E2E tests and the error envelope. `whitelist` silently strips only when `forbidNonWhitelisted` is false. `transform` does not make every implicit conversion safe.

If per-route pipes differ from the global pipe, document the intentional contract difference in code and test the actual route.

## Response Serialization

Map application results to explicit response DTOs or the established serializer/interceptor. Exclude secrets, password hashes, refresh tokens, internal authorization attributes, tenant internals, soft-delete markers, and provider-only columns.

Be deliberate about date, decimal, bigint, enum, and nullable serialization. `JSON.stringify` cannot serialize `bigint` directly, and ORM decimal objects may not match the accepted JSON number/string representation.

Avoid relying on `class-transformer` exclusion decorators unless the real response path invokes serialization. Returning a plain object or using `@Res()` can bypass the expected transform behavior.

## OpenAPI Consistency

When OpenAPI is published, ensure required/optional status, enum values, nested arrays, formats, defaults, examples, and response types match runtime validation. Import mapped types from the repository's established package (`@nestjs/swagger` or `@nestjs/mapped-types`) so both runtime metadata and generated schemas behave as intended.

## Verification

- Test valid input and each changed boundary: missing, null, malformed, out-of-range, unknown, nested, and array cases.
- Prove PATCH/PUT behavior for omitted, explicit-null, immutable, and server-owned fields.
- Exercise query conversion and allowlists through the real global `ValidationPipe`.
- Assert the exact validation envelope without exposing rejected values or internal targets.
- Verify sensitive-field exclusion and date/decimal/bigint representation on the real response path.
- Compare generated OpenAPI only when the published schema is task-owned.

## Delivery Evidence

Identify the input/query/response DTO and the runtime HTTP assertion proving validation, transformation, or serialization. TypeScript compilation and decorator presence alone do not prove that the global pipe or response serializer executes.

## Unsafe Defaults

- One DTO reused for create, update, persistence, and response.
- `PartialType` allowing immutable or server-owned fields.
- Implicit conversion relied on for booleans, arrays, or dates without HTTP tests.
- Database checks hidden in reusable validators.
- ORM entities serialized directly.
- Sensitive-field exclusion assumed without exercising the response path.
