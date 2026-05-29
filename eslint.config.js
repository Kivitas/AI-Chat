import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

const webViewGlobals = {
  File: 'readonly',
  FileReader: 'readonly',
  HTMLElement: 'readonly',
  MouseEvent: 'readonly',
  clearTimeout: 'readonly',
  confirm: 'readonly',
  document: 'readonly',
  getComputedStyle: 'readonly',
  localStorage: 'readonly',
  navigator: 'readonly',
  performance: 'readonly',
  setTimeout: 'readonly',
  window: 'readonly',
}

export default defineConfig([
  globalIgnores(['dist', 'src-tauri/target', 'node_modules']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      globals: webViewGlobals,
    },
  },
])
