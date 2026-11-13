import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
// @ts-expect-error JavaScript server module shared with the production container.
import { createMarkets } from './api/markets.mjs';
export default defineConfig({
  plugins: [react(), tailwindcss(), { name: 'market-api', configureServer(server) { const markets = createMarkets(); server.middlewares.use((req, res, next) => req.url?.startsWith('/api/markets/') ? markets(req, res) : next()); } }],
  server: { proxy: { '/api': 'http://127.0.0.1:8787' } },
  build: { chunkSizeWarningLimit: 900 },
});
