// @spec spec/commit/commit-files.spec.md
import { bundledLanguagesInfo } from "shiki"
import { describe, expect, it } from "vite-plus/test"
import { BUNDLED_LANGUAGE_IDS, filterShikiLanguageList } from "@/lib/diff/bundled-languages"
import { CUSTOM_EXTENSIONS, EXTENSION_LANGUAGES, languageOfPath } from "@/lib/diff/file-languages"

// Shiki's language list as the build reads it (`shikiBundle` in vite.config.ts)
const shikiLanguageLists = import.meta.glob<string>("/node_modules/shiki/dist/langs-bundle-full-*.mjs", { query: "?raw", import: "default", eager: true })

function readShikiLanguageList(): string {
  const [source] = Object.values(shikiLanguageLists)
  expect(source, "Shiki's language list").toBeDefined()
  return source!
}

/** What `bundledLanguages` of the build has: the bundled ids and their aliases */
const bundled: Record<string, true> = Object.fromEntries(
  bundledLanguagesInfo
    .filter(info => BUNDLED_LANGUAGE_IDS.includes(info.id))
    .flatMap(info => [info.id, ...(info.aliases ?? [])])
    .map(name => [name, true]),
)

describe("the bundled Shiki languages", () => {
  it("keeps only the bundled entries of Shiki's language list, so the other grammars leave the build", () => {
    const filtered = filterShikiLanguageList(readShikiLanguageList(), BUNDLED_LANGUAGE_IDS)
    const imports = [...filtered.matchAll(/import\("@shikijs\/langs\/([^"]+)"\)/g)].map(match => match[1] ?? "")
    expect(imports.toSorted()).toEqual([...BUNDLED_LANGUAGE_IDS].toSorted())
    expect(filtered).toContain("const bundledLanguages = {")
    expect(filtered).not.toContain("emacs-lisp")
  })

  it("fails the build for an id Shiki does not have", () => {
    expect(() => filterShikiLanguageList(readShikiLanguageList(), ["kotlin", "freemarker"])).toThrow("Not in Shiki's language list: freemarker")
  })

  it.each([
    "Foo.kt",
    "build.gradle.kts",
    "build.gradle",
    "Foo.java",
    "Foo.groovy",
    "plugin.xml",
    "intellij.platform.core.iml",
    "messages.properties",
    "package.json",
    "tsconfig.json",
    "config.yaml",
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "App.tsx",
    "index.ts",
    "main.js",
    "style.css",
    "main.rs",
    "main.go",
    "main.py",
    "main.cpp",
    "Foo.swift",
    "Foo.cs",
    "index.php",
    "app.rb",
    "query.sql",
    "api.proto",
    "run.sh",
    "Dockerfile",
    "Makefile",
    "login.feature",
    ".gitignore",
  ])("highlights %s with a bundled grammar", fileName => {
    const lang = languageOfPath(fileName)
    expect(lang === "text" || Object.hasOwn(bundled, lang), `${fileName}: ${lang}`).toBe(true)
  })

  it("maps the extension table and our custom extensions to bundled grammars only", () => {
    for (const lang of [...Object.values(EXTENSION_LANGUAGES), ...Object.values(CUSTOM_EXTENSIONS)]) {
      expect(Object.hasOwn(bundled, lang), lang).toBe(true)
    }
  })

  it("shows a file of a grammar the build leaves out as plain text", () => {
    expect(["init.el", "PAYROLL.cob", "page.ftl"].map(languageOfPath)).toEqual(["text", "text", "text"])
    expect(["Foo.kt", "messages.properties"].map(languageOfPath)).toEqual(["kotlin", "properties"])
  })
})
