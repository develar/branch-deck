// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitBodyBlock } from "@/lib/bindings"
import { groupCommitBodySections } from "@/lib/commit-message"

describe("groupCommitBodySections", () => {
  const list: CommitBodyBlock = { type: "list", ordered: false, start: 1, items: ["one", "two"] }
  const trailers: CommitBodyBlock = { type: "trailers", lines: ["Signed-off-by: Ann <ann@example.com>"] }

  it("turns a paragraph that is only Label: into the label of the next blocks", () => {
    expect(groupCommitBodySections([{ type: "paragraph", lines: ["What changed:"] }, list])).toEqual([{ label: "What changed", blocks: [list] }])
  })

  it("turns a short run-in Label: into a label above the rest of the paragraph", () => {
    expect(groupCommitBodySections([{ type: "paragraph", lines: ["Why: the pool ran dry."] }])).toEqual([
      { label: "Why", blocks: [{ type: "paragraph", lines: ["the pool ran dry."] }] },
    ])
  })

  it("keeps a run-in label longer than 20 characters as prose", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", lines: ["Something rather long: the text."] }
    expect(groupCommitBodySections([paragraph])).toEqual([{ label: null, blocks: [paragraph] }])
  })

  it("keeps a Label: paragraph without a following block as a paragraph", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", lines: ["Notes:"] }
    expect(groupCommitBodySections([paragraph])).toEqual([{ label: null, blocks: [paragraph] }])
  })

  it("keeps a Label: paragraph followed only by trailers as a paragraph", () => {
    const paragraph: CommitBodyBlock = { type: "paragraph", lines: ["Notes:"] }
    expect(groupCommitBodySections([paragraph, trailers])).toEqual([
      { label: null, blocks: [paragraph] },
      { label: null, blocks: [trailers] },
    ])
  })

  it("puts the trailers into the last, unlabelled section", () => {
    expect(groupCommitBodySections([{ type: "paragraph", lines: ["Why: the pool ran dry."] }, trailers])).toEqual([
      { label: "Why", blocks: [{ type: "paragraph", lines: ["the pool ran dry."] }] },
      { label: null, blocks: [trailers] },
    ])
  })

  it("keeps the lines of a run-in paragraph after its label", () => {
    expect(groupCommitBodySections([{ type: "paragraph", lines: ["Why: the pool", "ran dry."] }])).toEqual([
      { label: "Why", blocks: [{ type: "paragraph", lines: ["the pool", "ran dry."] }] },
    ])
  })

  it("adds unlabelled blocks to the current section", () => {
    const intro: CommitBodyBlock = { type: "paragraph", lines: ["Intro."] }
    const more: CommitBodyBlock = { type: "paragraph", lines: ["More detail."] }
    expect(groupCommitBodySections([intro, { type: "paragraph", lines: ["Why: the pool ran dry."] }, more, list])).toEqual([
      { label: null, blocks: [intro] },
      { label: "Why", blocks: [{ type: "paragraph", lines: ["the pool ran dry."] }, more, list] },
    ])
  })
})
