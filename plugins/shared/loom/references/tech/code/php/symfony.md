# PHP Symfony Quality

This file applies Symfony conventions to task-owned behavior.

## When To Use

- The task changes Symfony controllers, services, DTO validation, Doctrine integration, Messenger handlers, console commands, event subscribers, voters, serializer groups, or kernel tests.
- Use this when Symfony's container, routing, validation, security, event, or message lifecycle affects correctness.
- If the PHP project is not Symfony, do not borrow Symfony-specific structure.

## Implementation Focus

- Keep controllers thin: map request input, validate, authorize, call an application service/handler, and return a typed response. Do not bury business workflows in controller methods.
- Prefer constructor injection and autowired services. Do not pull services from the container inside domain/application code unless the repository already owns a service-locator boundary.
- Use DTOs with Symfony Validator constraints for request payloads when the task owns input semantics. Keep validation errors aligned with the API error contract.
- Keep Doctrine entities separate from API request/response DTOs. Avoid exposing entities directly when serializer groups would hide business rules or lazy-loading surprises.
- Put multi-entity writes in explicit transaction/application service boundaries. Doctrine repositories should not secretly perform full workflows.
- Use voters for user permission decisions and application/domain services for business eligibility. Do not conflate authorization with business state validation.
- Event subscribers/listeners should react to meaningful events and stay small. If a listener starts to own workflow decisions, move that logic into a service and test it directly.
- Messenger messages should be small, serializable, and idempotent where retries are possible. Handlers should load current state by identifier and handle missing/stale data deliberately.
- Console commands should validate arguments/options, delegate work to services, return correct exit codes, and avoid embedding business logic.
- Serializer groups, normalizers, and response DTOs must make public response shape intentional; do not rely on default object serialization.

## Verification Focus

- Run kernel/controller functional tests for route changes and service/unit tests for business changes.
- Test validation failure, voter denial/allowance, successful response shape, and persistence state when those paths are touched.
- For Messenger, test handler behavior and dispatch shape using the project's configured transport test helpers.
- For console commands, run or test the command with success and failure inputs, including exit code expectations.

## Evidence Focus

- In the evidence summary, name the Symfony decision: DTO validation, controller/service split, Doctrine boundary, voter, event subscriber, Messenger handler, console command, serializer shape, or kernel-test proof.
