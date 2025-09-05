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
        "@tauri-apps/api/window",
        "@tauri-apps/api/path",
        "tailwindcss/colors",
        "@vueuse/core",
        "@tauri-apps/plugin-log",
        "@tauri-apps/api/app",
        "@tauri-apps/api/core",
        "@git-diff-view/vue",
        "@tanstack/vue-table",
        "reka-ui",
        "p-debounce",
        "zod",
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

  // Disable devtools to avoid EBADF error
  devtools: { enabled: false },

  // Extend from layers
  extends: ["./layers/core", "./layers/shared-ui", "./layers/conflict-ui", "./layers/commit-ui", "./layers/ai", "./layers/branch-ui"],

  modules: ["@nuxt/ui", "@nuxt/eslint"],

  css: ["~/assets/css/main.css"],

  // TypeScript
  typescript: {
    strict: true,
    shim: false,
    tsConfig: {
      vueCompilerOptions: {
        strictTemplates: true,
        dataAttributes: ["data-*", "autocapitalize", "autocorrect", "spellcheck"],
      },
    },
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

  // Component configuration
  components: [
    { path: "~/components/unassigned", pathPrefix: false },
    "~/components",
  ],

  // Runtime config for test mode
  runtimeConfig: {
    public: {
      testMode: false, // Default value, overridden by NUXT_PUBLIC_TEST_MODE env var
    },
  },
})
