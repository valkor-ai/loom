# Non-Functional Requirements In Loom Architecture

Use this reference when writing architecture NFRs or assessing whether a design gives implementation a verifiable quality target.

NFRs in Loom must be concrete enough to guide implementation and verification. They are not slogans.

## Supported Categories

| Category | Good Target Shape |
|---|---|
| performance | Bounded API/list/query latency, pagination, payload size, build/start expectation. |
| scalability | Bounded growth behavior for users, records, background work, queue depth, storage, or read model size. |
| availability | Runtime surface health, graceful degradation, dependency outage behavior, recovery target, or maintenance behavior. |
| reliability | Recovery behavior, idempotency, transaction safety, retry limits, data loss boundary. |
| security | Authentication/authorization boundary, sensitive field handling, error disclosure, audit needs. |
| maintainability | Module ownership, migration readability, field mapping alignment, testable seams. |
| observability | Logs/events/errors/probes that prove critical transitions or failures. |
| cost | Avoiding unnecessary services, storage duplication, or runtime complexity in current phase. |

## Writing Rules

Each NFR must include:

- stable NFR id
- category
- target
- rationale
- architecture refs to decisions and risks when applicable
- verification strategy

Each target also needs measurement context:

- **source**: confirmed requirement or a derived minimum needed for current-phase product quality
- **source refs**: current-phase scope, acceptance, or requirement-detail evidence; a confirmed requirement must cite acceptance or requirement-detail evidence rather than only a broad scope
- **indicator**: observable value, state, or behavior used to evaluate the target
- **workload or condition**: dataset, request shape, dependency failure, lifecycle event, or operating condition under which it applies
- **evaluation boundary**: test, static analysis, review, local runtime, or another concrete boundary that can produce evidence
- **owner artifacts**: modules and interfaces whose implementation can satisfy or violate the target

Good NFR:

- category: performance
- target: list endpoints support bounded pagination and do not require loading all records
- rationale: the staff list page is a repeated operational workflow
- related decisions/risks: pagination decision and unbounded-list risk records
- verification strategy: task verification covers paginated query parameters and result metadata

Scalability NFRs should describe the current phase's growth boundary, not a generic future scale claim:

- target: list reads remain bounded by pagination and indexed filters as records grow beyond manual review size
- verification strategy: task verification covers bounded query parameters and the declared default/max page size

Availability NFRs should describe concrete runtime or dependency behavior:

- target: when a downstream approval service is unavailable, the API returns an actionable unavailable response instead of silent success
- rationale: operators need to distinguish retryable outage from business rejection
- related risk: approval dependency outage
- verification strategy: task verification covers the unavailable dependency path or documents a known gap

Weak NFR: "The system should be fast."

Do not write a numeric latency, throughput, availability, RPO, RTO, or cost target unless it is confirmed or can be justified from a concrete current-phase operating boundary. A derived minimum may specify bounded behavior without inventing a number, such as a maximum accepted page size or deterministic unavailable response.

## Missing User NFRs

When the user did not state explicit NFRs:

- Do not block the flow.
- Infer only minimum product-quality NFRs needed for current-phase delivery.
- Record them as architecture assumptions with verification strategy.
- Keep targets modest and implementation-verifiable.
- Do not invent cloud-scale targets, multi-region availability, or high-throughput assumptions when the current phase is local/internal and no requirement supports them.

## Anti-Patterns

- Writing NFRs with no verification strategy.
- Turning every best practice into a mandatory NFR.
- Creating cloud-scale NFRs for a local-first or internal product phase.
- Mixing UI quality rules into architecture NFRs; use UIX contract for UI-specific quality.
