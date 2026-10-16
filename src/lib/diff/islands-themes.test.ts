// @spec spec/commit/commit-files.spec.md
import { getSharedHighlighter, registerCustomTheme } from "@pierre/diffs"
import type { SupportedLanguages } from "@pierre/diffs"
import { describe, expect, it } from "vite-plus/test"
import { ISLANDS_DARK_THEME, ISLANDS_LIGHT_THEME } from "@/lib/diff/islands-themes"

registerCustomTheme("bd-islands-light", () => Promise.resolve(ISLANDS_LIGHT_THEME))
registerCustomTheme("bd-islands-dark", () => Promise.resolve(ISLANDS_DARK_THEME))

/** The colour the diff highlighter (the JavaScript regex engine) gives the first character of `needle` in `code` */
async function colorsOf(code: string, lang: SupportedLanguages, theme = "bd-islands-light"): Promise<(needle: string) => string | undefined> {
  const highlighter = await getSharedHighlighter({ themes: [theme], langs: [lang], preferredHighlighter: "shiki-js" })
  // tokens of one colour are merged, so look a token up by its offset
  // no time limit per line: the first compile of a grammar under a loaded test run takes longer than Shiki's 500 ms,
  // and the rest of the line would stay plain
  const tokens = highlighter.codeToTokens(code, { lang, theme, tokenizeTimeLimit: 60_000 }).tokens.flat()
  return (needle) => {
    const offset = code.indexOf(needle)
    return tokens.find(token => token.offset <= offset && offset < token.offset + token.content.length)?.color
  }
}

describe("Islands syntax themes", () => {
  it("colour Kotlin as the IntelliJ Light scheme does", async () => {
    const colorOf = await colorsOf("@Service\nclass A {\n  fun get(): Int = compute(42) // why\n}", "kotlin")
    expect(colorOf("@Service")).toBe("#9E880D")
    expect(colorOf("class")).toBe("#0033B3")
    expect(colorOf("get")).toBe("#00627A")
    // a call is plain text, a declaration is not
    expect(colorOf("compute")).toBe("#080808")
    expect(colorOf("42")).toBe("#1750EB")
    expect(colorOf("// why")).toBe("#8C8C8C")
  })

  it("colour Rust and TypeScript with the dark scheme", async () => {
    const rustColor = await colorsOf("fn main() { let s = \"x\"; }", "rust", "bd-islands-dark")
    expect(rustColor("fn")).toBe("#CF8E6D")
    expect(rustColor("main")).toBe("#56A8F5")
    expect(rustColor("\"x\"")).toBe("#6AAB73")
    const typescriptColor = await colorsOf("export const pool = { size: 50 }", "typescript", "bd-islands-dark")
    expect(typescriptColor("const")).toBe("#CF8E6D")
    expect(typescriptColor("50")).toBe("#2AACB8")
  })
})
