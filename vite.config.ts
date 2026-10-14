import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite"
import type { Plugin } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"

// gives @git-diff-view/lowlight a curated grammar set instead of all of highlight.js, see src/lib/lowlight-grammars.ts;
// only its import is redirected, the shim itself imports the real lowlight
function lowlightGrammars(): Plugin {
  const shim = fileURLToPath(new URL("./src/lib/lowlight-grammars.ts", import.meta.url))
  return {
    name: "lowlight-grammars",
    enforce: "pre",
    resolveId(source, importer) {
      return source === "lowlight" && importer?.includes("@git-diff-view/lowlight") ? shim : null
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), lowlightGrammars()],
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
      "@git-diff-view/react",
      "p-debounce",
      "zod",
    ],
    // the dev server pre-bundles @git-diff-view/react without the Vite plugins, so redirect there too
    rolldownOptions: {
      plugins: [lowlightGrammars()],
    },
  },
  clearScreen: false,
})
