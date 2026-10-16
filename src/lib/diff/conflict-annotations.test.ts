// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { getConflictAnnotations } from "@/lib/diff/conflict-annotations"

const PATCH = [
  "--- a/src/a.kt",
  "+++ b/src/a.kt",
  "@@ -1,2 +1,7 @@",
  " fun a() {",
  "-  old()",
  "+<<<<<<< HEAD",
  "+  ours()",
  "+||||||| 1234567",
  "+=======",
  "+  theirs()",
  "+>>>>>>> 89abcdef (Add theirs)",
  "\\ No newline at end of file",
  "",
].join("\n")

describe("getConflictAnnotations", () => {
  it("annotates each marker kind at its new-file line, past context and deleted lines", () => {
    const annotations = getConflictAnnotations(PATCH, undefined)
    expect(annotations.map(it => [it.side, it.lineNumber, it.metadata.conflictInfo?.type])).toEqual([
      ["additions", 2, "conflict-start"],
      ["additions", 4, "conflict-base"],
      ["additions", 5, "conflict-separator"],
      ["additions", 7, "conflict-end"],
    ])
  })

  it("names the branch, the base and the commit of the markers", () => {
    const [start, base, , end] = getConflictAnnotations(PATCH, undefined).map(it => it.metadata.conflictInfo)
    expect(start?.branch).toBe("HEAD")
    expect(base?.base).toBe("1234567")
    expect(end?.commit).toBe("89abcdef (Add theirs)")
  })

  it("counts lines from the start of each hunk", () => {
    const patch = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n@@ -40,2 +40,2 @@\n x\n+=======\n"
    expect(getConflictAnnotations(patch, undefined).map(it => it.lineNumber)).toEqual([41])
  })
})
