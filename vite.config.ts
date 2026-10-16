import { readFile } from "node:fs/promises"
import { fileURLToPath, URL } from "node:url"
import { defineConfig, lazyPlugins } from "vite-plus"
import type { Plugin } from "vite-plus"
import babel from "@rolldown/plugin-babel"
import react, { reactCompilerPreset } from "@vitejs/plugin-react"
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

/**
 * Preloads the upright InterVariable face (`src/index.css`): without it the browser finds the font only after the CSS
 * applies, and every window paints first in the system font and lays out again when Inter arrives. The build only:
 * the dev server serves the font from its package.
 */
function preloadInterFont(): Plugin {
  return {
    name: "preload-inter-font",
    transformIndexHtml: {
      order: "post",
      handler(_html, context) {
        const font = Object.values(context.bundle ?? {}).find(it => it.type === "asset" && /(^|\/)InterVariable-[\w-]+\.woff2$/.test(it.fileName) && !it.fileName.includes("Italic"))
        if (font === undefined) {
          return []
        }
        return [{ tag: "link", attrs: { rel: "preload", href: `/${font.fileName}`, as: "font", type: "font/woff2", crossorigin: true }, injectTo: "head" }]
      },
    },
  }
}

export default defineConfig({
  // React Compiler memoizes components and hooks (docs/design/rendering-performance.md); a component it would skip
  // fails the build instead of running unmemoized
  // lazy: `vp lint` and `vp fmt` read this file too and need none of the plugins
  plugins: lazyPlugins(() => [react(), babel({ presets: [reactCompilerPreset({ panicThreshold: "all_errors" })] }), tailwindcss(), shikiBundle(), preloadInterFont()]),
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
    // kB; the app reads its chunks from disk, not over a network. Catches a diff view pulled into the main chunk
    // (~600 + ~550 kB), not the lazy C++ grammar (~790 kB)
    chunkSizeWarningLimit: 1000,
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
  // Oxlint: the correctness category, the rules of the former ESLint presets (eslint, typescript-eslint, react-hooks,
  // react-refresh) that Oxlint files elsewhere, and the type-aware rules of tsgolint (TypeScript 7)
  lint: {
    plugins: ["eslint", "typescript", "unicorn", "oxc", "react"],
    categories: {
      correctness: "error",
    },
    options: {
      typeAware: true,
      typeCheck: true,
      reportUnusedDisableDirectives: "error",
    },
    ignorePatterns: [
      "src/lib/bindings.ts",
      "dist/**",
      "target/**",
      "src-tauri/target/**",
      "tests/results/**",
      "tests/test-repos/**",
      "playwright-report/**",
      "**/.tools/**",
      // agent worktrees are separate checkouts with their own tsconfig
      ".claude/**",
    ],
    rules: {
      "no-case-declarations": "error",
      "no-empty": "error",
      "no-fallthrough": "error",
      "no-prototype-builtins": "error",
      "no-regex-spaces": "error",
      "no-unexpected-multiline": "error",
      "no-useless-assignment": "error",
      "no-useless-constructor": "error",
      "no-var": "error",
      "preserve-caught-error": "error",
      "prefer-const": "error",
      "prefer-rest-params": "error",
      "prefer-spread": "error",
      "curly": ["error", "all"],
      "no-unused-vars": ["error", {
        args: "after-used",
        argsIgnorePattern: "^_",
        ignoreRestSiblings: true,
        vars: "all",
        varsIgnorePattern: "^_",
      }],
      "typescript/ban-ts-comment": ["error", { minimumDescriptionLength: 10 }],
      "typescript/consistent-type-imports": ["error", { disallowTypeAnnotations: false, prefer: "type-imports" }],
      "typescript/no-array-constructor": "error",
      "typescript/no-dynamic-delete": "error",
      "typescript/no-empty-object-type": "error",
      "typescript/no-explicit-any": "error",
      "typescript/no-extraneous-class": "error",
      "typescript/no-import-type-side-effects": "error",
      "typescript/no-invalid-void-type": "error",
      "typescript/no-misused-promises": "error",
      "typescript/no-namespace": "error",
      "typescript/no-non-null-asserted-nullish-coalescing": "error",
      "typescript/no-require-imports": "error",
      "typescript/no-unnecessary-type-constraint": "error",
      "typescript/no-unsafe-function-type": "error",
      "typescript/prefer-literal-enum-member": "error",
      "typescript/unified-signatures": "error",
    },
    overrides: [
      {
        // React Compiler: a suppressed rule of React makes it skip the function, so suppressing one is an error too
        files: ["src/**/*.{ts,tsx}"],
        rules: {
          "react/rules-of-hooks": "error",
          "react/rule-suppression": "error",
          "react/unsupported-syntax": "error",
        },
      },
      {
        files: ["src/**/*.tsx"],
        rules: {
          "react/only-export-components": ["error", { allowConstantExport: true, allowCompoundComponents: true }],
        },
      },
      {
        // generated shadcn/ui components export variant helpers (cva) and the toast manager next to components
        files: ["src/components/ui/**/*.tsx"],
        rules: {
          "react/only-export-components": "off",
        },
      },
    ],
  },
})
