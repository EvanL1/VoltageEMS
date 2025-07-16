/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{vue,js,ts,jsx,tsx}",
    "./public/index.html",
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#ecf5ff',
          100: '#d9ebff',
          200: '#b3d8ff',
          300: '#85c1ff',
          400: '#56a9ff',
          500: '#409eff',
          600: '#1890ff',
          700: '#0969da',
          800: '#0848a3',
          900: '#062d6a',
        },
        dark: {
          50: '#f7f8f9',
          100: '#eef0f3',
          200: '#dde1e7',
          300: '#bfcbd9',
          400: '#93a2b3',
          500: '#6c7983',
          600: '#4e5969',
          700: '#3a4654',
          800: '#2c3645',
          900: '#1a2332',
        }
      },
      animation: {
        'fade-in': 'fadeIn 0.5s ease-in-out',
        'slide-in-left': 'slideInLeft 0.3s ease-out',
        'slide-in-right': 'slideInRight 0.3s ease-out',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideInLeft: {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(0)' },
        },
        slideInRight: {
          '0%': { transform: 'translateX(100%)' },
          '100%': { transform: 'translateX(0)' },
        },
      },
    },
  },
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/typography'),
  ],
}