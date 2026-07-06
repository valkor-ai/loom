# NestJS DTO And Validation Quality

Use this topic reference when `tech/backend/nestjs/dtos.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md` and selected TypeScript type references. This file applies NestJS DTO, class-validator, class-transformer, and mapped-type rules to task-owned input/output models.

## When To Use

- The task changes DTO classes, validation decorators, transformation, query DTOs, nested validation, mapped types, response DTOs, or ValidationPipe behavior.
- Use this when input validation, sanitization, or API schema shape affects correctness.
- If the task only changes internal services with no input/output model change, do not load this DTO reference.

## Implementation Focus

- Define create/update/query DTOs explicitly. Use `PartialType`, `PickType`, and `OmitType` only when they preserve the contract clearly.
- Use class-validator decorators for required fields, formats, enum values, arrays, nested objects, numeric bounds, and string lengths.
- Use class-transformer `@Type` and `@Transform` deliberately for query parameters and nested objects. Do not rely on implicit JavaScript coercion.
- Enable or respect global ValidationPipe settings such as whitelist, forbidNonWhitelisted, and transform.
- Keep sensitive fields out of response DTOs and Swagger examples.
- Put database-dependent or ownership validation in services/guards, not only DTO decorators.
- Keep custom validators focused and testable when built-in decorators are insufficient.

## Verification Focus

- Test valid input, invalid field input, unknown property stripping/rejection, nested validation, query transformation, partial update, and sensitive field exclusion when touched.
- Verify global ValidationPipe behavior in E2E tests when endpoint behavior depends on it.
- Verify Swagger schema for DTO changes that affect the public contract.
- Run lint/typecheck so DTO mapped types and decorators compile correctly.

## Evidence Notes

- Record `nestjs.dtos` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/nestjs/dtos.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the DTO decision: validation decorator, mapped type, transform, nested validation, query DTO, response DTO, ValidationPipe behavior, or schema proof.
