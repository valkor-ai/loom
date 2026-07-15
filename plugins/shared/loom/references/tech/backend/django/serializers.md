# Django REST Framework Serializers

DRF serializers define API input validation and response representation. They do not own cross-entry-point business invariants, authorization, transaction policy, or unbounded database work.

## When To Use

Use this reference for DRF request/response serializers, `ModelSerializer`, field/object validation, nested representation or writes, partial updates, related fields, computed output, and serializer-driven create/update behavior.

## Implementation Focus

### Separate Directional Contracts

Use separate create, update/patch, list, and detail serializers when fields, trust, cost, or representation differ. Do not expose every model field through `fields = "__all__"` on a public API.

```python
class OrderCreateSerializer(serializers.Serializer):
    supplier_name = serializers.CharField(max_length=160, trim_whitespace=True)
    lines = OrderLineCreateSerializer(many=True, allow_empty=False)


class OrderReadSerializer(serializers.ModelSerializer):
    requester_name = serializers.CharField(source="requester.username", read_only=True)

    class Meta:
        model = Order
        fields = ["id", "supplier_name", "status", "requester_name", "requested_at"]
        read_only_fields = fields
```

Mark server-owned, audit, identity, state, and sensitive values explicitly read-only or omit them. Mark secrets write-only and ensure they are not retained in `validated_data` longer than required.

### Validation Ownership

Use `validate_<field>` for local field rules and `validate` for deterministic cross-field request rules. Database uniqueness, actor ownership, lifecycle eligibility, inventory, and multi-record invariants must also be enforced in the model/service/transaction boundary.

Serializer validation that queries the database can race and can create N+1 behavior in list/nested operations. Use it only for bounded feedback while preserving the authoritative constraint/write check.

Keep error codes/field placement aligned with the accepted API error contract. Do not return raw model/database exceptions from `create` or `update`.

### Related Fields And Representation

Choose IDs, slugs, hyperlinks, nested objects, or side-loaded data from the accepted response contract. Avoid implicit deep nesting and unrestricted reverse relationships.

`source="relation.field"` and nested serializers require matching queryset loading. A serializer must not be the first place where query planning is discovered. `SerializerMethodField` must remain deterministic, bounded, and free from per-row database queries.

### Create, Update, And Nested Writes

Keep simple model construction in `ModelSerializer` only when it preserves business and transaction behavior. Move multi-model workflows and state transitions to an application service.

Nested writes need explicit ownership, matching rules, create/update/delete semantics, and `transaction.atomic()`. Do not delete and recreate all children on every patch unless replacement is the accepted contract.

For partial updates, distinguish omitted fields from explicit null/empty values. `partial=True` relaxes required-field validation; it does not define business patch semantics automatically.

### Serializer Context

Use serializer context for request/actor, URL generation, locale, or already-computed values. Do not access module-global request state. Keep actor-dependent output consistent with authorization and queryset scoping.

### Performance And Pagination

Use lighter list serializers or annotated/projection fields for tables and summaries. Avoid serializing huge querysets or file/blob fields by default. Pagination belongs at the view/query boundary before serializer evaluation.

## Verification Focus

- Test valid and invalid field/cross-field input with exact error structure.
- Prove create, full update, partial update, explicit null, omission, and immutable/server-owned fields.
- Assert sensitive/write-only field exclusion and stable list/detail response shape.
- Test nested write ownership, rollback, update matching, and removal semantics when owned.
- Pair computed/related serializers with query-count or loader evidence.
- Exercise serializer behavior through API tests when status or error envelopes matter.

## Evidence Focus

Identify the directional serializer, validation rule, nested-write policy, or representation and the assertion that proves wire and persistence behavior. Serializer `.is_valid()` alone does not prove authorization, transaction, query count, or endpoint errors.

## Unsafe Defaults

- Public `ModelSerializer` with `fields = "__all__"`.
- One serializer reused for create, patch, list, and detail despite different contracts.
- Authorization or durable business invariants enforced only by serializer validation.
- `SerializerMethodField` issuing per-object queries.
- Nested updates that silently delete and recreate related records.
- Request/user access through global state.
- Sensitive or server-owned model fields exposed by inference.
