import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import { filterEmptyText, filterPaths, reviewFilterChips } from "@/lib/review-owners"
import type { BranchReviewGroup } from "@/types/sub-window"

function file(path: string, oldPath: string | null = null): CommitFile {
  return { path, oldPath, status: oldPath === null ? "modified" : "renamed", additions: 1, deletions: 0, binary: false }
}

const files = [file("plugins/air/A.kt"), file("plugins/air/B.kt", "plugins/air/Old.kt"), file("build/x.kts"), file("README.md")]
const owners = new Map([["plugins/air/A.kt", "Air Plugin"], ["plugins/air/B.kt", "Air Plugin"], ["build/x.kts", "IJ Build Scripts Maintainers"]])
const groups: BranchReviewGroup[] = [
  // Patronus names the old path of the rename (it reads the commits without rename detection)
  { name: "Air Plugin", paths: ["plugins/air/A.kt", "plugins/air/Old.kt"], branchWide: false },
  { name: "Bazel QA", paths: [], branchWide: true },
  { name: "Delegated", paths: [], branchWide: false },
]

describe("reviewFilterChips", () => {
  it("lists all files, the groups the check asks for, then the other owners and the files without one", () => {
    expect(reviewFilterChips(files, groups, owners).map(chip => [chip.label, chip.count, chip.needsApproval])).toEqual([
      ["All", 4, false],
      ["Air Plugin", 2, true],
      ["Bazel QA", 4, true],
      ["Delegated", null, true],
      ["IJ Build Scripts Maintainers", 1, false],
      ["No Owner", 1, false],
    ])
  })

  it("has no owner chips without OWNERSHIP files", () => {
    expect(reviewFilterChips(files, [], new Map()).map(chip => chip.label)).toEqual(["All"])
  })
})

describe("filterPaths", () => {
  it("keeps the files of a group (a rename by its old path), of an owner, or without an owner", () => {
    expect(filterPaths({ kind: "all" }, files, groups, owners)).toBeNull()
    expect([...filterPaths({ kind: "group", name: "Air Plugin" }, files, groups, owners)!]).toEqual(["plugins/air/A.kt", "plugins/air/B.kt"])
    expect(filterPaths({ kind: "group", name: "Bazel QA" }, files, groups, owners)).toBeNull()
    expect([...filterPaths({ kind: "owner", name: "IJ Build Scripts Maintainers" }, files, groups, owners)!]).toEqual(["build/x.kts"])
    expect([...filterPaths({ kind: "unowned" }, files, groups, owners)!]).toEqual(["README.md"])
  })

  it("says why a filter shows nothing", () => {
    expect(filterEmptyText({ kind: "group", name: "Air Plugin" })).toBe("No files of Air Plugin in this range")
  })
})
