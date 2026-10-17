import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import { filterEmptyText, filterPaths, reviewFilterChips, soleOwner } from "@/lib/review-owners"
import type { BranchReviewGroup } from "@/types/sub-window"

function file(path: string, oldPath: string | null = null): CommitFile {
  return { path, oldPath, status: oldPath === null ? "modified" : "renamed", additions: 1, deletions: 0, binary: false }
}

const files = [file("plugins/air/A.kt"), file("plugins/air/B.kt", "plugins/air/Old.kt"), file("build/x.kts"), file("README.md")]
const owners = new Map([
  ["plugins/air/A.kt", "Air Plugin"],
  ["plugins/air/B.kt", "Air Plugin"],
  ["build/x.kts", "IJ Build Scripts Maintainers"],
])
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

  it("is just All when one owner has every file, unless a group asks for an approval", () => {
    const airFiles = files.slice(0, 2)
    expect(reviewFilterChips(airFiles, [], owners).map(chip => chip.label)).toEqual(["All"])
    expect(reviewFilterChips(airFiles, groups.slice(0, 1), owners).map(chip => [chip.label, chip.count])).toEqual([
      ["All", 2],
      ["Air Plugin", 2],
    ])
    // no owner of any file is one owner too
    expect(reviewFilterChips([file("README.md")], [], owners).map(chip => chip.label)).toEqual(["All"])
  })

  it("keeps the chip of the active filter when the files have none of its own, so it can be turned off", () => {
    const airFiles = files.slice(0, 2)
    expect(reviewFilterChips(airFiles, [], owners, { kind: "owner", name: "IJ Build Scripts Maintainers" }).map(chip => [chip.label, chip.count])).toEqual([
      ["All", 2],
      ["Air Plugin", 2],
      ["IJ Build Scripts Maintainers", 0],
    ])
    expect(reviewFilterChips(airFiles, [], owners, { kind: "unowned" }).map(chip => chip.label)).toEqual(["All", "Air Plugin", "No Owner"])
    // an active filter that keeps every file, or a part without files, still shows its chip
    expect(reviewFilterChips(airFiles, [], owners, { kind: "owner", name: "Air Plugin" }).map(chip => chip.label)).toEqual(["All", "Air Plugin"])
    expect(reviewFilterChips([], [], owners, { kind: "unowned" }).map(chip => [chip.label, chip.count])).toEqual([
      ["All", 0],
      ["No Owner", 0],
    ])
  })
})

describe("soleOwner", () => {
  it("is the owner of every file, else null", () => {
    expect(soleOwner(files.slice(0, 2), owners)).toBe("Air Plugin")
    expect(soleOwner(files, owners)).toBeNull()
    expect(soleOwner([file("README.md")], owners)).toBeNull()
    expect(soleOwner([], owners)).toBeNull()
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
