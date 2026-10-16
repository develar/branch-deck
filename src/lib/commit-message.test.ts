// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { groupCommitBodySections, parseCommitBody } from "@/lib/commit-message"
import type { CommitBodyBlock } from "@/lib/commit-message"

describe("parseCommitBody", () => {
  it("joins hard-wrapped lines into one paragraph", () => {
    expect(parseCommitBody("The pool size was too small\nfor the nightly import.")).toEqual([
      { type: "paragraph", text: "The pool size was too small for the nightly import." },
    ])
  })

  it("starts a new block after a blank line", () => {
    expect(parseCommitBody("First paragraph.\n\nSecond paragraph.")).toEqual([
      { type: "paragraph", text: "First paragraph." },
      { type: "paragraph", text: "Second paragraph." },
    ])
  })

  it("turns lines with -, * and • markers into one bullet list without the markers", () => {
    expect(parseCommitBody("- dash item\n* star item\n• bullet item")).toEqual([
      { type: "list", ordered: false, start: 1, items: ["dash item", "star item", "bullet item"] },
    ])
  })

  it("turns lines with 1. markers into an ordered list", () => {
    expect(parseCommitBody("1. first\n2. second")).toEqual([
      { type: "list", ordered: true, start: 1, items: ["first", "second"] },
    ])
  })

  it("keeps the number of the first item of an ordered list with 1) markers", () => {
    expect(parseCommitBody("3) third\n4) fourth")).toEqual([
      { type: "list", ordered: true, start: 3, items: ["third", "fourth"] },
    ])
  })

  it("starts a new list when bullets change to numbers", () => {
    expect(parseCommitBody("- bullet\n1. number")).toEqual([
      { type: "list", ordered: false, start: 1, items: ["bullet"] },
      { type: "list", ordered: true, start: 1, items: ["number"] },
    ])
  })

  it("keeps a continuation line in its list item", () => {
    expect(parseCommitBody("- a long item that git\n  wrapped here\n- next item\nunindented continuation")).toEqual([
      { type: "list", ordered: false, start: 1, items: ["a long item that git wrapped here", "next item unindented continuation"] },
    ])
  })

  it("keeps lines indented by four spaces verbatim, without the common indent", () => {
    expect(parseCommitBody("Run it with:\n    pnpm test:unit\n      --watch")).toEqual([
      { type: "paragraph", text: "Run it with:" },
      { type: "preformatted", text: "pnpm test:unit\n  --watch" },
    ])
  })

  it("keeps lines indented by a tab verbatim, without the tab", () => {
    expect(parseCommitBody("\tfirst()\n\tsecond()")).toEqual([
      { type: "preformatted", text: "first()\nsecond()" },
    ])
  })

  it("keeps a fenced block verbatim with its blank lines and without the fences", () => {
    expect(parseCommitBody("Before.\n```ts\nconst a = 1\n\nconst b = 2\n```\nAfter.")).toEqual([
      { type: "paragraph", text: "Before." },
      { type: "preformatted", text: "const a = 1\n\nconst b = 2" },
      { type: "paragraph", text: "After." },
    ])
  })

  it("runs an unclosed fence to the end of the body", () => {
    expect(parseCommitBody("```\nfirst\n\nsecond\n")).toEqual([
      { type: "preformatted", text: "first\n\nsecond" },
    ])
  })

  it("turns a last block of git trailers into a trailers block", () => {
    expect(parseCommitBody("Body.\n\nSigned-off-by: Ann <ann@example.com>\nCo-Authored-By: Bob <bob@example.com>")).toEqual([
      { type: "paragraph", text: "Body." },
      { type: "trailers", lines: ["Signed-off-by: Ann <ann@example.com>", "Co-Authored-By: Bob <bob@example.com>"] },
    ])
  })

  it("joins an indented trailer continuation line to its trailer", () => {
    expect(parseCommitBody("Body.\n\nFixes: a long trailer value\n  that continues")).toEqual([
      { type: "paragraph", text: "Body." },
      { type: "trailers", lines: ["Fixes: a long trailer value that continues"] },
    ])
  })

  it("accepts a cherry-pick note as a trailer", () => {
    expect(parseCommitBody("Body.\n\n(cherry picked from commit 0123abcd)\nSigned-off-by: Ann <ann@example.com>")).toEqual([
      { type: "paragraph", text: "Body." },
      { type: "trailers", lines: ["(cherry picked from commit 0123abcd)", "Signed-off-by: Ann <ann@example.com>"] },
    ])
  })

  it("keeps a trailer-shaped block that is not the last block as a paragraph", () => {
    expect(parseCommitBody("Note: read this first\n\nThe body.")).toEqual([
      { type: "paragraph", text: "Note: read this first" },
      { type: "paragraph", text: "The body." },
    ])
  })

  it("keeps a last block as a paragraph when one of its lines is not a trailer", () => {
    expect(parseCommitBody("Fixes: JIRA-1\nand some prose")).toEqual([
      { type: "paragraph", text: "Fixes: JIRA-1 and some prose" },
    ])
  })

  it("treats CRLF line endings like LF", () => {
    expect(parseCommitBody("First line\r\nsecond line\r\n\r\n- item")).toEqual([
      { type: "paragraph", text: "First line second line" },
      { type: "list", ordered: false, start: 1, items: ["item"] },
    ])
  })
})

