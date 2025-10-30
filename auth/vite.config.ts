import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react-swc'

// https://vite.dev/config/
export default defineConfig(({ isSsrBuild }) => {
  // Base configuration for all builds
  const baseConfig = {
    plugins: [react()],
  }

  // SSR builds (server and worker)
  if (isSsrBuild) {
    return {
      ...baseConfig,
      build: {
        ssr: true,
        minify: true,
        rollupOptions: {
          external: ['@stackframe/js'],
          output: {
            format: 'es' as const,
          },
        },
      },
      ssr: {
        target: 'webworker',
        noExternal: ['react', 'react-dom'],
        external: ['@stackframe/js'],
      },
    }
  }

  // Client build
  return {
    ...baseConfig,
    build: {
      rollupOptions: {
        output: {
          format: 'es' as const,
        },
      },
    },
  }
})
