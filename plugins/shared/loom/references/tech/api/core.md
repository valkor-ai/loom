# Loom API Contract Core

Load this file only when the current MCP request selects `techReferenceProfile.groups.api` item `core`.

API work in Loom is not a standalone specification exercise. It is a task-verifiable contract that connects Architecture interface design, TaskPlan assignment, Execution implementation, TaskResult evidence, Review routing, and later deploy/runtime probes.

## Operating Model

1. Read the current MCP request groups first. The MCP request is the source of truth.
2. Load only API reference files selected by `techReferenceProfile.groups.api`.
3. Model only current-phase APIs that are required by confirmed scope, frontend bindings, business workflows, runtime probes, or integration boundaries.
4. Express API decisions through AAC `interfaces[]` and downstream `apiContractRequirements`; do not paste reference text into Loom JSON artifacts.
5. Keep API quality evidence concrete: changed files, endpoint paths, status/error behavior, request/response DTOs, tests, runtime probes, or contract files.

## Required API Contract Assets

| Asset | Purpose | Later Consumer |
|---|---|---|
| Interface record | Names API ownership, method, path, resource, operation kind, schemas, status codes, and refs. | TaskPlan write boundary and Execution projection. |
| Request model | Describes accepted body/query/path fields and validation. | Implementation and tests. |
| Response model | Describes success body shape and readback fields. | Frontend binding, runtime probes, Review. |
| Error model | Describes business, validation, auth, conflict, and not-found responses. | UI feedback and TaskResult evidence. |
| Pagination/filtering policy | Defines bounded collection behavior when collection endpoints are unbounded. | API task verification and performance NFRs. |
| Auth policy | States actor/permission requirements when current scope includes protected operations. | Security-sensitive implementation and Review. |
| Evolution policy | Captures compatibility constraints only when existing/public clients or explicit versioning requirements exist. | Architecture risk and API repair. |

## Contract Discipline

- Prefer `type: "http_api"` for HTTP endpoints and keep `interfaceId` compact.
- Use resource-oriented paths for REST APIs. Do not use command names as paths unless the domain operation is genuinely command-like and cannot be represented as a resource state transition.
- For every write API, specify request validation, success status, business-blocking errors, and state/readback proof.
- For every collection API, declare pagination/filtering only when the collection can grow beyond a bounded current-phase dataset.
- Do not require OpenAPI files unless selected by the contract file reference or explicitly required by user/repo context.
- Do not invent `/v1` or deprecation policy by default.

## Minimum Quality Bar

A usable API contract lets a later agent answer:

- Which endpoint or service method should this task implement?
- Which request fields are accepted and validated?
- Which response fields prove the business result?
- Which business errors must be actionable for the UI/client?
- Which status codes are expected for success, validation, conflict, missing resource, auth, and unexpected failure?
- Which verification evidence proves this endpoint is not a mock or silent failure?

If those answers are missing, repair Architecture or TaskPlan instead of leaving Execution to guess.
