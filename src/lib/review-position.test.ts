import { describe, expect, it } from "vite-plus/test"
import { formatReviewPosition } from "@/lib/review-position"

describe("formatReviewPosition", () => {
  const file = { path: "src/a.ts", index: 2, count: 12 }

  it("names the file, after the commit or the scope", () => {
    expect(formatReviewPosition({ file })).toBe("File 3 / 12")
    expect(formatReviewPosition({ file, commit: { index: 1, count: 5 } })).toBe("Commit 2 / 5 · File 3 / 12")
    expect(formatReviewPosition({ file, scope: "All Changes" })).toBe("All Changes · File 3 / 12")
  })

  it("counts the part of the chosen owner chip after the file: its place on a file of its own, its files on another", () => {
    const own = { ...file, owner: { label: "Design", index: 1, count: 5 } }
    expect(formatReviewPosition({ file: own, scope: "All Changes" })).toBe("All Changes · File 3 / 12 · Design 2 / 5")
    const other = { ...file, owner: { label: "Design", index: -1, count: 5 } }
    expect(formatReviewPosition({ file: other, scope: "All Changes" })).toBe("All Changes · File 3 / 12 · Design 5")
    expect(formatReviewPosition({ file: { ...file, owner: { label: "Design", index: -1, count: 0 } } })).toBe("File 3 / 12")
  })

  it("leaves the file out until the review reports one", () => {
    expect(formatReviewPosition({ file: null, commit: { index: 0, count: 2 } })).toBe("Commit 1 / 2")
    expect(formatReviewPosition({ file: { path: null, index: -1, count: 0 } })).toBe("")
  })
})
