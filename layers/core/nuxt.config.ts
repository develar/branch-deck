// Core layer - provides base utilities and stores for all other layers
export default defineNuxtConfig({
  // Auto-import stores and utilities
  imports: {
    dirs: ["stores", "utils"],
  },
})