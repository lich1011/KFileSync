/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // Material You (M3) Color System Tokens
        surface: {
          DEFAULT: 'var(--md-surface)',
          dim: 'var(--md-surface-dim)',
          bright: 'var(--md-surface-bright)',
          container: {
            lowest: 'var(--md-surface-container-lowest)',
            low: 'var(--md-surface-container-low)',
            DEFAULT: 'var(--md-surface-container)',
            high: 'var(--md-surface-container-high)',
            highest: 'var(--md-surface-container-highest)',
          },
        },
        'on-surface': {
          DEFAULT: 'var(--md-on-surface)',
          variant: 'var(--md-on-surface-variant)',
          muted: 'var(--md-on-surface-muted)',
        },
        outline: {
          DEFAULT: 'var(--md-outline)',
          variant: 'var(--md-outline-variant)',
        },
        primary: {
          DEFAULT: 'var(--md-primary)',
          container: 'var(--md-primary-container)',
          'on-container': 'var(--md-on-primary-container)',
          foreground: 'var(--md-on-primary)',
        },
        secondary: {
          DEFAULT: 'var(--md-secondary)',
          container: 'var(--md-secondary-container)',
          'on-container': 'var(--md-on-secondary-container)',
          foreground: 'var(--md-on-secondary)',
        },
        tertiary: {
          DEFAULT: 'var(--md-tertiary)',
          container: 'var(--md-tertiary-container)',
          'on-container': 'var(--md-on-tertiary-container)',
          foreground: 'var(--md-on-tertiary)',
        },
        error: {
          DEFAULT: 'var(--md-error)',
          container: 'var(--md-error-container)',
          'on-container': 'var(--md-on-error-container)',
          foreground: 'var(--md-on-error)',
        },
        success: {
          DEFAULT: 'var(--md-success)',
          container: 'var(--md-success-container)',
          'on-container': 'var(--md-on-success-container)',
          foreground: 'var(--md-on-success)',
        },
      },
      borderRadius: {
        'xs': '4px',
        'sm': '8px',
        'md': '12px',
        'lg': '16px',
        'xl': '20px',
        '2xl': '24px',
        '3xl': '28px',
        '4xl': '32px',
      },
      boxShadow: {
        'm3-1': '0px 1px 3px 1px rgba(0, 0, 0, 0.15), 0px 1px 2px 0px rgba(0, 0, 0, 0.30)',
        'm3-2': '0px 2px 6px 2px rgba(0, 0, 0, 0.15), 0px 1px 2px 0px rgba(0, 0, 0, 0.30)',
        'm3-3': '0px 4px 8px 3px rgba(0, 0, 0, 0.15), 0px 1px 3px 0px rgba(0, 0, 0, 0.30)',
        'm3-4': '0px 6px 10px 4px rgba(0, 0, 0, 0.15), 0px 2px 3px 0px rgba(0, 0, 0, 0.30)',
      },
      fontFamily: {
        sans: ['Roboto', 'Inter', 'system-ui', '-apple-system', 'sans-serif'],
      },
      keyframes: {
        radar: {
          '0%': { transform: 'scale(0.9)', opacity: '0.8' },
          '50%': { transform: 'scale(1.25)', opacity: '0.2' },
          '100%': { transform: 'scale(0.9)', opacity: '0.8' },
        },
      },
      animation: {
        radar: 'radar 2.4s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
