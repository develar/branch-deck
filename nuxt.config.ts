// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  // Enable Nuxt 4 features
  future: {
    compatibilityVersion: 4,
  },

  vite: {
    optimizeDeps: {
      // avoid "optimized dependencies changed. reloading"
      force: true,

      include: [
        "@tauri-apps/plugin-store",
        "@tauri-apps/api/webviewWindow",
        "@tauri-apps/api/event",
        "tailwindcss/colors",
        "@vueuse/core",
        "@tauri-apps/plugin-log",
        "@tauri-apps/api/app",
        "@tauri-apps/plugin-dialog",
        "@tauri-apps/api/core",
        "@git-diff-view/vue",
        "reka-ui",
      ],
    },
  },

  nitro: {
    preset: "static",
  },

  // Compatibility date for Nitro
  compatibilityDate: "2025-07-09",

  // Disable SSR for Tauri desktop app
  ssr: false,

  // Enable devtools
  devtools: { enabled: true },

  // Modules
  modules: ["@nuxt/ui-pro", "@nuxt/eslint", "@nuxt/icon"],

  css: ["~/assets/css/main.css"],

  // TypeScript
  typescript: {
    strict: true,
    shim: false,
  },

  devServer: {
    port: 1420,
  },

  // Avoids error [unhandledRejection] EMFILE: too many open files, watch
  ignore: ["**/src-tauri/**"],

  telemetry: false,

  app: {
    head: {
      title: "Branch Deck",
      meta: [
        { charset: "utf-8" },
        { name: "viewport", content: "width=device-width, initial-scale=1" },
      ],
    },
  },
})
