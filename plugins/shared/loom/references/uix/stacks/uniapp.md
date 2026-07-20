# UIX Stack: UniApp And Mini-App

Use for UniApp, WeChat/Alipay mini-programs, H5/mobile hybrid targets, and similar cross-platform mobile surfaces.

## Structure

- Follow the target platform's page, component, store, and routing conventions.
- Keep platform-specific capabilities behind small adapters when multiple targets are in scope.
- Design for mobile task flows first; desktop web patterns should not leak into mini-app screens.

## Suggested Split

```text
pages/
components/
stores/
services/
styles/
  tokens
```

## Implementation Rules

- Respect safe areas, native navigation bars, tab bars, and platform gesture expectations.
- Use platform-compatible units and components according to the repo's existing stack.
- Keep forms single-column, touch-friendly, and explicit about validation.
- Avoid hover-only interactions and tiny table layouts.
- Handle loading, empty, error, permission, and business-blocking states on the page that triggers them.
- Translate token template intent into the project's UniApp style variables or theme file; do not add web-only CSS that the target cannot consume.
- Keep page actions reachable with thumb-friendly spacing and platform keyboard behavior.

## Page Pattern

```html
<view class="page">
  <view class="page-header"></view>
  <scroll-view class="page-content"></scroll-view>
  <view class="page-actionbar"></view>
</view>
```

Use project-native syntax and components; the pattern is about regions, not exact markup.

## Verification

- Use the available mini-app/H5 preview target when present.
- Check safe areas, keyboard behavior, scroll, and platform permission flows.
- Verify target-specific API limitations or permission prompts do not leave the user on a blank page.

## Cross-Target Page Boundary

UIX owns the page regions and mobile task flow. Platform conditionals, package
configuration, API adapters, and target build rules remain in the repository's
UniApp engineering boundary.

```text
pages.json route -> page shell -> scroll/content region -> action bar
-> validation/permission -> platform result -> updated page or next route
```

- Keep one page identity and one primary task across H5, mini-app, and native builds; adapt controls where platform capability requires it.
- Use platform-native safe areas, navigation bars, tab bars, and keyboard behavior rather than importing desktop web layout assumptions.
- Put loading, empty, permission, error, success, and business-blocking feedback in the page region that owns the action.
- Keep conditional compilation around platform adapters or capability-specific controls, not around duplicated business workflow markup.
- Extend the existing `uni.scss` or theme variables for semantic tokens. Do not create a web-only CSS layer that the target renderer cannot consume.
