# Redis Integration Core

## When To Use

Use this reference when the task owns an application boundary that uses an accepted Redis capability. It provides cross-capability integration rules; use the capability-specific reference for the selected capability's behavior.

Do not add Redis because a package, environment variable, or generic architecture text mentions it. The accepted TechnicalBaseline provider and task-owned capability are the applicability authority.

## Implementation Focus

- Keep the business source of truth explicit. Redis is not automatically authoritative for domain records.
- Give every key a stable namespace owned by the feature and include every identity or version dimension that changes the value.
- Include tenant, actor, locale, permission, filter, or schema version dimensions when they change the value.
- Bound value size, list length, stream retention, concurrency, and retry work.
- Set timeouts and connection-pool limits at the selected framework integration boundary.
- Treat Redis unavailability according to the accepted capability contract; optional optimization paths may fall back, while required work may fail explicitly.
- Keep serialization versioned and avoid putting mutable ORM entities or credentials into shared Redis values.
- Use one application-owned adapter rather than scattering provider commands through handlers.

## Capability Boundaries

Redis capabilities have different correctness rules. Do not combine them behind one generic helper that hides TTL, authorization, retry, acknowledgment, or lease behavior.

If two capabilities need different durability or isolation policies, use separate logical namespaces or separate accepted dependency ids. Do not silently change the shared Redis deployment to satisfy an unconfirmed requirement.

## Verification Focus

- Verify the selected capability's key namespace and identity dimensions.
- Verify connection timeout, reconnect, and unavailable-provider behavior.
- Verify values cannot cross tenants, users, versions, or feature boundaries.
- Verify expiration, cleanup, and bounded resource behavior.
- Verify serialization remains compatible with the current code and deployment image.

## Evidence Focus

Record the adapter, capability, key examples, configuration source, unavailable-provider behavior, and the focused test or runtime evidence that proves the decision.

Evidence should point to changed files and concrete test cases. A successful Redis ping does not prove application-level key isolation or recovery behavior.

## Unsafe Defaults

- Treating Redis package detection as a reason to add a service.
- Using `localhost` for a container-to-container Redis URL.
- Sharing keys between unrelated capability data without an explicit contract.
- Storing secrets, access tokens, or unrestricted domain objects in broad keys.
- Leaving TTL, size, timeout, or retry behavior to provider defaults.
