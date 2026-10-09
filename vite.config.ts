import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite"
import vue from "@vitejs/plugin-vue"
import ui from "@nuxt/ui/vite"

export default defineConfig({
  plugins: [
    vue(),
    ui({
      // all components and composables are imported explicitly
      autoImport: false,
      components: false,
      dts: false,
      ui: {
        colors: {
          primary: "green",
        },
        toast: {
          slots: {
            // make text selectable
            description: "text-sm text-muted select-text",
          },
        },
      },
      icon: {
        clientBundle: {
          // bundle every icon referenced in the sources, so nothing is fetched from the Iconify API at runtime
          scan: {
            globInclude: ["app/**/*.{vue,ts}", "layers/**/*.{vue,ts}"],
          },
        },
      },
    }),
  ],
  resolve: {
    alias: {
      "~": fileURLToPath(new URL("./app", import.meta.url)),
      "@layers": fileURLToPath(new URL("./layers", import.meta.url)),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // avoids EMFILE: too many open files
      ignored: ["**/src-tauri/**", "**/target/**", "**/crates/**"],
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  optimizeDeps: {
    // The app (reka-ui 2.11) and Nuxt UI (pinned reka-ui 2.10) use different reka-ui versions. The optimizer keys
    // deps by bare name and would mix both copies, breaking provide/inject between reka-ui components in dev.
    // Serving reka-ui unbundled resolves it per importer, as the production build does.
    exclude: ["reka-ui"],
    include: [
      "@tauri-apps/plugin-store",
      "@tauri-apps/api/webviewWindow",
      "@tauri-apps/api/event",
      "@tauri-apps/api/window",
      "@tauri-apps/api/path",
      "tailwindcss/colors",
      "@vueuse/core",
      "@tauri-apps/plugin-log",
      "@tauri-apps/api/app",
      "@tauri-apps/api/core",
      "@git-diff-view/vue",
      "@tanstack/vue-table",
      "p-debounce",
      "zod",
    ],
  },
  clearScreen: false,
})
