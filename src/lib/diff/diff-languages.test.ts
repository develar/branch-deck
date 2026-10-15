// @spec spec/commit/commit-files.spec.md
import { getFiletypeFromFileName, resolveLanguage } from "@pierre/diffs"
import type { SupportedLanguages } from "@pierre/diffs"
import { describe, expect, it } from "vitest"
import { CUSTOM_EXTENSIONS, registerDiffLanguages } from "@/lib/diff/diff-languages"

registerDiffLanguages()

// loads the grammar as the highlighter does, so a missing one fails
async function expectGrammar(lang: SupportedLanguages) {
  expect(lang).not.toBe("text")
  const resolved = await resolveLanguage(lang as Exclude<SupportedLanguages, "text" | "ansi">)
  expect(resolved.data.length).toBeGreaterThan(0)
}

describe("diff languages", () => {
  it("maps our custom extensions to grammars Shiki has", async () => {
    for (const lang of new Set(Object.values(CUSTOM_EXTENSIONS))) {
      await expectGrammar(lang)
    }
  })

  it.each([
    ["Foo.kt", "kotlin"],
    ["build.gradle.kts", "kts"],
    ["build.gradle", "groovy"],
    ["intellij.platform.core.iml", "xml"],
    ["src/main.rs", "rust"],
    ["Cargo.lock", "toml"],
    ["src/App.tsx", "tsx"],
    ["Foo.java", "java"],
    ["messages.properties", "properties"],
    ["api.proto", "protobuf"],
    ["Dockerfile", "dockerfile"],
    ["docs/README.md", "markdown"],
  ] as const)("highlights %s as %s", async (fileName, lang) => {
    expect(getFiletypeFromFileName(fileName)).toBe(lang)
    await expectGrammar(lang)
  })
})
