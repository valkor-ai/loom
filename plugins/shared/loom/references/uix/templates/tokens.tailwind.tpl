/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{html,js,jsx,ts,tsx,vue,svelte}', './app/**/*.{js,jsx,ts,tsx}'],
  darkMode: ['class', '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: 'oklch(0.48 0.15 255)',
          hover: 'oklch(0.42 0.17 255)',
          foreground: 'oklch(0.99 0.01 255)',
        },
        secondary: {
          DEFAULT: 'oklch(0.62 0.08 230)',
          hover: 'oklch(0.56 0.10 230)',
        },
        accent: {
          DEFAULT: 'oklch(0.68 0.13 165)',
          foreground: 'oklch(0.12 0.02 165)',
        },
        surface: {
          DEFAULT: 'oklch(0.99 0.004 255)',
          tinted: 'oklch(0.96 0.01 255)',
          elevated: 'oklch(1 0 0)',
        },
        'on-surface': {
          DEFAULT: 'oklch(0.18 0.02 255)',
          muted: 'oklch(0.46 0.02 255)',
        },
        border: {
          DEFAULT: 'oklch(0.88 0.01 255)',
          strong: 'oklch(0.68 0.02 255)',
        },
        error: {
          DEFAULT: 'oklch(0.55 0.20 25)',
          foreground: 'oklch(0.99 0.01 25)',
        },
        success: {
          DEFAULT: 'oklch(0.58 0.16 145)',
          foreground: 'oklch(0.99 0.01 145)',
        },
        warning: {
          DEFAULT: 'oklch(0.76 0.15 80)',
          foreground: 'oklch(0.22 0.03 80)',
        },
        info: {
          DEFAULT: 'oklch(0.58 0.15 230)',
          foreground: 'oklch(0.99 0.01 230)',
        },
      },
      fontFamily: {
        sans: ['"Noto Sans SC"', '"PingFang SC"', '"Microsoft YaHei"', 'Geist', 'system-ui', 'sans-serif'],
        serif: ['"Source Han Serif SC"', '"Noto Serif SC"', '"Songti SC"', 'Georgia', 'serif'],
        mono: ['"JetBrains Mono"', '"Fira Code"', '"Cascadia Code"', '"SF Mono"', 'monospace'],
      },
      fontSize: {
        xs: ['0.75rem', { lineHeight: '1.5' }],
        sm: ['0.875rem', { lineHeight: '1.5' }],
        base: ['1rem', { lineHeight: '1.5' }],
        lg: ['1.125rem', { lineHeight: '1.4' }],
        xl: ['1.25rem', { lineHeight: '1.3' }],
        '2xl': ['1.5rem', { lineHeight: '1.2' }],
        '3xl': ['2rem', { lineHeight: '1.15' }],
        '4xl': ['2.5rem', { lineHeight: '1.1' }],
        '5xl': ['3.5rem', { lineHeight: '1.05' }],
      },
      spacing: {
        18: '4.5rem',
        88: '22rem',
        128: '32rem',
        'shell-sidebar': '240px',
        'shell-sidebar-compact': '64px',
        'shell-topbar': '56px',
        'shell-detail': '380px',
        'table-min': '760px',
      },
      borderRadius: {
        none: '0',
        xs: '2px',
        sm: '4px',
        md: '8px',
        lg: '12px',
        xl: '16px',
        '2xl': '24px',
        '3xl': '32px',
        full: '9999px',
      },
      boxShadow: {
        xs: '0 1px 2px rgb(16 24 40 / 0.04)',
        sm: '0 1px 3px rgb(16 24 40 / 0.08), 0 1px 2px rgb(16 24 40 / 0.06)',
        md: '0 4px 8px rgb(16 24 40 / 0.10)',
        lg: '0 12px 24px rgb(16 24 40 / 0.12)',
        xl: '0 20px 40px rgb(16 24 40 / 0.16)',
        '2xl': '0 28px 56px rgb(16 24 40 / 0.20)',
        inner: 'inset 0 2px 4px rgb(16 24 40 / 0.06)',
      },
      transitionDuration: {
        fast: '100ms',
        quick: '150ms',
        base: '200ms',
        medium: '300ms',
        slow: '400ms',
        slower: '600ms',
      },
      transitionTimingFunction: {
        'out-quart': 'cubic-bezier(0.16, 1, 0.3, 1)',
        'out-quint': 'cubic-bezier(0.22, 1, 0.36, 1)',
        'out-expo': 'cubic-bezier(0.19, 1, 0.22, 1)',
        'out-cubic': 'cubic-bezier(0.33, 1, 0.68, 1)',
        'in-quart': 'cubic-bezier(0.5, 0, 0.75, 0)',
        'in-cubic': 'cubic-bezier(0.32, 0, 0.67, 0)',
      },
      screens: {
        '3xl': '1920px',
      },
      maxWidth: {
        prose: '72ch',
        '8xl': '1536px',
      },
      minHeight: {
        touch: '44px',
        'touch-android': '48px',
        'row-compact': '40px',
        row: '48px',
        'control-sm': '32px',
        control: '40px',
        'control-lg': '48px',
      },
      minWidth: {
        touch: '44px',
        'touch-android': '48px',
        table: '760px',
      },
      width: {
        'shell-sidebar': '240px',
        'shell-sidebar-compact': '64px',
        'shell-detail': '380px',
        drawer: 'min(420px, 92vw)',
      },
      ringColor: {
        focus: 'oklch(0.48 0.15 255 / 0.28)',
      },
      ringWidth: {
        focus: '3px',
      },
      zIndex: {
        hide: '-1',
        base: '0',
        raised: '1',
        dropdown: '10',
        sticky: '20',
        fixed: '30',
        modal: '50',
        popover: '60',
        tooltip: '70',
        notification: '80',
      },
    },
  },
  plugins: [
    // Enable only when these dependencies already exist in package.json:
    // require('@tailwindcss/forms')({ strategy: 'class' }),
    // require('@tailwindcss/typography'),
  ],
};
