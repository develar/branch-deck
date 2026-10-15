// @spec spec/commit/commit-files.spec.md
import { EXTENSION_TO_FILE_FORMAT, getFiletypeFromFileName } from "@pierre/diffs"
import { bundledLanguagesInfo } from "shiki"
import { describe, expect, it } from "vitest"
import { BUNDLED_LANGUAGE_IDS, filterShikiLanguageList, getUnbundledExtensions } from "@/lib/diff/bundled-languages"
import { CUSTOM_EXTENSIONS } from "@/lib/diff/diff-languages"

// Shiki's language list as the build reads it (`shikiBundle` in vite.config.ts)
const shikiLanguageLists = import.meta.glob<string>("/node_modules/shiki/dist/langs-bundle-full-*.mjs", { query: "?raw", import: "default", eager: true })

function readShikiLanguageList(): string {
  const [source] = Object.values(shikiLanguageLists)
  expect(source, "Shiki's language list").toBeDefined()
  return source!
}

/** What `bundledLanguages` of the build has: the bundled ids and their aliases */
const bundled: Record<string, true> = Object.fromEntries(bundledLanguagesInfo
  .filter(info => BUNDLED_LANGUAGE_IDS.includes(info.id))
  .flatMap(info => [info.id, ...(info.aliases ?? [])])
  .map(name => [name, true]))

describe("the bundled Shiki languages", () => {
  it("keeps only the bundled entries of Shiki's language list, so the other grammars leave the build", () => {
    const filtered = filterShikiLanguageList(readShikiLanguageList(), BUNDLED_LANGUAGE_IDS)
    const imports = [...filtered.matchAll(/import\("@shikijs\/langs\/([^"]+)"\)/g)].map(match => match[1])
    expect(imports.toSorted()).toEqual([...BUNDLED_LANGUAGE_IDS].toSorted())
    expect(filtered).toContain("const bundledLanguages = {")
    expect(filtered).not.toContain("emacs-lisp")
  })

  it("fails the build for an id Shiki does not have", () => {
    expect(() => filterShikiLanguageList(readShikiLanguageList(), ["kotlin", "freemarker"])).toThrow("Not in Shiki's language list: freemarker")
  })

  it.each([
    "Foo.kt", "build.gradle.kts", "build.gradle", "Foo.java", "Foo.groovy", "plugin.xml", "intellij.platform.core.iml",
    "messages.properties", "package.json", "tsconfig.json", "config.yaml", "Cargo.toml", "Cargo.lock", "README.md",
    "App.tsx", "index.ts", "main.js", "style.css", "main.rs", "main.go", "main.py", "main.cpp", "Foo.swift", "Foo.cs",
    "index.php", "app.rb", "query.sql", "api.proto", "run.sh", "Dockerfile", "Makefile", "login.feature", ".gitignore",
  ])("highlights %s with a bundled grammar", (fileName) => {
    const lang = getFiletypeFromFileName(fileName)
    expect(lang === "text" || Object.hasOwn(bundled, lang), `${fileName}: ${lang}`).toBe(true)
  })

  it("maps our custom extensions to bundled grammars", () => {
    for (const lang of Object.values(CUSTOM_EXTENSIONS)) {
      expect(Object.hasOwn(bundled, lang), lang).toBe(true)
    }
  })

  it("shows a file of a grammar the build leaves out as plain text", () => {
    const unbundled = new Set(getUnbundledExtensions(EXTENSION_TO_FILE_FORMAT, bundled))
    expect(unbundled.has("el")).toBe(true)
    expect(unbundled.has("cob")).toBe(true)
    // FreeMarker is mapped by Pierre, but Shiki has no grammar for it
    expect(unbundled.has("ftl")).toBe(true)
    expect(unbundled.has("kt")).toBe(false)
    expect(unbundled.has("properties")).toBe(false)
  })
})
