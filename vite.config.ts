import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
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
  // the diff highlighter worker imports shiki chunks lazily
  worker: {
    format: "es",
  },
  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react-dom/client",
      "react-router",
      "zustand",
      "@tauri-apps/plugin-store",
      "@tauri-apps/api/webviewWindow",
      "@tauri-apps/api/event",
      "@tauri-apps/api/window",
      "@tauri-apps/api/path",
      "tailwindcss/colors",
      "@tauri-apps/plugin-log",
      "@tauri-apps/api/app",
      "@tauri-apps/api/core",
      "@pierre/diffs",
      "@pierre/diffs/react",
      "p-debounce",
      "zod",
    ],
  },
  clearScreen: false,
})
