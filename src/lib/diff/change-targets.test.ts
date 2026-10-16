// @spec spec/commit/commit-review.spec.md
import { processFile } from "@pierre/diffs"
import { describe, expect, it } from "vite-plus/test"
import { getChangeTargets } from "@/lib/diff/change-targets"

function parse(patch: string) {
  const fileDiff = processFile(patch, { isGitDiff: false, throwOnError: true })
  if (fileDiff === undefined) {
    throw new Error("no diff")
  }
  return fileDiff
}

describe("getChangeTargets", () => {
  it("finds each block of changed lines on the new side, past the context and the other blocks", () => {
    const patch = ["--- a/a.kt", "+++ b/a.kt", "@@ -10,6 +10,8 @@", " a", "-b", "+B", "+B2", " c", " d", "+e", " f", " g", ""].join("\n")
    expect(getChangeTargets(parse(patch))).toEqual([
      { side: "additions", lineNumber: 11 },
      { side: "additions", lineNumber: 15 },
    ])
  })

  it("takes the old side of a block that only deletes, and counts every hunk from its own start", () => {
    const patch = "--- a/a.kt\n+++ b/a.kt\n@@ -1,3 +1,2 @@\n a\n-b\n c\n@@ -40,2 +39,3 @@\n x\n+y\n z\n"
    expect(getChangeTargets(parse(patch))).toEqual([
      { side: "deletions", lineNumber: 2 },
      { side: "additions", lineNumber: 40 },
    ])
  })
})
