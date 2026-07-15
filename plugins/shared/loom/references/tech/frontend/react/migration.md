# React Migration

Migrate only the task-owned React boundary and preserve observable behavior. A migration is not permission to modernize adjacent components, replace established libraries, or adopt APIs unsupported by the accepted React/framework version.

## Migration Inventory

Before editing, identify the component public props, rendered states, events, route/store/context dependencies, subscriptions, timers, refs, error behavior, and tests. Record which behaviors must remain equivalent and which change is explicitly required.

Keep a class component when it is an error boundary, depends on class inheritance, or cannot be converted inside the task boundary without changing public behavior.

Prioritize changed, reusable, or correctness-sensitive components. Stable components outside the owned feature are not migration candidates.

## Lifecycle Mapping

Map behavior by purpose rather than translating each lifecycle mechanically:

| Class behavior | Functional design |
| --- | --- |
| constructor state | `useState` lazy initialization or `useReducer` |
| `componentDidMount` subscription | effect setup plus symmetric cleanup |
| prop-sensitive update | render derivation, event handling, or dependency-complete effect |
| `componentWillUnmount` | effect cleanup that cancels/disposes real work |
| previous-state `setState` | functional update or reducer transition |
| `shouldComponentUpdate` | measured `memo` boundary, often no replacement |
| `getDerivedStateFromProps` | derive during render unless an editable draft is required |
| `componentDidCatch` | retain a class error boundary or repository wrapper |

Do not combine unrelated mount/update/unmount logic into one large effect. Separate subscriptions, browser integration, and replaceable async work so each has correct dependencies and cleanup.

## State And Instance Fields

Use separate state for independent values and a reducer for coupled transitions such as idle/loading/success/failure or multi-field editing. Preserve class `setState` merge semantics explicitly; hook setters replace values.

Move mutable non-render fields such as timer IDs, DOM handles, previous values, and third-party instances to refs. UI-visible values remain state. Convert callbacks that depend on previous state to functional updates so rapid events do not lose transitions.

Do not mirror incoming props into state during conversion. Preserve a separate draft only when the user can edit independently of refreshed server data, and define reset/rebase behavior.

## HOC And Render-Prop Conversion

Replace a HOC or render prop only when the resulting hook has a clear typed input/output contract and removes real nesting or duplication. Preserve provider order, subscription lifetime, ref forwarding, static metadata, display names used by tooling, and error/loading semantics.

Do not hide routing, authorization, tenant selection, API policy, or global side effects in a generic convenience hook. Keep those dependencies visible at the feature boundary.

## Context And Refs

When replacing legacy context, preserve provider scope and default-value failure behavior. A permissive fake default can make missing providers silently run with invalid state; use an explicit nullable context plus an invariant when absence is an error.

Preserve the repository's ref convention. Use `forwardRef` for versions that require it; adopt ref-as-prop only when the accepted React version and types support it.

## Incremental Delivery

Keep public exports, routes, API payloads, selectors, analytics events, accessible names, focus behavior, and user-visible copy stable unless the requirement changes them. Avoid mixing migration with state-library, router, styling, or test-runner replacement.

Migrate in a buildable slice. If old and new implementations coexist temporarily, keep a single source of truth and remove the temporary adapter inside the task when the cutover is complete.

## Verification

- Capture focused behavior tests before conversion when existing coverage is absent or implementation-sensitive.
- Prove initial render, prop changes, user events, validation/error states, and emitted payloads remain equivalent.
- Exercise subscription/listener/timer disposal and replacement of stale async work on dependency change and unmount.
- Run the repository typecheck, lint rules for hooks, focused component tests, and production build affected by changed exports.
- Verify error-boundary behavior separately when a class boundary is intentionally retained.

## Delivery Evidence

Name the migrated boundary, the lifecycle/state mapping decisions, the retained exceptions, and the behavior assertions proving parity. A successful compile or smaller component file does not prove lifecycle, cleanup, focus, error, or data-flow equivalence.

## Unsafe Defaults

- Repository-wide class conversion attached to a feature task.
- One effect that imitates several unrelated lifecycle methods.
- Mounted flags used instead of cancellation or stale-result ordering.
- Hook dependency lint suppression introduced to preserve old behavior.
- HOCs replaced with hooks that hide broader global dependencies.
- Error boundaries removed because no hook equivalent exists.
- Memoization added mechanically during migration without a measured boundary.
