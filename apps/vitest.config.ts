import { fileURLToPath } from 'node:url'
import { mergeConfig, defineConfig, configDefaults } from 'vitest/config'
import viteConfig from './vite.config'

export default defineConfig(async (env) => {
  const resolvedViteConfig = typeof viteConfig === 'function' ? await viteConfig(env) : viteConfig

  return mergeConfig(
    resolvedViteConfig,
    defineConfig({
      test: {
        environment: 'jsdom',
        exclude: [...configDefaults.exclude, 'e2e/**'],
        root: fileURLToPath(new URL('./', import.meta.url)),
        coverage: {
          provider: 'v8',
          reporter: ['text', 'json', 'json-summary', 'html', 'lcov'],
          reportsDirectory: './coverage',
          include: ['src/**/*.{js,ts,vue}'],
          exclude: [
            'src/**/*.d.ts',
            'src/**/*.test.{js,ts}',
            'src/**/*.spec.{js,ts}',
            'src/main.ts',
          ],
        },
        css: {
          modules: {
            classNameStrategy: 'non-scoped',
          },
        },
      },
    }),
  )
})
