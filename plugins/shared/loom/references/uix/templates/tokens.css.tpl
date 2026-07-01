/* Loom UIX design tokens.
   Merge into the project's existing token/theme file when one exists.
   Do not create a parallel token system beside an existing design system. */

:root {
  /* Spacing: 4px grid */
  --space-0: 0;
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-8: 2rem;
  --space-10: 2.5rem;
  --space-12: 3rem;
  --space-16: 4rem;
  --space-20: 5rem;
  --space-24: 6rem;
  --space-32: 8rem;
  --space-40: 10rem;
  --space-48: 12rem;
  --space-64: 16rem;

  /* Typography */
  --font-family-sans: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", "Geist", system-ui, sans-serif;
  --font-family-serif: "Source Han Serif SC", "Noto Serif SC", "Songti SC", Georgia, serif;
  --font-family-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono", monospace;
  --font-size-xs: 0.75rem;
  --font-size-sm: 0.875rem;
  --font-size-base: 1rem;
  --font-size-lg: 1.125rem;
  --font-size-xl: 1.25rem;
  --font-size-2xl: 1.5rem;
  --font-size-3xl: 2rem;
  --font-size-4xl: 2.5rem;
  --font-size-5xl: 3.5rem;
  --font-weight-regular: 400;
  --font-weight-medium: 500;
  --font-weight-semibold: 600;
  --font-weight-bold: 700;
  --line-height-tight: 1.15;
  --line-height-snug: 1.3;
  --line-height-normal: 1.5;
  --line-height-relaxed: 1.65;
  --line-height-loose: 1.8;

  /* Radius and elevation */
  --radius-none: 0;
  --radius-xs: 2px;
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-2xl: 24px;
  --radius-3xl: 32px;
  --radius-full: 9999px;
  --shadow-xs: 0 1px 2px rgb(16 24 40 / 0.04);
  --shadow-sm: 0 1px 3px rgb(16 24 40 / 0.08), 0 1px 2px rgb(16 24 40 / 0.06);
  --shadow-md: 0 4px 8px rgb(16 24 40 / 0.10);
  --shadow-lg: 0 12px 24px rgb(16 24 40 / 0.12);
  --shadow-xl: 0 20px 40px rgb(16 24 40 / 0.16);
  --shadow-2xl: 0 28px 56px rgb(16 24 40 / 0.20);
  --shadow-inner: inset 0 2px 4px rgb(16 24 40 / 0.06);

  /* Motion */
  --duration-instant: 0ms;
  --duration-fast: 100ms;
  --duration-quick: 150ms;
  --duration-base: 200ms;
  --duration-medium: 300ms;
  --duration-slow: 400ms;
  --duration-slower: 600ms;
  --ease-out-quart: cubic-bezier(0.16, 1, 0.3, 1);
  --ease-out-quint: cubic-bezier(0.22, 1, 0.36, 1);
  --ease-out-expo: cubic-bezier(0.19, 1, 0.22, 1);
  --ease-out-cubic: cubic-bezier(0.33, 1, 0.68, 1);
  --ease-in-quart: cubic-bezier(0.5, 0, 0.75, 0);
  --ease-in-cubic: cubic-bezier(0.32, 0, 0.67, 0);
  --ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);

  /* Breakpoints are reference tokens; media queries must use literal values. */
  --breakpoint-sm: 640px;
  --breakpoint-md: 768px;
  --breakpoint-lg: 1024px;
  --breakpoint-xl: 1280px;
  --breakpoint-2xl: 1536px;
  --breakpoint-3xl: 1920px;

  /* Containers and layers */
  --container-sm: 640px;
  --container-md: 768px;
  --container-lg: 1024px;
  --container-xl: 1280px;
  --container-2xl: 1536px;
  --container-prose: 72ch;
  --z-hide: -1;
  --z-base: 0;
  --z-raised: 1;
  --z-dropdown: 10;
  --z-sticky: 20;
  --z-fixed: 30;
  --z-modal-backdrop: 40;
  --z-modal: 50;
  --z-popover: 60;
  --z-tooltip: 70;
  --z-notification: 80;
  --z-max: 9999;

  /* Touch targets */
  --touch-target-min: 44px;
  --touch-target-android: 48px;
}

