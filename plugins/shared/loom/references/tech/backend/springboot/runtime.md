# Spring Boot Runtime Quality

Use this topic reference when `tech/backend/springboot/runtime.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. This file applies Spring Boot runtime behavior rules; deployment assets remain under Loom deploy references.

## When To Use

- The task changes Spring Boot application properties, profiles, typed configuration, actuator/health endpoints, startup behavior, logging, tracing, resilience, external-service clients, graceful shutdown, or runtime diagnostics.
- Use this when the code must start reliably across local/test/runtime environments or expose operational state needed by later tasks.
- Do not use this file for Dockerfile, Compose, Kubernetes, registry, or cloud deployment asset generation.

## Implementation Focus

- Prefer typed `@ConfigurationProperties` for grouped settings and keep secrets, URLs, credentials, ports, and feature flags outside source code.
- Provide safe local/test defaults when a new bean would otherwise fail startup before the relevant external service exists.
- Use Spring profiles deliberately. Do not make tests or local runtime depend on production-only settings.
- Add actuator endpoints only for runtime needs in scope. Expose minimal health/info behavior and avoid leaking environment, secrets, or broad actuator details.
- Model health checks around dependencies the app truly needs to serve requests. Health indicators should be lightweight and not run business workflows.
- Keep logging structured enough to diagnose failures without dumping sensitive payloads. Preserve existing correlation/request-id conventions when present.
- Add resilience patterns such as timeout, retry, circuit breaker, or bulkhead only for external calls with a current failure mode. Do not add Spring Cloud components as boilerplate.
- For startup runners, schedulers, and background tasks, define ownership, idempotency, failure handling, and shutdown behavior.
- Keep runtime changes aligned with architecture/runtime-delivery artifacts while leaving container/server generation to deploy.

## Verification Focus

- Run an application context startup or runtime smoke for configuration, actuator, health, scheduler, or bean wiring changes.
- Test configuration binding defaults and invalid-value behavior when the task adds required settings.
- Probe the exact actuator/health/runtime endpoint changed by the task.
- For external client resilience, verify timeout/error mapping and ensure retries do not duplicate unsafe operations.

## Evidence Notes

- Record `springboot.runtime` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/backend/springboot/runtime.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the runtime decision: typed config, profile/default, actuator exposure, health indicator, logging/tracing, resilience, startup task, shutdown, or runtime smoke.
