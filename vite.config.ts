import { readFile } from "node:fs/promises"
import { fileURLToPath, URL } from "node:url"
import { defineConfig } from "vite"
import type { Plugin } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { BUNDLED_LANGUAGE_IDS, filterShikiLanguageList } from "./src/lib/diff/bundled-languages"

const SHIKI_WASM_STUB = "\0shiki-wasm-stub"

/**
 * Cuts Shiki down to what the diff view uses (docs/design/diff-view.md "Languages"): the grammars of
 * `BUNDLED_LANGUAGE_IDS`, no bundled theme (the diffs have their own), and no WebAssembly regex engine (the JavaScript
 * one is preferred): about 6.5 MB less in `dist`.
 */
function shikiBundle(): Plugin {
  return {
    name: "shiki-bundle",
    enforce: "pre",
    resolveId(source) {
      return source === "shiki/wasm" ? SHIKI_WASM_STUB : null
    },
    async load(id) {
      // the dev server adds a version query
      const path = id.replace(/\?.*$/, "")
      if (path === SHIKI_WASM_STUB) {
        return "export default () => { throw new Error(\"The WebAssembly regex engine of Shiki is not bundled\") }"
      }
      if (/[/\\]shiki[/\\]dist[/\\]langs-bundle-full-[^/\\]+\.mjs$/.test(path)) {
        return filterShikiLanguageList(await readFile(path, "utf8"), BUNDLED_LANGUAGE_IDS)
      }
      // Shiki's themes and the collection of them in @pierre/theming: the diffs register their own (islands-themes.ts)
      if (/[/\\]shiki[/\\]dist[/\\]themes\.mjs$/.test(path)) {
        return "const bundledThemesInfo = []\nconst bundledThemes = {}\nexport { bundledThemes, bundledThemesInfo }\n"
      }
      if (/[/\\]@pierre[/\\]theming[/\\]dist[/\\]collections[/\\]shiki\.js$/.test(path)) {
        return "import { createThemeCollection } from \"../modules/createThemeCollection.js\"\nexport const shikiThemes = createThemeCollection({ themes: Object.freeze([]) })\n"
      }
      return null
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), shikiBundle()],
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
    plugins: () => [shikiBundle()],
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
    // the dev server pre-bundles @pierre/diffs with shiki in it, without the plugins above
    rolldownOptions: {
      plugins: [shikiBundle()],
    },
  },
  clearScreen: false,
})
