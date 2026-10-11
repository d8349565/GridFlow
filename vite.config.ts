import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({
  plugins: [react()],
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**','**/release/**','**/.cache/**','**/.tools/**','**/.cargo-home/**','**/data/**','**/test-results/**'] } },
  build: { target: 'es2022', chunkSizeWarningLimit: 1300 },
  clearScreen: false,
});
