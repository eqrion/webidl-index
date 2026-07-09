import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [preact()],
  // Relative so the built site works from any subpath (GitHub Pages project
  // site, custom domain, etc.) without knowing the repo name in advance.
  base: './',
  build: {
    // public/data is a dev-only symlink to ../data, so the dev server can
    // serve it at /data alongside the app. In production, CI places the
    // real data/ directory next to dist/ itself (see .github/workflows) --
    // copying it into dist/ here too would just duplicate a directory that
    // only grows over time.
    copyPublicDir: false,
  },
})
