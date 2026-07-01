# Motion Tokens

Use when adding or reviewing transitions, loading indicators, scene movement, route changes, or microinteractions.

## Rules

- Motion must communicate state, relationship, feedback, or progress.
- Prefer transform and opacity for transitions. Avoid animating layout properties when it causes jank.
- Use short durations for repeated workflow UI. Long or decorative motion should be rare and purposeful.
- Respect `prefers-reduced-motion` or platform equivalents.
- Loading indicators should match expected wait: skeleton for known layout, progress for measurable long operations, and subtle indicators for short async work.