:root,
[data-theme="light"] {
  --color-primary: oklch(0.48 0.15 255);
  --color-primary-hover: oklch(0.42 0.17 255);
  --color-primary-foreground: oklch(0.99 0.01 255);
  --color-secondary: oklch(0.62 0.08 230);
  --color-secondary-hover: oklch(0.56 0.10 230);
  --color-accent: oklch(0.68 0.13 165);
  --color-accent-foreground: oklch(0.12 0.02 165);
  --color-surface: oklch(0.99 0.004 255);
  --color-surface-tinted: oklch(0.96 0.01 255);
  --color-surface-elevated: oklch(1 0 0);
  --color-on-surface: oklch(0.18 0.02 255);
  --color-on-surface-muted: oklch(0.46 0.02 255);
  --color-on-primary: var(--color-primary-foreground);
  --color-border: oklch(0.88 0.01 255);
  --color-border-strong: oklch(0.68 0.02 255);
  --color-error: oklch(0.55 0.20 25);
  --color-error-foreground: oklch(0.99 0.01 25);
  --color-success: oklch(0.58 0.16 145);
  --color-success-foreground: oklch(0.99 0.01 145);
  --color-warning: oklch(0.76 0.15 80);
  --color-warning-foreground: oklch(0.22 0.03 80);
  --color-info: oklch(0.58 0.15 230);
  --color-info-foreground: oklch(0.99 0.01 230);
}

[data-theme="dark"] {
  --color-primary: oklch(0.66 0.15 255);
  --color-primary-hover: oklch(0.72 0.16 255);
  --color-primary-foreground: oklch(0.12 0.02 255);
  --color-secondary: oklch(0.64 0.08 230);
  --color-secondary-hover: oklch(0.70 0.09 230);
  --color-accent: oklch(0.72 0.14 165);
  --color-accent-foreground: oklch(0.12 0.02 165);
  --color-surface: oklch(0.17 0.02 255);
  --color-surface-tinted: oklch(0.22 0.02 255);
  --color-surface-elevated: oklch(0.25 0.02 255);
  --color-on-surface: oklch(0.96 0.01 255);
  --color-on-surface-muted: oklch(0.70 0.02 255);
  --color-on-primary: var(--color-primary-foreground);
  --color-border: oklch(0.33 0.02 255);
  --color-border-strong: oklch(0.48 0.03 255);
  --color-error: oklch(0.68 0.18 25);
  --color-error-foreground: oklch(0.12 0.02 25);
  --color-success: oklch(0.72 0.16 145);
  --color-success-foreground: oklch(0.12 0.02 145);
  --color-warning: oklch(0.82 0.15 80);
  --color-warning-foreground: oklch(0.18 0.03 80);
  --color-info: oklch(0.72 0.15 230);
  --color-info-foreground: oklch(0.12 0.02 230);
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

html {
  font-family: var(--font-family-sans);
  font-size: 16px;
  line-height: var(--line-height-normal);
  color: var(--color-on-surface);
  background: var(--color-surface);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
}

body {
  margin: 0;
  min-height: 100dvh;
}

h1,
h2,
h3,
h4,
h5,
h6 {
  margin: 0;
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-tight);
}

p {
  margin: 0;
  line-height: var(--line-height-relaxed);
}

a {
  color: var(--color-primary);
  text-decoration: none;
  transition: color var(--duration-quick) var(--ease-out-quart);
}

a:hover {
  color: var(--color-primary-hover);
}

button {
  font: inherit;
  cursor: pointer;
  border: 0;
  background: none;
}

img,
svg,
video {
  display: block;
  max-width: 100%;
  height: auto;
}

:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 1ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 1ms !important;
    scroll-behavior: auto !important;
  }
}
