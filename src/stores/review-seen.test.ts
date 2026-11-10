// @spec openspec/specs/branch/review-tour/spec.md
import { describe, expect, it } from "vite-plus/test"
import { markCardsSeen } from "@/stores/review-seen"

describe("the files the reader saw", () => {
  it("sees a file of one card at its card, a file the story shows in parts once each of its parts was seen", () => {
    const parts = new Map([["src/Loader.kt", ["file:src/Loader.kt", "file:src/Loader.kt#1", "group:imports:src/Loader.kt"]]])
    expect(markCardsSeen("head-1", [{ path: "src/Config.kt", key: "file:src/Config.kt" }], parts)).toEqual(["src/Config.kt"])
    expect(markCardsSeen("head-1", [{ path: "src/Loader.kt", key: "file:src/Loader.kt" }], parts)).toEqual([])
    const rest = [
      { path: "src/Loader.kt", key: "file:src/Loader.kt#1" },
      { path: "src/Loader.kt", key: "group:imports:src/Loader.kt" },
    ]
    expect(markCardsSeen("head-1", rest, parts)).toEqual(["src/Loader.kt"])
    // a new head counts from zero
    expect(markCardsSeen("head-2", [{ path: "src/Loader.kt", key: "file:src/Loader.kt#1" }], parts)).toEqual([])
    expect(markCardsSeen(null, [{ path: "src/Config.kt", key: "file:src/Config.kt" }], parts)).toEqual([])
  })
})
