# Vue Component Contracts

Apply component guidance when the task owns Vue props, emits, models, slots, dependency injection, dynamic/kept-alive components, Teleport, transitions, or reusable component boundaries.

## Ownership Boundary

Feature components may know product state and commands; reusable UI primitives receive values and emit intent without importing domain API/store/router logic.

Split components around independent state, behavior, accessibility, rendering cost, and reuse. Avoid a single component controlled by many unrelated booleans or a component per markup fragment.

Model component APIs from runtime use. Backend DTOs rarely make safe editable form/view models when values may be partial, formatted, invalid, or pending.

## Props And Defaults

Use the repository's TypeScript or runtime prop convention. Keep required/optional/null behavior exact and use factory defaults for mutable arrays/objects where runtime declarations require it.

Do not mutate props. Derive values with computed state, emit an update, or keep an explicit local draft with rules for parent refresh/reset.

Avoid copying props into local refs without synchronization semantics. A draft must define when it initializes, rebases, discards, and survives backend failure.

## Emits And Commands

Declare emitted events and payloads. Event names should describe intent/result rather than DOM mechanics, and payloads must carry stable target identity plus all command inputs.

Do not emit a generic object bag or make the parent read mutable selected state to infer the target. Do not use component emits as a global event bus.

Validate runtime payloads when JavaScript/external consumers can violate static types and the component boundary is public.

## Model Contracts

Use `v-model`/`defineModel` only when the installed Vue version and repository convention support a controlled value/update contract. Name multiple models and define modifiers/normalization deliberately.

Keep invalid editable text representable. Converting every input immediately to a domain number/date can erase intermediate values and make validation impossible.

Avoid two sources of truth between parent model, internal ref, form library, and store. Emit final normalized values at the agreed boundary.

## Slots

Use slots for layout/content extension and scoped slots when the child owns data/actions the parent renders. Keep slot props small, stable, typed where tooling supports it, and semantically documented through usage.

Provide useful default slot content only when omission is valid. Do not make accessibility labels, required actions, or business state disappear silently because a slot was absent.

Stable domain keys still apply inside generic list/table slots; index keys are unsafe for mutable collections.

## Provide And Inject

Use provide/inject for stable cross-tree dependencies such as form, theme, compound component, or plugin context, not as an invisible substitute for ordinary feature data flow.

Use a typed `InjectionKey`, provide refs/readonly state when reactivity is required, and fail clearly when required context is missing. Providing `readonly(user.value)` captures a plain current value; provide the readonly ref/reactive owner instead.

Keep mutations in provider-owned commands rather than allowing descendants to mutate shared state freely.

## Teleport, Dialogs, And Dynamic Content

Teleport changes DOM placement, not Vue ownership. Dialogs/menus/toasts need correct labels, focus entry/trap/return, escape/outside behavior, scroll locking, stacking, and target availability.

Use `KeepAlive` only when preserved component state is product behavior. Define include/exclude/key/cache bounds and lifecycle handling with activated/deactivated hooks.

Async/dynamic components require stable loading, failure, retry, and ready states. Transitions must respect reduced motion and must not gate correctness on animation completion.

## Verification

- Test prop defaults/nullability, emitted payload and stable target identity, model updates, modifiers, and invalid intermediate values.
- Exercise named/scoped/default slots and missing required context.
- Verify Teleport dialog focus, close behavior, action target, stacking, and restored focus.
- Test KeepAlive activation/deactivation and dynamic/async loading-error-retry behavior when owned.
- Run SFC type/build checks for prop/emit/slot/injection/model consumers.

## Delivery Evidence

Name the public component contract and the consumer-visible assertion proving it. A mounted component or snapshot does not establish model ownership, emitted target integrity, injected reactivity, Teleport accessibility, or kept-alive lifecycle behavior.

## Unsafe Defaults

- Props copied into local state with no rebase/reset policy.
- Broad object emits requiring parents to infer intent.
- `v-model` layered over another independent source of truth.
- Provide/inject used as an untyped hidden global store.
- Plain snapshot values provided when reactive updates are expected.
- Teleported overlays without focus and close semantics.
- KeepAlive or async components enabled without lifecycle/error behavior.
