# Django REST Framework Serializer Quality

Use this topic reference when `tech/backend/django/serializers.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md` and selected API references. This file applies DRF serializer rules to task-owned request validation and response shaping.

## When To Use

- The task changes DRF serializers, input validation, nested serializer behavior, read/write field split, computed fields, create/update overrides, or representation shape.
- Use this when user input, API output, or model-to-DTO mapping affects correctness.
- If the task only changes Django models or raw views with no DRF serialization boundary, do not load this serializer reference.

## Implementation Focus

- Keep request and response intent explicit. Use separate serializers when create/update input differs from read output or when sensitive/internal fields must never serialize.
- Use `read_only_fields`, write-only fields, `source`, and related-field serializers deliberately. Do not expose model internals just because `ModelSerializer` can infer them.
- Put syntax and cross-field input validation in `validate_<field>` and `validate`; keep business invariants that must hold outside the API in the domain/service/model layer.
- For nested writes, make create/update ownership explicit and transactional. Do not silently replace related rows unless the API contract says so.
- Treat `SerializerMethodField` as a computed read model, not a place for unbounded database work. Pair it with queryset optimization or precomputed annotations.
- Use serializer context for request/user-dependent behavior instead of global state.
- Keep partial update behavior intentional: `required`, defaults, omitted fields, and `partial=True` must match the API contract.

## Verification Focus

- Test valid input, invalid field input, cross-field validation, partial update, nested input, and sensitive field exclusion when touched.
- Verify response shape for list/detail serializers, including read-only/computed fields and related-object representation.
- For serializer-driven create/update overrides, assert database side effects and transaction behavior.
- Pair serializer tests with view/API tests when status codes or permission behavior depend on serializer errors.

## Evidence Notes

- Record `django.serializers` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/django/serializers.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the serializer decision: read/write split, field validation, object validation, nested write, computed field, partial update, or sensitive-field exclusion.
