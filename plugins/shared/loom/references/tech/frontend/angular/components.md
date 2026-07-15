# Angular Components And Templates

Components translate task-owned business surfaces into accessible rendering and interactions. Design component APIs around product concepts and visible workflow state, not raw backend entities or generic wrappers.

## Component Responsibility

Container/surface components may coordinate route data, API/facade/store state, and business actions. Presentational components receive typed inputs, emit user intent, and render states without hidden navigation, HTTP, or store mutation.

Split components at reusable behavior/visual boundaries or independent state ownership. Do not fragment every row/label into a component, and do not build one page component that owns unrelated list, detail, form, modal, and transport logic.

## Inputs, Outputs, And Models

Use signal inputs/outputs/models only on a compatible Angular version. Required inputs are appropriate when the component cannot render without a value; optional inputs need explicit defaults and fallback behavior.

```typescript
@Component({
  selector: 'app-order-row',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './order-row.component.html',
})
export class OrderRowComponent {
  readonly order = input.required<OrderRowViewModel>();
  readonly disabled = input(false);
  readonly inspect = output<{ orderId: string }>();
}
```

Emit stable identifiers plus required context. A row action should not depend on a mutable global selected item that can change after sorting, filtering, refresh, or modal opening.

Use `model<T>()` for genuinely controlled simple values. Save/validate/confirm workflows need explicit events and draft state rather than implicit two-way mutation of persisted objects.

## Template Control Flow

Keep templates declarative and bounded. Use `@if` for mutually exclusive state, `@for (...; track item.id)` for dynamic collections, and `@empty` for collection-empty output where supported.

Avoid methods/getters that allocate, sort, filter, format, or mutate on every change-detection pass. Compute view models in signals/selectors/pipes. Do not use `track $index` for reorderable, pageable, insertable, or refreshable records.

Keep loading/error/empty feedback near the affected region while retaining page context and recovery actions. Overlay spinners should not erase unrelated usable work.

## Forms And Controls

Use native semantic controls or the established component library. Every input has a programmatic label; validation messages associate to the field; icon-only buttons have accessible names; disabled and read-only semantics are not interchangeable.

For typed reactive forms, initialize defaults intentionally and keep DTO conversion at a boundary:

```typescript
readonly form = this.formBuilder.nonNullable.group({
  supplierName: ['', [Validators.required, Validators.maxLength(120)]],
  amount: [0, [Validators.required, Validators.min(0.01)]],
});
```

Show field and form-level backend errors, preserve valid draft values, focus/announce meaningful blocking errors, and prevent duplicate submits. Confirmation dialogs must keep affected object identity and consequence visible.

## Content Projection And Reuse

Use content projection for stable extension points such as toolbar actions, card/header content, empty state, or modal footer. Name/select slots clearly and provide sensible default rendering where appropriate.

Avoid generic shell components with many boolean inputs and projection slots that hide page composition. Prefer focused product primitives with typed APIs.

## Styling, Layout, And Responsiveness

Use repository UIX tokens, semantic colors, spacing, typography, density, and component primitives. Keep component styles scoped according to the existing strategy and avoid `::ng-deep`, global selectors, and one-off inline values unless a documented integration requires them.

Business tables need stable columns/actions and a narrow-screen strategy such as cards, drawer/detail route, or intentional horizontal handling. Fixed toolbars, overlays, menus, and dialogs must not overlap content or lose keyboard/focus behavior.

Use Angular CDK overlay/focus utilities or the selected component library for complex dialogs, menus, drag/drop, and focus trapping rather than reimplementing interaction primitives casually.

## Performance And Lifecycle

OnPush and stable input identity reduce work only when state updates are explicit. Virtualize genuinely large lists with CDK or repository tooling after measuring, and preserve item identity, keyboard behavior, and scroll restoration.

Clean up event/observer/subscription resources through `DestroyRef`, `takeUntilDestroyed`, async pipe, or signal interop. Avoid manual DOM listeners and timers without teardown.

## Verification

- Test visible behavior through public inputs, DOM roles/labels/text, and emitted user events.
- Cover loading, empty, populated, validation, disabled, submitting, conflict, permission, and destructive confirmation states owned by the component.
- Verify row/action identity after sort, filter, pagination, refresh, and overlay open/close.
- Exercise keyboard, focus return, error announcement, and accessible naming for custom interactions.
- Verify responsive behavior for dense lists/forms/details and long/localized content.
- Build templates to catch missing standalone imports and invalid bindings.

## Delivery Evidence

Name the component API/state and the rendered interaction assertion proving it. Private-field tests, shallow class construction, or static template inspection cannot prove binding, focus, accessibility, action identity, or responsive behavior.

## Unsafe Defaults

- Presentational components making HTTP/store/router decisions.
- Persisted entities mutated through two-way binding.
- Template methods doing repeated filtering/sorting/allocation.
- Index tracking on dynamic business records.
- Generic projected shells replacing clear page composition.
- Custom dialogs/menus without focus and keyboard behavior.
- One-off styles bypassing UIX/design-system tokens.
