import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite-plus"

export default defineConfig({
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
    // the text of the theme for its contrast test (src/lib/theme-contrast.test.ts): Vitest gives every other CSS import,
    // a `?raw` one too, as an empty string
    css: { include: [/[/\\]src[/\\](index|islands\.generated)\.css/] },
  },
})
