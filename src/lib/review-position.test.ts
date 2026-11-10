import { describe, expect, it } from "vite-plus/test"
import { formatReviewPosition, shownPath } from "@/lib/review-position"

describe("formatReviewPosition", () => {
  const file = { path: "src/a.ts", index: 2, count: 12 }

  it("names the file, after the commit or the scope", () => {
    expect(formatReviewPosition({ file })).toBe("File 3 / 12")
    expect(formatReviewPosition({ file, commit: { index: 1, count: 5 } })).toBe("Commit 2 / 5 · File 3 / 12")
    expect(formatReviewPosition({ file, scope: "All Changes" })).toBe("All Changes · File 3 / 12")
  })

  it("names the chosen owner chip without duplicating the file count", () => {
    const own = { ...file, owner: "Design" }
    expect(formatReviewPosition({ file: own, scope: "All Changes" })).toBe("All Changes · File 3 / 12 · Design")
    expect(formatReviewPosition({ file: { ...file, owner: "Design" } })).toBe("File 3 / 12 · Design")
  })

  it("leaves the file out until the review reports one", () => {
    expect(formatReviewPosition({ file: null, commit: { index: 0, count: 2 } })).toBe("Commit 1 / 2")
    expect(formatReviewPosition({ file: { path: null, index: -1, count: 0 } })).toBe("")
  })
})

describe("the position of the document of a review", () => {
  it("says Overview while the overview is at the top, and the step of the story before the file", () => {
    expect(formatReviewPosition({ file: { path: "a.kt", index: 0, count: 9, overview: true }, scope: "All Changes" })).toBe("All Changes · Overview")
    expect(formatReviewPosition({ file: { path: "a.kt", index: 2, count: 9, step: 2 }, scope: "All Changes" })).toBe("All Changes · Step 2 · File 3 / 9")
  })

  it("names the part at the top of a file the story shows in several sections after the file", () => {
    expect(formatReviewPosition({ file: { path: "a.kt", index: 0, count: 7, step: 1, part: { index: 1, count: 2 } } })).toBe("Step 1 · File 1 / 7 · part 2 of 2")
  })
})

describe("shownPath", () => {
  it("shows the file of the position, none while the overview is at the top", () => {
    expect(shownPath({ path: "src/a.ts", index: 0, count: 9 })).toBe("src/a.ts")
    expect(shownPath({ path: "src/a.ts", index: 0, count: 9, overview: true })).toBeNull()
    expect(shownPath({ path: null, index: -1, count: 0 })).toBeNull()
  })

  it("shows no file while one stands in for the file on screen the chip lacks", () => {
    expect(shownPath({ path: "ui/login.css", index: 0, count: 1, owner: "Design", standIn: true })).toBeNull()
  })
})
