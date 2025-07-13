// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  // Enable Nuxt 4 features
  future: {
    compatibilityVersion: 4,
  },

  // Compatibility date for Nitro
  compatibilityDate: "2025-07-09",

  // Disable SSR for Tauri desktop app
  ssr: false,

  // Enable devtools
  devtools: {enabled: true},

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
        {charset: "utf-8"},
        {name: "viewport", content: "width=device-width, initial-scale=1"},
      ],
    },
  },
})