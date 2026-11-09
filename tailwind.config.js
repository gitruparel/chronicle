/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: 'var(--bg-primary)',
        surface: 'var(--bg-surface)',
        'surface-hover': 'var(--bg-surface-hover)',
        border: 'var(--border)',
        'border-hover': 'var(--border-hover)',
        text: {
          primary: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          muted: 'var(--text-muted)',
        },
        accent: {
          DEFAULT: '#3B82F6',
          hover: '#2563EB',
          subtle: 'rgba(59, 130, 246, 0.1)',
        },
        success: {
          DEFAULT: '#10B981',
          subtle: 'rgba(16, 185, 129, 0.1)',
        },
        warning: {
          DEFAULT: '#F59E0B',
          subtle: 'rgba(245, 158, 11, 0.1)',
        },
        danger: {
          DEFAULT: '#EF4444',
          subtle: 'rgba(239, 68, 68, 0.1)',
        }
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
        mono: ['JetBrains Mono', 'Consolas', 'Courier New', 'monospace'],
      },
    },
  },
  plugins: [],
}
