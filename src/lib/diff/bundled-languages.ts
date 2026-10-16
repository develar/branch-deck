/*
 * The Shiki grammars the app bundles (docs/design/diff-view.md "Languages"). `vite.config.ts` cuts Shiki's language
 * list down to them, so the other ~150 grammars (Emacs Lisp, Wolfram, COBOL, …) are not in `dist`; a file of one of
 * them is shown as plain text (`getUnbundledExtensions`). No imports: `vite.config.ts` loads this module in Node.
 */

/** Shiki language ids; their aliases (`kt`, `kts`, `sh`, `yml`, …) come with them, and so do the grammars they embed */
export const BUNDLED_LANGUAGE_IDS: readonly string[] = [
  // JVM, the build, the data and the configuration of a JetBrains project
  "kotlin",
  "java",
  "groovy",
  "scala",
  "xml",
  "xsl",
  "json",
  "jsonc",
  "json5",
  "jsonl",
  "yaml",
  "toml",
  "ini",
  "markdown",
  "csv",
  "tsv",
  "proto",
  "graphql",
  "sql",
  "regexp",
  "mermaid",
  "diff",
  // the web
  "html",
  "css",
  "scss",
  "sass",
  "less",
  "javascript",
  "typescript",
  "jsx",
  "tsx",
  "vue",
  "svelte",
  "angular-ts",
  "angular-html",
  "handlebars",
  "jinja",
  "twig",
  "http",
  // the languages of the other IDEs and their test data
  "python",
  "rust",
  "go",
  "c",
  "cpp",
  "objective-c",
  "objective-cpp",
  "swift",
  "csharp",
  "fsharp",
  "razor",
  "vb",
  "php",
  "ruby",
  "dart",
  "lua",
  "r",
  "perl",
  "haskell",
  "erlang",
  "elixir",
  "clojure",
  "zig",
  "gherkin",
  "glsl",
  // scripts, the build and the infrastructure
  "shellscript",
  "shellsession",
  "powershell",
  "bat",
  "docker",
  "make",
  "cmake",
  "just",
  "hcl",
  "terraform",
  "nginx",
  "apache",
  "systemd",
  "ssh-config",
  "dotenv",
  // git and the rest of a repository
  "git-commit",
  "git-rebase",
  "codeowners",
  "log",
  "po",
]

/** The entries of Shiki's language list: `\t{ "id": "<id>", …, "import": (() => import("@shikijs/langs/<id>")) }` */
const LANGUAGE_ENTRY = /\t\{\n\t\t"id": "([^"]+)"[\s\S]*?\n\t\}/g

/**
 * Shiki's language list (`shiki/dist/langs-bundle-full-*.mjs`) with only the entries of `ids`: the dynamic imports of
 * the others go, and with them their chunks. Throws when an id is not in the list, so a Shiki upgrade that renames a
 * grammar or changes the file fails the build instead of dropping a language.
 */
export function filterShikiLanguageList(source: string, ids: readonly string[]): string {
  const wanted = new Set(ids)
  const found = new Set<string>()
  const start = source.indexOf("const bundledLanguagesInfo = [\n")
  const end = source.indexOf("\n];", start)
  if (start === -1 || end === -1) {
    throw new Error("Shiki's language list has an unknown layout: update filterShikiLanguageList")
  }
  const head = source.slice(0, start + "const bundledLanguagesInfo = [\n".length)
  const entries = [...source.slice(head.length, end).matchAll(LANGUAGE_ENTRY)].filter(([, id]) => wanted.has(id!) && found.add(id!)).map(([entry]) => entry)
  const missing = ids.filter(id => !found.has(id))
  if (missing.length > 0) {
    throw new Error(`Not in Shiki's language list: ${missing.join(", ")}`)
  }
  return `${head}${entries.join(",\n")}${source.slice(end)}`
}

/**
 * The file names and extensions Pierre maps to a grammar the bundle lacks: they are shown as plain text, as
 * Pierre fails a highlight of a language it cannot resolve
 */
export function getUnbundledExtensions(extensionMap: Readonly<Record<string, string | undefined>>, bundled: Readonly<Record<string, unknown>>): string[] {
  return Object.entries(extensionMap)
    .filter(([, lang]) => lang !== undefined && lang !== "text" && lang !== "ansi" && !Object.hasOwn(bundled, lang))
    .map(([extension]) => extension)
}
