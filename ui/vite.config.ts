import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import { licenses } from './licenses.ts'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte(), licenses()],
  // The app's CSP has no data: source, so fonts must stay files instead of being inlined.
  build: { assetsInlineLimit: 0 },
})
