# FastAPI Pydantic V2 Contract Models

Use Pydantic models as explicit transport and configuration contracts. They validate and serialize data; they do not replace domain rules, persistence models, or authorization checks.

## When To Use

Use this reference when a task owns FastAPI request/response models, parameter models, Pydantic V2 validators, serialization aliases, ORM-backed response mapping, settings models, or generated JSON Schema/OpenAPI shape.

## Implementation Focus

### Separate Input, Patch, And Output Models

Model direction and trust explicitly. Create, replace, patch, internal command, and response shapes often differ. Never reuse a database entity or one broad model merely to avoid mapping code.

```python
class OrderCreate(BaseModel):
    supplier_name: Annotated[str, Field(min_length=1, max_length=160)]
    requested_at: datetime
    lines: Annotated[list[OrderLineCreate], Field(min_length=1)]


class OrderPatch(BaseModel):
    supplier_name: Annotated[str, Field(min_length=1, max_length=160)] | None = None


class OrderRead(BaseModel):
    model_config = ConfigDict(from_attributes=True)

    id: UUID
    supplier_name: str
    status: OrderStatus
    requested_at: datetime
```

For patch commands, `None`, omission, and explicit clearing can be different operations. Use `model_fields_set` and `model_dump(exclude_unset=True)` only when the accepted update semantics distinguish them.

### Pydantic V2 APIs

Use `field_validator`, `model_validator`, `ConfigDict`, `model_validate`, and `model_dump`. Do not mix V1 `@validator`, `@root_validator`, `class Config`, `.dict()`, or `orm_mode` into a V2 codebase.

Field validators handle local normalization and constraints. Model validators handle deterministic cross-field shape rules. Validators must not query databases, call external services, read request identity, or perform writes; those checks belong to application services or dependencies.

```python
class DateWindow(BaseModel):
    starts_at: datetime
    ends_at: datetime

    @model_validator(mode="after")
    def end_follows_start(self) -> Self:
        if self.ends_at <= self.starts_at:
            raise ValueError("ends_at must be after starts_at")
        return self
```

### Optionality, Defaults, And Types

`T | None` means the value can be null; a default determines whether it may be omitted. Do not make fields optional to silence validation failures. Use constrained types, enums, `Decimal`, UUID, timezone-aware datetime policy, URLs, and domain-specific value models where their serialization is defined.

Use `default_factory` for mutable collections. Keep monetary values out of binary floating-point models unless the accepted contract explicitly uses them.

### Serialization And ORM Mapping

Use `from_attributes=True` only on response models intentionally created from objects. Ensure required relationships are loaded before serialization; Pydantic must not trigger hidden async lazy loads after the session boundary.

Exclude credentials, hashes, tokens, internal flags, provider payloads, and unrestricted relationship graphs from output models. `repr=False` is not a serialization security boundary; control actual fields and serializers.

Aliases must match the accepted wire contract. Use `validation_alias` and `serialization_alias` deliberately when input and output names differ, and preserve repository-wide casing conventions.

### Custom Serialization And Computed Fields

Use field/model serializers for stable wire conversion, not business decisions. Computed fields belong in a response model only when their cost is bounded and their dependencies are already loaded. Avoid dynamic fields that change OpenAPI shape by runtime condition.

### Settings Models

Use `BaseSettings` and `SettingsConfigDict` for typed runtime configuration. Keep environment names, prefixes, case sensitivity, nested delimiters, and required values explicit. Do not instantiate settings repeatedly in dependencies or commit secret values as defaults.

## Verification Focus

- Validate accepted and rejected create/replace/patch payloads.
- Prove omission, explicit null, default, alias, enum, datetime, decimal, and nested-list behavior when owned.
- Test sensitive-field exclusion and ORM-backed `model_validate` with required relationships loaded.
- Assert stable serialized output with `model_dump` rather than only model construction.
- Inspect generated OpenAPI/JSON Schema for changed public models.
- Test settings defaults, required values, and invalid configuration independently of process-global state.

## Evidence Focus

Name the model direction, validator, serialization rule, alias, or settings boundary and show the assertion that proves the wire shape. Model construction alone does not prove endpoint status, error envelope, relationship loading, or sensitive-field exclusion.

## Unsafe Defaults

- One model for create, patch, persistence, and response.
- Pydantic V1 syntax in a V2 project.
- Database or network calls from validators.
- Optional fields added solely to make invalid payloads pass.
- Serializing SQLAlchemy entities or lazy relationships without an explicit response model.
- Password, token, or provider-secret fields retained in response models.
- Hardcoded environment-specific URLs, origins, or secrets in settings defaults.
