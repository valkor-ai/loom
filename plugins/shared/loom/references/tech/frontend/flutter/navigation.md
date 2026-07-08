# Flutter Navigation Quality

This file applies GoRouter and Flutter navigation discipline to task-owned route trees, shell routes, redirects, deep links, parameters, auth guards, and navigation side effects.

## When To Use

- The task creates or changes GoRouter setup, route configuration, shell routes, nested routes, route params, query params, redirects, auth gates, deep links, navigation calls, or MaterialApp.router wiring.
- Use this when navigation ownership affects product workflow, protected access, back-stack semantics, URL/deep-link behavior, or persistent app chrome.
- If the task only changes a screen's internal widget rendering, keep this file out of scope unless route behavior changes.

## Implementation Focus

- Preserve the repository's navigation library. Do not introduce GoRouter into an app that intentionally uses Navigator 1.0, AutoRoute, Beamer, or another router unless the accepted architecture requires migration.
- Keep router construction near app-level ownership and inject auth/session dependencies through existing provider/bloc/service patterns.
- Use `context.go` for replacing location and `context.push` for adding to stack. Do not use replacement when users need to return to the previous workflow step.
- Validate path and query parameters before fetching or mutating data. Missing, malformed, unauthorized, or not-found parameters need explicit route or screen states.
- Use shell routes for persistent app chrome such as tabs, side navigation, or authenticated shells. Avoid duplicating tab/scaffold wrappers in every child route.
- Keep redirect logic deterministic and side-effect free. Redirects should read state and return a target; they should not show dialogs, mutate storage, or fire network calls.
- Keep large mutable objects out of route params. Pass stable IDs or small serializable values and load current records through the data/state layer.
- Preserve deep-link behavior when route paths, URL schemes, or web paths are part of the product.
- Keep navigation side effects from state management in listeners or route orchestration, not directly inside reducers/notifiers/blocs.

## Verification Focus

- Verify route entry, push/go semantics, back behavior, nested/shell route rendering, auth redirects, invalid params, query params, and not-found states touched by the task.
- Verify deep links or browser refresh for public/web routes when applicable.
- Verify protected routes handle loading auth state without flicker or redirect loops.
- Verify navigation calls target displayed record IDs after filtering, sorting, list refresh, or modal interactions.
- Run route-focused widget tests or app-level navigation tests when the repository has utilities for them.

## Evidence Focus

- In the evidence summary, name the navigation decision: router ownership, go-vs-push semantics, shell route, redirect rule, param validation, deep-link proof, protected route handling, or navigation-listener boundary.
