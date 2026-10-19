// @spec spec/commit/commit-files.spec.md
import { beforeAll, describe, expect, it } from "vite-plus/test"
import { CUSTOM_EXTENSIONS, languageOfPath } from "@/lib/diff/file-languages"
import type { GrammarSource } from "@/lib/diff/highlight/engine"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"

let source: GrammarSource

beforeAll(async () => {
  source = await createGrammarSource()
})

// loads the grammar as the highlight worker does, so a missing one fails
async function expectGrammar(lang: string) {
  expect(lang).not.toBe("text")
  expect(await source.grammar(lang), lang).not.toBeNull()
}

describe("file languages", () => {
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
    ["docker/Dockerfile", "dockerfile"],
    ["docs/README.md", "markdown"],
    ["src/app/app.component.ts", "angular-ts"],
  ] as const)("highlights %s as %s", async (fileName, lang) => {
    expect(languageOfPath(fileName)).toBe(lang)
    await expectGrammar(lang)
  })

  it("shows a file without a known name as plain text", () => {
    expect([languageOfPath("LICENSE"), languageOfPath("notes.unknown")]).toEqual(["text", "text"])
  })
})
