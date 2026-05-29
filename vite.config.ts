import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Tauri desktop app. Vite is used only to compile the WebView assets.
// Development commands build static files for Tauri instead of starting a hosted app.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Tauri uses Chromium (Windows) / WebKit (macOS, Linux)
    target: ['chrome105', 'safari15'],
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
