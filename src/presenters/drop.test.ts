import { describe, expect, it } from "vite-plus/test"
import { describeDropped } from "@/presenters/drop"

describe("describeDropped", () => {
  it("says that the files stayed and names the branches left without a commit", () => {
    expect(describeDropped({ filesChanged: true, emptiedBranches: [] })).toBeNull()
    expect(describeDropped({ filesChanged: false, emptiedBranches: [] })).toBe("Your files did not change: the commits cancel each other")
    expect(describeDropped({ filesChanged: true, emptiedBranches: ["auth"] })).toBe("auth has no commits left")
    expect(describeDropped({ filesChanged: false, emptiedBranches: ["auth", "ui", "docs"] })).toBe(
      "Your files did not change: the commits cancel each other. auth, ui and docs have no commits left",
    )
  })
})
