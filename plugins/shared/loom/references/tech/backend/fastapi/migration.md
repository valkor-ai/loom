# Django And DRF To FastAPI Migration

This reference applies only to an explicitly owned framework migration. Preserve accepted behavior and data semantics before introducing FastAPI-native structure; migration is not permission to redesign the product contract.

## When To Use

Use this reference when a task explicitly ports Django/DRF serializers, views/viewsets, permissions, ORM access, middleware, signals, tests, or operational commands to FastAPI. Ordinary FastAPI feature work does not need migration guidance.

## Implementation Focus

### Build A Parity Inventory

Inventory the source behavior before changing code:

- route method/path/name and trailing-slash behavior
- request fields, defaults, read-only/write-only fields, and validation
- response envelope, fields, ordering, pagination, and status codes
- authentication, permissions, object ownership, and existence disclosure
- filters, search, sorting, queryset scoping, and eager loading
- transactions, constraints, signals, audit behavior, and side effects
- middleware, throttling, exception shape, file handling, and content types
- source tests and production defect behavior that must remain fixed

Classify each difference as required parity, accepted change, deferred capability, or blocker. Do not silently normalize behavior to a preferred FastAPI style.

### Concept Mapping

| Django/DRF | FastAPI Boundary |
|---|---|
| Serializer | Pydantic input/output models plus service validation |
| ModelSerializer create/update | Explicit application operation and SQLAlchemy/repository mapping |
| ViewSet action | `APIRouter` endpoint with explicit method/path/dependencies |
| Permission class | Authentication dependency plus permission/ownership service |
| `get_queryset()` scoping | Task-owned query component receiving current actor/filter policy |
| `select_related`/`prefetch_related` | Explicit SQLAlchemy loader/projection strategy |
| Middleware | FastAPI/ASGI middleware or focused dependency according to scope |
| Signal | Explicit domain/application event with transaction timing defined |
| Management command | Repository-standard CLI/job entry point with shared application service |

The mapping is semantic, not mechanical. Preserve what the source component guaranteed, then place the responsibility in the correct target boundary.

### Serializer To Pydantic Models

Separate create, replace, patch, and read models. Recreate field defaults, aliases, enum/date/decimal behavior, nested validation, read-only/write-only semantics, and sensitive-field exclusion.

DRF validators that query the database or depend on request identity move to an application service or dependency. `SerializerMethodField` behavior becomes an explicit response field/read model only when query cost and loading are controlled.

### ViewSet To Router And Service

Map every standard and custom action to its accepted method/path/status. Keep router functions thin and make transaction ownership explicit in the application operation.

Do not convert every ViewSet hook into a dependency. Query scoping, mutation rules, and side effects usually belong in query/application services. Preserve idempotency and conflict behavior for custom state-transition actions.

### ORM And Data Semantics

Do not rewrite query behavior by syntax alone. Preserve:

- table and column names while sharing or transitioning an existing schema
- identifiers, sequences, enums, decimals, timestamps, nullability, and defaults
- foreign keys, unique/check constraints, cascades, and delete behavior
- transaction/locking behavior and integrity-error translation
- query scoping, annotations/aggregates, deterministic ordering, and pagination
- `select_related`/`prefetch_related` intent through explicit loading or projection

Async SQLAlchemy cannot perform hidden lazy I/O safely during serialization. Load required relationships inside the query/session boundary. Keep Django and target migrations coordinated when both runtimes write the same schema.

### Authentication And Permissions

Do not replace a mature session, JWT, OAuth, or identity-provider contract with a tutorial token implementation. Preserve token/session format, issuer/audience, claims, expiry, refresh/revocation behavior, permission semantics, and `401`/`403`/not-found disclosure.

Object-level permissions need the actor and target resource. A global role dependency is not an equivalent replacement.

### Middleware, Signals, And Side Effects

List source middleware and signals explicitly. Move request-scoped concerns to ASGI middleware or dependencies, and move business side effects to application operations/events. Preserve transaction timing: a Django post-save signal does not automatically translate to safe after-commit or durable delivery.

Admin, templates, forms, uploads, background workers, and management commands are separate surfaces. Retain, replace, or defer them explicitly; do not assume API endpoint parity covers them.

### Incremental Replacement

When old and new implementations coexist, define route/data ownership and one source of truth for each operation. Avoid dual writes without idempotency, reconciliation, and failure handling. Shared database access requires compatible migrations, transaction assumptions, and audit identity.

Cut over by behavior slice only after parity tests pass and rollback/ownership are clear. Do not remove source paths, permissions, jobs, or admin workflows merely because the new API serves happy-path requests.

## Verification Focus

- Build source-versus-target contract tests for success and meaningful failure paths.
- Compare validation errors, statuses, response fields, pagination/filtering/sorting, and trailing-slash behavior.
- Prove authentication, broad permission, object ownership, and protected-resource disclosure parity.
- Verify query results, relationship loading, constraints, transactions, and side-effect timing against representative persisted data.
- Preserve source regression tests or equivalent target tests for known defects.
- Record and test every accepted behavior difference rather than hiding it in implementation details.

## Evidence Focus

Identify the source component, target component, parity obligation, accepted difference, and test proving the result. A route-by-route code translation or target-only happy-path suite does not demonstrate migration completeness.

## Unsafe Defaults

- Treating DRF serializers as direct Pydantic model translations without service rules.
- Replacing object permissions with one role check.
- Loading relationships lazily during async response serialization.
- Running Django and FastAPI writes against one schema without migration ownership.
- Recreating source signals as unbounded in-process background tasks.
- Redesigning paths, statuses, validation, or error envelopes during an unapproved migration.
- Removing admin, command, worker, upload, or middleware behavior without explicit disposition.
