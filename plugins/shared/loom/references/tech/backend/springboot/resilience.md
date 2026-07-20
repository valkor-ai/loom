# Spring Boot Resilience Policies

Resilience policies implement accepted dependency failure behavior. They do not make every external call reliable automatically, and they must not alter business meaning.

## Policy Ownership

Start with the failure contract:

- dependency and operation
- timeout budget
- retry safety/idempotency
- terminal caller-visible result
- degraded behavior, if truthful
- concurrency/resource limit
- recovery signal and observability

Use Resilience4j, Spring Cloud CircuitBreaker, or the repository's existing library. Do not stack multiple retry/circuit implementations around the same call.

## Timeout Budget

Set connection, response, and total operation budgets coherently. An outer timeout must account for all attempts and backoff. Avoid a retry policy whose worst-case duration exceeds the caller/runtime budget.

Timeout cancellation does not guarantee the downstream operation stopped. Treat timed-out writes as outcome-unknown unless idempotency or provider status lookup resolves them.

## Retry Safety

Retry only classified transient failures and only when the operation is safe:

| Operation | Retry Position |
|---|---|
| Idempotent read | Bounded retry can be valid for connection/selected `5xx` failures |
| Idempotent update with stable key/version | Retry only under the accepted conditional/idempotency contract |
| Create/payment/state transition | No automatic retry without idempotency or deduplication |
| Validation/auth/not-found/conflict | Do not retry without changed input or authorization |

Honor `Retry-After` when the provider contract supports it. Use jittered backoff for shared dependencies where coordinated retries could amplify an outage.

## Circuit Breaker

A circuit breaker protects callers and resources from repeatedly invoking an unhealthy dependency. Configure failure classification, slow-call threshold, minimum calls, window, open duration, and half-open probes from the current dependency behavior.

Do not register every business rejection as a circuit failure. Do not expose circuit state as the product error; translate it to the accepted unavailable/degraded behavior.

## Bulkhead And Concurrency

Use semaphore/thread-pool bulkheads only when dependency concurrency can exhaust application resources. Define queue/rejection behavior and preserve context deliberately. A thread-pool bulkhead in a reactive chain can reintroduce blocking semantics.

## Fallback Semantics

A fallback is valid only when it is truthful and safe:

- cached read within an accepted freshness window
- reduced optional data with explicit degraded indication
- queued durable work when asynchronous acceptance is part of the contract
- actionable unavailable response

Never fabricate domain records, claim a write succeeded, or return stale authorization-sensitive data merely to keep status `200`.

## Policy Composition

Keep attempt and resource budgets visible when combining timeout, retry, circuit breaker, rate limiter, and bulkhead. Apply metrics/events through the observability boundary without high-cardinality tags.

## Verification Focus

Useful resilience evidence includes:

- exact retryable and non-retryable failure classes/statuses
- total attempts and backoff under deterministic test timing
- no retry for non-idempotent writes without a key
- timeout terminal behavior and outcome-unknown handling
- circuit open and half-open transitions
- bulkhead saturation/rejection behavior
- truthful fallback and propagated correlation signal
- one policy layer rather than duplicated gateway/client retries

## Unsafe Defaults

- `.retry(3)` on every WebClient call.
- Retrying validation, auth, conflict, or not-found responses.
- Combining gateway retry and client retry without an attempt budget.
- Returning fake successful data from fallback methods.
- Marking every exception as a circuit failure.
- Using production-duration sleeps in tests.
