# React Migration Quality

This file applies when a task explicitly migrates class components, HOCs, render props, or legacy React patterns to modern React.

## When To Use

- The task owns migration from class components to function components, lifecycle methods to hooks, HOCs/render props to custom hooks, or legacy context to modern context hooks.
- Use this only for migration tasks. Do not migrate stable working components merely because a nearby feature is being edited.
- Keep class components when they are error boundaries, required by third-party inheritance, or outside the task's safe change boundary.

## Implementation Focus

- Migrate only the component or feature boundary owned by the task. Avoid repository-wide conversion unless explicitly requested.
- Add or preserve behavior tests before migration when the existing component has meaningful behavior.
- Map lifecycle methods deliberately: mount/update/unmount behavior becomes effects with complete dependencies and cleanup.
- Replace `this.setState` updates that depend on previous state with functional state updates or reducers.
- Use reducers for complex related state transitions rather than many drifting `useState` calls.
- Replace HOCs and render props with custom hooks only when the resulting hook has a clear public contract and simpler data flow.
- Preserve error boundaries as class components unless the repository already has a compatible wrapper pattern.
- Keep refs, timers, subscriptions, and mutable instance fields in `useRef` with cleanup where needed.
- Avoid over-memoization during migration. Use `memo`, `useMemo`, and `useCallback` only where they preserve existing performance or support stable child props.
- Do not change product behavior, user-visible copy, route shape, or API contracts as a side effect of migration.

## Verification Focus

- Run existing tests before and after migration when feasible, or add targeted tests around the migrated behavior.
- Verify lifecycle-equivalent behavior: initial load, prop changes, cleanup on unmount, event listener disposal, timer disposal, and async cancellation.
- Check that no hook dependency lint suppressions were added to hide stale closure issues.
- Verify TypeScript props, state, and ref types are complete after migration.

## Evidence Focus

- In the evidence summary, name the migration decision: lifecycle mapping, cleanup preservation, reducer extraction, HOC-to-hook conversion, render-prop conversion, error-boundary exception, or behavior test preservation.

