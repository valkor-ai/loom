# Non-Functional Requirements In Loom Architecture

Use this reference when writing `architectureQuality.nfrs` or reviewing whether an architecture gives later tasks verifiable quality targets.

NFRs in Loom must be concrete enough for TaskPlan and Review. They are not slogans.

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

Scalability NFRs should describe the current phase's growth boundary, not a generic future scale claim:

```json
{
  "nfrId": "nfr-list-growth-boundary",
  "category": "scalability",
  "target": "List reads must remain bounded by pagination and indexed filters as records grow beyond manual review size.",
  "rationale": "The staff workflow repeatedly scans operational records.",
  "architectureRefs": {
    "decisions": ["adr-current-001"],
    "risks": ["risk-unbounded-list"]
  },
  "verificationStrategy": "Task verification should cover bounded query parameters and the declared default/max page size."
}
```

Availability NFRs should describe concrete runtime or dependency behavior:

```json
{
  "nfrId": "nfr-api-dependency-outage",
  "category": "availability",
  "target": "When the downstream approval service is unavailable, the API returns an actionable unavailable response instead of silent success.",
  "rationale": "Operators need to distinguish retryable outage from business rejection.",
  "architectureRefs": {
    "decisions": [],
    "risks": ["risk-approval-dependency"]
  },
  "verificationStrategy": "Task verification should cover the unavailable dependency path or document a known gap."
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
- Do not invent cloud-scale targets, multi-region availability, or high-throughput assumptions when the current phase is local/internal and no requirement supports them.

## Review Routing

- Missing NFR structure in AAC -> architecture repair.
- NFR not assigned to a task -> taskplan repair.
- Assigned task lacks evidence -> execution repair.

## Anti-Patterns

- Writing NFRs with no verification strategy.
- Turning every best practice into a mandatory NFR.
- Creating cloud-scale NFRs for a local-first or internal product phase.
- Mixing UI quality rules into architecture NFRs; use UIX contract for UI-specific quality.
