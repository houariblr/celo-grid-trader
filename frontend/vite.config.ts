import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '');

  return {
    plugins: [react(), tailwindcss()],

    define: {
      'process.env.GEMINI_API_KEY': JSON.stringify(env.GEMINI_API_KEY),
    },

    resolve: {
      alias: {
        '@': path.resolve(__dirname, '.'),
      },
    },

    server: {
      port: 3000,
      host: '0.0.0.0',
      // HMR is disabled in AI Studio via DISABLE_HMR env var.
      hmr: process.env.DISABLE_HMR !== 'true',
    },

    build: {
      // Target modern browsers — aligns with Celo's user base (Opera MiniPay, etc.)
      target: 'es2020',

      // Chunk splitting: separate vendor bundles for better caching
      rollupOptions: {
        output: {
          manualChunks: {
            'vendor-react':   ['react', 'react-dom'],
            'vendor-wagmi':   ['wagmi', 'viem', '@wagmi/core'],
            'vendor-charts':  ['recharts'],
            'vendor-motion':  ['motion'],
          },
        },
      },

      // Warn on chunks > 500 kB (Lighthouse performance budget)
      chunkSizeWarningLimit: 500,

      // Generate source maps for production debugging
      sourcemap: mode === 'development',
    },

    // Pre-bundle large deps to avoid slow first-load
    optimizeDeps: {
      include: ['react', 'react-dom', 'wagmi', 'viem', 'recharts', 'motion'],
    },
  };
});
