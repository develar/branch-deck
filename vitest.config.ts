import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite-plus"

export default defineConfig({
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    include: ["src/**/*.test.ts", "tests/e2e/setup/**/*.test.ts"],
    environment: "node",
  },
})
