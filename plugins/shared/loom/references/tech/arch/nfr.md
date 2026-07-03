# Non-Functional Requirements In Loom Architecture

Use this reference when writing `architectureQuality.nfrs` or reviewing whether an architecture gives later tasks verifiable quality targets.

NFRs in Loom must be concrete enough for TaskPlan and Review. They are not slogans.

## Supported Categories

| Category | Good Target Shape |
|---|---|
| performance | Bounded API/list/query latency, pagination, payload size, build/start expectation. |
| reliability | Recovery behavior, idempotency, transaction safety, retry limits, data loss boundary. |
| security | Authentication/authorization boundary, sensitive field handling, error disclosure, audit needs. |
| maintainability | Module ownership, migration readability, field mapping alignment, testable seams. |
| observability | Logs/events/errors/probes that prove critical transitions or failures. |
| cost | Avoiding unnecessary services, storage duplication, or runtime complexity in current phase. |

## Writing Rules

Each NFR must include:

- `nfrId`
- category
- target
- rationale
- architecture refs to decisions and risks when applicable
- verification strategy

Good NFR:

```json
{
  "nfrId": "nfr-query-pagination",
  "category": "performance",
  "target": "List endpoints must support bounded pagination and must not require loading all records.",
  "rationale": "The staff list page is a repeated operational workflow.",
  "architectureRefs": {
    "decisions": ["adr-current-001"],
    "risks": ["risk-unbounded-list"]
  },
  "verificationStrategy": "Task verification should cover paginated query parameters and result metadata."
}
```

Weak NFR:

```json
{
  "category": "performance",
  "target": "The system should be fast."
}
```

## Missing User NFRs

When the user did not state explicit NFRs:

- Do not block the flow.
- Infer only minimum product-quality NFRs needed for current-phase delivery.
- Record them as architecture assumptions with verification strategy.
- Keep targets modest and implementation-verifiable.

## Review Routing

- Missing NFR structure in AAC -> architecture repair.
- NFR not assigned to a task -> taskplan repair.
- Assigned task lacks evidence -> execution repair.

## Anti-Patterns

- Writing NFRs with no verification strategy.
- Turning every best practice into a mandatory NFR.
- Creating cloud-scale NFRs for a local-first or internal product phase.
- Mixing UI quality rules into architecture NFRs; use UIX contract for UI-specific quality.
