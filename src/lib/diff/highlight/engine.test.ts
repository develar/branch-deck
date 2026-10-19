// @spec spec/commit/commit-files.spec.md
import { beforeAll, describe, expect, it } from "vite-plus/test"
import { HighlightEngine } from "@/lib/diff/highlight/engine"
import type { GrammarSource } from "@/lib/diff/highlight/engine"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"
import { FONT_STYLE_SHIFT, PAGE_LINES, ROLE_MASK } from "@/lib/diff/highlight/protocol"
import type { HighlightAnswer } from "@/lib/diff/highlight/protocol"
import { SYNTAX_ROLES } from "@/lib/diff/islands-themes"

type PageAnswer = Extract<HighlightAnswer, { type: "page" }>

let source: GrammarSource

beforeAll(async () => {
  source = await createGrammarSource()
  // the first compile of a grammar under a loaded test run is slow: warm it outside the tests
  await Promise.all(["kotlin", "rust", "typescript"].map(language => source.grammar(language)))
})

function engineOf(): { engine: HighlightEngine; answers: HighlightAnswer[] } {
  const answers: HighlightAnswer[] = []
  const engine = new HighlightEngine(source, answer => answers.push(answer))
  return { engine, answers }
}

/** The role name and the font style bits at `offset` of line `line` of a page */
function styleAt(page: PageAnswer, line: number, offset: number): string {
  let style = 0
  for (let pair = page.lineStarts[line]!; pair < page.lineStarts[line + 1]!; pair++) {
    if (page.runs[pair * 2]! <= offset) {
      style = page.runs[pair * 2 + 1]!
    }
  }
  const role = style & ROLE_MASK
  const fontStyle = style >> FONT_STYLE_SHIFT
  return `${role === 0 ? "foreground" : SYNTAX_ROLES[role - 1]}${fontStyle & 1 ? " italic" : ""}`
}

function pages(answers: readonly HighlightAnswer[]): PageAnswer[] {
  return answers.filter((answer): answer is PageAnswer => answer.type === "page")
}

describe("HighlightEngine", () => {
  it("gives each token the role of the Islands themes, in runs that change only with the style", async () => {
    const { engine, answers } = engineOf()
    const code = "@Service\nclass A {\n  fun get(): Int = compute(42) // why\n}"
    engine.handle({ type: "text", blobId: "a", language: "kotlin", text: code })
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "a", pages: [0], priority: 0 }] })
    await engine.idle()
    const [page] = pages(answers)
    expect(page).toMatchObject({ exact: true })
    const line = (text: string, needle: string) => styleAt(page!, code.split("\n").indexOf(text), text.indexOf(needle))
    expect(line("@Service", "@")).toBe("annotation")
    expect(line("class A {", "class")).toBe("keyword")
    const third = "  fun get(): Int = compute(42) // why"
    expect([line(third, "get"), line(third, "compute"), line(third, "42"), line(third, "//")]).toEqual(["functionDeclaration", "foreground", "number", "comment italic"])
    // the brace of the last line is the default foreground: no run
    expect(page!.lineStarts[4]! - page!.lineStarts[3]!).toBe(0)
  })

  it("gives Rust and TypeScript tokens the roles of the Islands themes too", async () => {
    const { engine, answers } = engineOf()
    const rust = 'fn main() { let s = "x"; }'
    const typescript = "export const pool = { size: 50 }"
    engine.handle({ type: "text", blobId: "rust", language: "rust", text: rust })
    engine.handle({ type: "text", blobId: "ts", language: "typescript", text: typescript })
    engine.handle({
      type: "tokens",
      view: "v",
      wants: [
        { blobId: "rust", pages: [0], priority: 0 },
        { blobId: "ts", pages: [0], priority: 0 },
      ],
    })
    await engine.idle()
    const pageOf = (blobId: string) => pages(answers).find(page => page.blobId === blobId)!
    expect(["fn", "main", '"x"'].map(needle => styleAt(pageOf("rust"), 0, rust.indexOf(needle)))).toEqual(["keyword", "functionDeclaration", "string"])
    expect(["const", "50"].map(needle => styleAt(pageOf("ts"), 0, typescript.indexOf(needle)))).toEqual(["keyword", "number"])
  })

  it("answers a page far into a file first as provisional, then exact, and confirms equal runs without arrays", async () => {
    const { engine, answers } = engineOf()
    const text = Array.from({ length: PAGE_LINES * 3 }, (_, index) => `val v${index} = ${index}`).join("\n")
    engine.handle({ type: "text", blobId: "b", language: "kotlin", text })
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "b", pages: [2], priority: 0 }] })
    await engine.idle()
    expect(answers.map(answer => `${answer.type}${answer.type === "page" ? ` ${answer.page} ${answer.exact}` : ""}`)).toEqual(["page 2 false", "exact"])
  })

  it("sends the exact runs of a provisional page that started in the wrong state", async () => {
    const { engine, answers } = engineOf()
    // a block comment from line 10, its lines without indent: the guess starts inside it from the initial state
    const lines = Array.from({ length: PAGE_LINES * 3 }, (_, index) => (index === 10 ? "/*" : `text ${index}`))
    lines.push("*/")
    engine.handle({ type: "text", blobId: "c", language: "kotlin", text: lines.join("\n") })
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "c", pages: [2], priority: 0 }] })
    await engine.idle()
    const [provisional, exact] = pages(answers)
    expect([provisional?.exact, exact?.exact]).toEqual([false, true])
    expect(styleAt(provisional!, 0, 0)).toBe("foreground")
    expect(styleAt(exact!, 0, 0)).toBe("comment italic")
  })

  it("asks for a text it does not have, and says when a language has no grammar", async () => {
    const { engine, answers } = engineOf()
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "none", pages: [0], priority: 0 }] })
    engine.handle({ type: "text", blobId: "e", language: "no-such-language", text: "x" })
    engine.handle({ type: "tokens", view: "w", wants: [{ blobId: "e", pages: [0], priority: 0 }] })
    await engine.idle()
    expect(answers.map(answer => answer.type)).toEqual(["missing", "failed"])
  })

  it("drops the work a view asked for before its new request", async () => {
    const { engine, answers } = engineOf()
    engine.handle({ type: "text", blobId: "d", language: "kotlin", text: "val a = 1\n".repeat(PAGE_LINES * 6) })
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "d", pages: [4, 5], priority: 0 }] })
    engine.handle({ type: "tokens", view: "v", wants: [{ blobId: "d", pages: [0], priority: 0 }] })
    await engine.idle()
    expect(pages(answers).map(page => page.page)).toEqual([0])
  })
})
