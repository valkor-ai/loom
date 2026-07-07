# Angular Testing Quality

This file applies Angular testing discipline to task-owned components, services, guards, resolvers, RxJS streams, NgRx stores/effects, and feature workflows.

## When To Use

- The task creates or changes Angular behavior that should be proven with TestBed, service tests, component tests, guard/resolver tests, observable tests, NgRx tests, or integration-style UI tests.
- Use this for test scope, test data, async handling, dependency mocks, HTTP testing, route testing, and evidence quality.
- Pair with component, routing, RxJS, or NgRx references when the task changes those implementation areas.

## Implementation Focus

- Test behavior through public inputs, rendered text, controls, outputs, HTTP calls, router decisions, store actions, or observable emissions. Avoid tests that assert private implementation details.
- Configure standalone component tests by importing the component under test. Provide only the services, stores, routes, pipes, and tokens needed for the behavior being proven.
- Use `HttpTestingController` for Angular HTTP services. Assert URL, method, body, params, headers, success mapping, and error mapping for task-owned API calls.
- Use `TestBed.runInInjectionContext()` for functional guards, resolvers, interceptors, or functions that rely on `inject()`.
- Test signals by reading signal values and then triggering the real event or service response that updates them. Do not bypass the workflow by setting all component state directly unless it is purely presentational.
- Use marble tests or scheduler-based tests where stream ordering, timing, cancellation, or retry behavior is the risk. Keep simpler observable tests direct.
- Use mock store or facade mocks for component tests. Use reducer/selector/effect tests for NgRx behavior itself.
- Keep test fixtures domain-specific and small. Do not build giant generic mock objects when the workflow needs only a few fields.
- Avoid brittle DOM selectors when accessible queries, labels, roles, text, or stable test IDs already exist in the repository.

## Verification Focus

- Run the focused Angular test command for the changed project or library, then build/typecheck/lint when templates, imports, routes, or public contracts changed.
- Cover loading, empty, ready, validation error, business-blocking error, submitting, success, and disabled states touched by the task.
- Cover failure paths for guards, resolvers, HTTP services, effects, and destructive actions.
- Verify cleanup for subscriptions, router events, timers, and async callbacks when lifecycle behavior changed.
- Verify no test relies on delivery text, framework explanations, or temporary runtime instructions being present in the UI.

## Evidence Focus

- In the evidence summary, name the Angular test proof: component behavior, service HTTP contract, guard/resolver decision, RxJS timing/order, NgRx reducer/selector/effect, lifecycle cleanup, or route integration.
