# Consumer Compatibility

Inventory supported consumers using package references, documented integrations, and available usage records. Include queued events, scheduled jobs, scripts, and persisted configuration that survive deploys.

Test the combinations that the rollout can actually produce: old consumer with new producer, new consumer with old producer, and delayed old messages after cutover. Assert public behavior such as defaults, serialization, exit status, and retry semantics. For events, include replay and duplicate processing where applicable.

Record migration ownership, the replacement behavior, and the compatibility cutoff. Telemetry showing no old-version traffic is useful only for the observation window and surfaces measured. Identify offline or infrequent consumers separately.
