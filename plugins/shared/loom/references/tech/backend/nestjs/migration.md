# Express To NestJS Migration

Use NestJS as the target implementation while preserving the accepted Express behavior until an intentional contract change is separately approved. Migration is a compatibility operation, not permission to redesign routes, errors, auth, persistence, or deployment conventions.

## Establish The Parity Baseline

Inventory behavior before moving code:

- mounted router prefixes and effective method/path pairs
- path/query/body parsing and defaults
- validation rules and error envelope
- middleware order and short-circuit behavior
- authentication, authorization, ownership, and public exceptions
- success/error statuses, headers, cookies, redirects, and response shape
- transaction, external effect, event, and background-work ordering
- configuration keys, startup behavior, health/readiness, and shutdown
- tests that currently prove these behaviors

Create a route-by-route parity matrix with old entry point, Nest target, required behavior, verification, and intentional difference. Do not infer the contract only from Express handler bodies; include router mounting and application middleware.

## Map Responsibilities Deliberately

| Express responsibility | NestJS target |
|---|---|
| `express.Router` and handler binding | Controller/module |
| Primitive request parsing | Param/query pipes |
| Structured input validation | DTO plus `ValidationPipe` |
| Authentication gate | Strategy/guard |
| Operation/role authorization | Guard/policy plus service/query scoping |
| Request/response cross-cutting behavior | Interceptor or middleware |
| Error middleware | Exception filter/mapping boundary |
| Manually constructed service/client | Registered provider and injection token |
| Process bootstrap/config | Nest bootstrap/config module |

Do not mechanically convert every Express middleware into Nest middleware. Select a guard, pipe, interceptor, filter, or provider according to responsibility and lifecycle.

## Create A Coexistence Boundary

For incremental migration, choose an explicit routing owner for each path. Express and Nest must not both handle the same route accidentally, and proxy prefixes must not double-strip or duplicate `/api`.

Share contracts and stable ports where practical, but do not let Nest providers import Express request/response objects through the application layer. If both runtimes coexist, define how identity, correlation, errors, config, database transactions, and shutdown propagate across the boundary.

Prefer vertical route/capability slices that can be verified and switched independently. Avoid a long-lived half-conversion where controllers use Nest while services remain hidden singletons constructed by Express modules.

## Convert Modules And Dependency Injection

Create cohesive feature modules and register controllers/providers with explicit imports and exports. Replace module-level singletons and `new` construction with constructor injection.

Use stable provider tokens for repositories/external clients and preserve their lifecycle. Validate configuration before provider construction. Resolve dependency cycles by changing ownership; do not normalize `forwardRef` as the migration architecture.

When moving persistence code, preserve the selected ORM/client and migration history. NestJS does not require TypeORM. Do not rewrite storage while migrating the web framework unless the task explicitly owns both changes.

## Preserve HTTP And Error Behavior

Verify the effective route after global prefix and controller composition. Keep status, headers, cookies, redirects, pagination, and serialized fields compatible.

Configure global pipes, filters, guards, and interceptors through one production bootstrap path and reuse it in HTTP tests. Default Nest validation/error payloads may differ from Express; map them deliberately when compatibility is required.

Preserve trusted proxy, CORS, CSRF, body-size, raw-body/webhook, multipart, compression, and rate-limit behavior that applies to migrated routes. Adapter differences between Express and Fastify must be explicit.

## Preserve Security And Side Effects

Map auth middleware to the selected strategy/guard and reproduce public exceptions, role/permission checks, owner/tenant scoping, and wrong-resource disclosure behavior.

Preserve transaction and side-effect order. An Express handler that commits then emits an event cannot be migrated to a provider that emits inside the transaction without an accepted change. Define retry/idempotency behavior for migrated callbacks and jobs.

Never carry over plaintext secrets, ad hoc token parsing, or error-detail leakage merely for parity; record required security corrections as explicit migration changes with tests.

## Cutover And Removal

Before switching a route group, run parity checks against both implementations or captured contract fixtures. Confirm traffic routing, configuration, health/readiness, metrics/log correlation, and rollback behavior.

After cutover, remove obsolete routers, middleware, manual constructors, duplicate config, tests, and dependencies in the owned scope. Leaving two implementations active is not a completed migration.

## Verification

- Prove success and each owned failure/auth branch for every migrated route group.
- Compare exact status, body, headers, validation, and error behavior where parity is required.
- Compile the production-representative Nest module and exercise global bootstrap through HTTP.
- Verify provider injection, database durability/rollback, external-effect ordering, and shutdown behavior when touched.
- Test the coexistence/proxy path so prefix ownership is unambiguous.
- Record every intentional difference with its accepted requirement and evidence.

## Delivery Evidence

Reference the parity-matrix entry and identify old/new requests or contract assertions proving equivalence. Nest compilation, generated files, or one successful route cannot prove middleware, security, error, persistence, or cutover parity.

## Unsafe Defaults

- Migration activated from prose instead of a structured task action.
- Endpoint count or team size used to choose architecture.
- Every Express middleware translated into Nest middleware.
- TypeORM introduced because a Nest example uses it.
- Express and Nest both owning the same route during cutover.
- Default Nest error/validation payloads assumed compatible.
- Old routers and duplicate configuration left active after acceptance.
