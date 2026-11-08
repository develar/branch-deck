import { describe, expect, it } from "vite-plus/test"
import type { CommitBody, CommitBodyBlock } from "@/lib/bindings"
import { presentSummary } from "@/presenters/commit-body"

function body(blocks: CommitBodyBlock[], summary: number | null, trailers: string[] = []): CommitBody {
  return { blocks, trailers, lead: summary === 0, summary, parts: [], code: [{ block: 1, item: 0, spans: [{ start: 0, end: 4, explicit: false, joined: false }] }] }
}

const GOAL: CommitBodyBlock = { type: "heading", level: 2, text: "Goal" }
const SUMMARY: CommitBodyBlock = { type: "paragraph", lines: ["Play video in the chat."] }
const SIZES: CommitBodyBlock = { type: "table", header: ["Module", "Size"], align: ["none", "right"], rows: [["media", "6.4 MB"]] }

describe("presentSummary", () => {
  it("leads with the block Rust names the summary, its code kept, and says more of a description beside it", () => {
    const view = presentSummary(body([GOAL, SUMMARY, { type: "heading", level: 2, text: "Sizes" }, SIZES], 1))
    expect(view.parts).toEqual({
      lead: { index: 1, block: SUMMARY },
      shown: [],
      folded: [],
      code: [{ block: 1, item: 0, spans: [{ start: 0, end: 4, explicit: false, joined: false }] }],
    })
    expect(view.more).toBe(true)
  })

  it("says no more of a summary under its heading alone, more of one with trailers", () => {
    expect(presentSummary(body([GOAL, SUMMARY], 1)).more).toBe(false)
    expect(presentSummary(body([SUMMARY], 0)).more).toBe(false)
    expect(presentSummary(body([SUMMARY], 0, ["Co-Authored-By: dave"])).more).toBe(true)
  })

  it("leads with nothing of a body without a summary, which the overview shows whole", () => {
    const view = presentSummary(body([GOAL, SIZES], null))
    expect(view.parts).toBeNull()
    expect(view.more).toBe(true)
  })
})
