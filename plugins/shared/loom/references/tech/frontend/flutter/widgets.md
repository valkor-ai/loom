# Flutter Widgets And Business Surfaces

Design widgets around task-owned product surfaces, visible states, and user intent. Reusable widgets receive typed values/callbacks; feature screens may coordinate selected state/repository boundaries without hiding business logic in layout code.

## Widget Contracts

Use required constructor parameters when rendering cannot proceed without a value and explicit defaults for optional behavior. Keep callbacks event-oriented and include stable target identity.

```dart
class OrderRow extends StatelessWidget {
  const OrderRow({
    required this.order,
    required this.onInspect,
    this.disabled = false,
    super.key,
  });

  final OrderRowViewModel order;
  final ValueChanged<String> onInspect;
  final bool disabled;

  @override
  Widget build(BuildContext context) => ListTile(
    key: ValueKey(order.id),
    title: Text(order.number),
    onTap: disabled ? null : () => onInspect(order.id),
  );
}
```

Do not pass repositories, router objects, or mutable API entities into presentational widgets. Avoid generic components with many booleans that encode unrelated product variants.

## Build Purity And Lifecycle

Keep `build()` deterministic and cheap. Create/dispose text/editing/scroll/animation/focus controllers and subscriptions in the owning State lifecycle, or use established hooks safely.

Use `const` for static constructors/children, but do not churn code solely for const when state ownership/rebuild scope is the real issue. Never start network calls, timers, or state writes directly during build.

Use `didUpdateWidget`, `didChangeDependencies`, post-frame callbacks, and keys only for their intended lifecycle semantics. Post-frame callbacks must not become an endless setState loop.

## Layout And Responsiveness

Compose with constraints (`LayoutBuilder`, `MediaQuery`, repository breakpoints) rather than assumed device sizes. Keep fixed controls/toolbars stable while content adapts.

Use `Expanded`/`Flexible`/scroll views deliberately; avoid unbounded constraints, nested same-axis scrollables, and shrinkWrap on large dynamic lists. For dense list/detail workflows, define mobile cards/drawer/route and tablet/desktop split behavior.

Respect safe areas, keyboard insets, text scaling, long/localized labels, orientation, pointer/hover, and desktop/web widths. Controls and text must not overlap or clip incoherently.

## Collections And Slivers

Use `ListView.builder`, `GridView.builder`, `SliverList`, pagination, or repository abstractions for large/unknown collections. Preserve stable keys and displayed action identity after reorder/filter/refresh.

Use slivers when one coordinated scroll surface needs app bars, headers, grids, and lists. Avoid nesting independent scrollables merely to reproduce a static mockup.

Empty/error/loading states belong inside the collection region while preserving surrounding page context and actions.

## Forms, Dialogs, And Actions

Own `Form` keys/controllers at a stable lifecycle. Distinguish client field validation from backend field/global errors. Preserve draft, focus first meaningful error, prevent duplicate submit, and reconcile saved response.

Use selected Material/Cupertino/design-system dialogs, menus, sheets, pickers, and buttons with semantic labels and focus/keyboard behavior. Destructive confirmation must show affected object and consequence; snack bars alone are not confirmation.

Do not use a tappable `Container`/`GestureDetector` when a semantic button/list control fits. Custom interactions need `Semantics`, focus, keyboard shortcuts/actions, hit size, pressed/disabled feedback, and screen-reader behavior.

## Theme And Visual System

Use `Theme.of`, `ColorScheme`, text theme, theme extensions, and repository tokens/components. Avoid repeated literal colors, spacing, radius, and typography inside feature widgets.

Images/icons need bounded dimensions, fit, loading/error behavior, semantics, and asset declarations. Motion should communicate state and respect reduced-motion/platform behavior; avoid decorative animation that obscures repeated work.

## Verification

- Test rendering through visible text/semantics and callbacks through taps/keyboard/input.
- Cover loading, empty, ready, validation, disabled, submitting, conflict, permission, offline, and confirmation states owned by the widget.
- Verify dynamic row identity/action target after sorting/filtering/paging/refresh.
- Test focus, semantics, hit targets, error association, dialog/sheet dismissal, and focus return.
- Exercise representative narrow/wide/text-scale/long-content constraints.
- Use golden tests only where the repository maintains them and visual regression is the actual risk.

## Delivery Evidence

Name the widget contract/layout/state and the widget/semantics/viewport assertion proving it. A constructor unit test, static tree dump, or single unconstrained golden cannot prove interaction identity, lifecycle, accessibility, or responsive usability.

## Unsafe Defaults

- Repository/router/state-library access inside reusable visual widgets.
- Futures/controllers/providers created in `build()`.
- Large eager child lists or shrinkWrap used to silence constraint problems.
- Dynamic rows keyed by index or no key where identity matters.
- Raw gesture containers replacing semantic controls.
- One-off visual literals bypassing the selected theme/tokens.