describe("groupCommitBodySections", () => {
  const list: CommitBodyBlock = { type: "list", ordered: false, start: 1, items: ["one", "two"] }
  const trailers: CommitBodyBlock = { type: "trailers", lines: ["Signed-off-by: Ann <ann@example.com>"] }

  it("turns a paragraph that is only Label: into the label of the next blocks", () => {
    expect(groupCommitBodySections([{ type: "paragraph", text: "What changed:" }, list])).toEqual([
      { label: "What changed", blocks: [list] },
    ])
  })

  it("turns a short run-in Label: into a label above the rest of the paragraph", () => {
    expect(groupCommitBodySections([{ type: "paragraph", text: "Why: the pool ran dry." }])).toEqual([
      { label: "Why", blocks: [{ type: "paragraph", text: "the pool ran dry." }] },
    ])
  })

  it("keeps a run-in label longer than 20 characters as prose", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", text: "Something rather long: the text." }
    expect(groupCommitBodySections([paragraph])).toEqual([
      { label: null, blocks: [paragraph] },
    ])
  })

  it("keeps a Label: paragraph without a following block as a paragraph", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", text: "Notes:" }
    expect(groupCommitBodySections([paragraph])).toEqual([
      { label: null, blocks: [paragraph] },
    ])
  })

  it("keeps a Label: paragraph followed only by trailers as a paragraph", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", text: "Notes:" }
    expect(groupCommitBodySections([paragraph, trailers])).toEqual([
      { label: null, blocks: [paragraph] },
      { label: null, blocks: [trailers] },
    ])
  })

  it("puts the trailers into the last, unlabelled section", () => {
    expect(groupCommitBodySections([{ type: "paragraph", text: "Why: the pool ran dry." }, trailers])).toEqual([
      { label: "Why", blocks: [{ type: "paragraph", text: "the pool ran dry." }] },
      { label: null, blocks: [trailers] },
    ])
  })

  it("adds unlabelled blocks to the current section", () => {
    const intro: CommitBodyBlock = { type: "paragraph", text: "Intro." }
    const more: CommitBodyBlock = { type: "paragraph", text: "More detail." }
    expect(groupCommitBodySections([intro, { type: "paragraph", text: "Why: the pool ran dry." }, more, list])).toEqual([
      { label: null, blocks: [intro] },
      { label: "Why", blocks: [{ type: "paragraph", text: "the pool ran dry." }, more, list] },
    ])
  })
})
