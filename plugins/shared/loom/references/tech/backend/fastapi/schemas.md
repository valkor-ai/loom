# FastAPI Pydantic Schema Quality

This file applies Pydantic V2 and FastAPI schema rules to task-owned request and response models.

## When To Use

- The task changes Pydantic models, request validation, response models, query/path parameter types, settings models, serialization aliases, computed fields, or OpenAPI schema shape.
- Use this when validation, input/output separation, or generated API documentation affects correctness.
- If the task only changes internal database code with no API schema surface, do not load this schemas reference.

## Implementation Focus

- Use Pydantic V2 patterns: `field_validator`, `model_validator`, `model_config`, `from_attributes`, `model_dump`, and `model_validate`.
- Use typed create/read/update schemas instead of one broad model for every direction. Keep sensitive fields excluded from responses.
- Express constraints with `Field`, `Annotated`, enum types, and explicit optionality. Prefer `X | None` for nullable values.
- Keep business validation that depends on database state or ownership in service/dependency code, not only inside Pydantic validators.
- Use `from_attributes` only for response schemas that intentionally serialize ORM objects.
- Keep aliases and serialization names aligned with the accepted API contract; do not silently change casing or envelope shape.
- Use Pydantic settings for runtime configuration when the task owns configuration shape.

## Verification Focus

- Test valid payloads, invalid field values, cross-field validation, omitted optional fields, partial update schemas, and sensitive field exclusion.
- Verify response models for ORM-backed output and generated OpenAPI schema when the public contract changes.
- Test settings binding defaults and invalid configuration when settings models are added or changed.
- Pair schema tests with endpoint tests when HTTP status codes or error payloads depend on validation.

## Evidence Focus

- In the evidence summary, name the Pydantic decision: input/read/update split, validator, field constraint, `from_attributes`, aliasing, settings model, sensitive-field exclusion, or OpenAPI schema proof.
