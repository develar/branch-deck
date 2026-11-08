import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import {
  awaitsRememberedChip,
  canonicalOwnerFilter,
  initialOwnerFilter,
  ownerChipKey,
  ownerEmptyText,
  ownerLabel,
  ownerPaths,
  reviewFilterChips,
  reviewOwnerMarks,
  soleOwner,
} from "@/lib/review-owners"
import type { BranchReviewGroup } from "@/types/sub-window"

function file(path: string, oldPath: string | null = null): CommitFile {
  return { path, oldPath, status: oldPath === null ? "modified" : "renamed", additions: 1, deletions: 0, binary: false, oldId: null, newId: null, submodule: false }
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

  it("keeps the chosen chip when the files have none of its own, so it can be turned off", () => {
    const airFiles = files.slice(0, 2)
    expect(reviewFilterChips(airFiles, [], owners, { kind: "owner", name: "IJ Build Scripts Maintainers" }).map(chip => [chip.label, chip.count])).toEqual([
      ["All", 2],
      ["Air Plugin", 2],
      ["IJ Build Scripts Maintainers", 0],
    ])
    expect(reviewFilterChips(airFiles, [], owners, { kind: "unowned" }).map(chip => chip.label)).toEqual(["All", "Air Plugin", "No Owner"])
    // a chosen chip that marks every file, or a part without files, still shows
    expect(reviewFilterChips(airFiles, [], owners, { kind: "owner", name: "Air Plugin" }).map(chip => chip.label)).toEqual(["All", "Air Plugin"])
    expect(reviewFilterChips([], [], owners, { kind: "unowned" }).map(chip => [chip.label, chip.count])).toEqual([
      ["All", 0],
      ["No Owner", 0],
    ])
  })
})

describe("the groups of a pull request", () => {
  // Design needs the user's approval, Air Plugin has its approvals, Build is the user's and approved
  const requestGroups: BranchReviewGroup[] = [
    { name: "Air Plugin", paths: ["plugins/air/A.kt"], branchWide: false, approved: true, yours: false },
    { name: "Build", paths: ["build/x.kts"], branchWide: false, approved: true, yours: true },
    { name: "Design", paths: ["README.md"], branchWide: false, approved: false, yours: true },
  ]

  it("lists the user's groups first, each with whether it has its approvals", () => {
    expect(reviewFilterChips(files, requestGroups, owners).map(chip => [chip.label, chip.yours, chip.approved])).toEqual([
      ["All", false, false],
      ["Build", true, true],
      ["Design", true, false],
      ["Air Plugin", false, true],
      ["IJ Build Scripts Maintainers", false, false],
      ["No Owner", false, false],
    ])
  })

  it("opens on the remembered chip where it marks files, else the user's group that still needs an approval, else All", () => {
    const chips = reviewFilterChips(files, requestGroups, owners)
    expect(initialOwnerFilter(chips, undefined, true)).toEqual({ kind: "group", name: "Design" })
    // the branch window doesn't prefer the user's group
    expect(initialOwnerFilter(chips, undefined, false)).toEqual({ kind: "all" })
    expect(initialOwnerFilter(chips, { kind: "group", name: "Air Plugin" }, true)).toEqual({ kind: "group", name: "Air Plugin" })
    // remembered as an owner of the OWNERSHIP files, it is the group of the same name here, and the other way round
    expect(initialOwnerFilter(chips, { kind: "owner", name: "air plugin" }, true)).toEqual({ kind: "group", name: "Air Plugin" })
    expect(initialOwnerFilter(reviewFilterChips(files, [], owners), { kind: "group", name: "Air Plugin" }, true)).toEqual({ kind: "owner", name: "Air Plugin" })
    // a remembered chip without files here, or missing, is ignored
    expect(initialOwnerFilter(chips, { kind: "owner", name: "Kotlin" }, true)).toEqual({ kind: "group", name: "Design" })
    expect(initialOwnerFilter(reviewFilterChips(files.slice(0, 2), requestGroups, owners), { kind: "owner", name: "Build" }, true)).toEqual({ kind: "all" })
    expect(initialOwnerFilter(chips, { kind: "unowned" }, true)).toEqual({ kind: "unowned" })
  })

  it("waits for a remembered chip the chips don't have yet: the owners come after the groups", () => {
    const chips = reviewFilterChips(files, requestGroups, owners)
    // the groups alone: the owner "Air Plugin" may still come, named by its group here
    expect(awaitsRememberedChip(reviewFilterChips(files, requestGroups, new Map()), { kind: "owner", name: "Kotlin" })).toBe(true)
    expect(awaitsRememberedChip(chips, { kind: "owner", name: "air plugin" })).toBe(false)
    expect(awaitsRememberedChip(chips, undefined)).toBe(false)
    expect(awaitsRememberedChip(chips, { kind: "all" })).toBe(false)
  })

  it("never opens on a group whose files Patronus doesn't tell, and opens on a branch-wide one", () => {
    const unknown: BranchReviewGroup[] = [{ name: "Delegated", paths: [], branchWide: false, approved: false, yours: true }]
    expect(initialOwnerFilter(reviewFilterChips(files, unknown, owners), { kind: "group", name: "Delegated" }, true)).toEqual({ kind: "all" })
    const wide: BranchReviewGroup[] = [{ name: "Bazel QA", paths: [], branchWide: true, approved: false, yours: true }]
    const chips = reviewFilterChips(files, wide, owners)
    expect(initialOwnerFilter(chips, undefined, true)).toEqual({ kind: "group", name: "Bazel QA" })
    // it marks every file: nothing is muted, and the footer counts its part as all of them
    const marks = reviewOwnerMarks({ kind: "group", name: "Bazel QA" }, { files, groups: wide, owners })
    expect([marks?.label, marks?.paths.size]).toEqual(["Bazel QA", files.length])
    expect(files.every(file => marks?.paths.has(file.path))).toBe(true)
  })

  it("makes an owner chip held before the groups came the group of its name: one chip, pressed, with the group's files", () => {
    // Design owns README.md and B.kt by the OWNERSHIP files; its group owes README.md alone
    const designOwners = new Map([...owners, ["README.md", "Design"], ["plugins/air/B.kt", "Design"]])
    const design = requestGroups.filter(group => group.name === "Design")
    const before = reviewFilterChips(files, [], designOwners)
    // remembered as the group, or chosen as the owner, before Patronus answers: the owner chip
    const held = initialOwnerFilter(before, { kind: "group", name: "Design" }, true)
    expect(held).toEqual({ kind: "owner", name: "Design" })
    expect(canonicalOwnerFilter(before, held)).toEqual(held)

    const after = reviewFilterChips(files, design, designOwners)
    const filter = canonicalOwnerFilter(after, held)
    expect(filter).toEqual({ kind: "group", name: "Design" })
    // the held owner adds no chip of its own beside the group's, and no two chips share a value
    for (const active of [held, filter]) {
      const chips = reviewFilterChips(files, design, designOwners, active)
      expect(chips.map(chip => [chip.label, chip.count, chip.yours])).toEqual([
        ["All", 4, false],
        ["Design", 1, true],
        ["Air Plugin", 1, false],
        ["IJ Build Scripts Maintainers", 1, false],
      ])
      expect(new Set(chips.map(chip => ownerChipKey(chip.filter))).size).toBe(chips.length)
    }
    expect([...reviewOwnerMarks(filter, { files, groups: design, owners: designOwners })!.paths]).toEqual(["README.md"])
    // the groups go again: the owner chip of the name
    expect(canonicalOwnerFilter(before, filter)).toEqual({ kind: "owner", name: "Design" })
    // a chip of another name, All and No Owner stay as they are
    expect(canonicalOwnerFilter(after, { kind: "owner", name: "Kotlin" })).toEqual({ kind: "owner", name: "Kotlin" })
    expect(canonicalOwnerFilter(after, { kind: "all" })).toEqual({ kind: "all" })
    expect(canonicalOwnerFilter(after, { kind: "unowned" })).toEqual({ kind: "unowned" })
  })

  it("marks the files of the chosen chip by its name", () => {
    const marks = reviewOwnerMarks({ kind: "group", name: "Design" }, { files, groups: requestGroups, owners })
    expect(marks?.label).toBe("Design")
    expect([...marks!.paths]).toEqual(["README.md"])
    expect(reviewOwnerMarks({ kind: "all" }, { files, groups: requestGroups, owners })).toBeNull()
    expect(reviewOwnerMarks({ kind: "group", name: "Design" }, { files: undefined, groups: requestGroups, owners })).toBeNull()
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

describe("ownerPaths", () => {
  it("marks the files of a group (a rename by its old path), of an owner, or without an owner", () => {
    expect(ownerPaths({ kind: "all" }, files, groups, owners)).toBeNull()
    expect([...ownerPaths({ kind: "group", name: "Air Plugin" }, files, groups, owners)!]).toEqual(["plugins/air/A.kt", "plugins/air/B.kt"])
    // a branch-wide group marks every file (nothing is muted), one Patronus doesn't tell none
    expect([...ownerPaths({ kind: "group", name: "Bazel QA" }, files, groups, owners)!]).toEqual(files.map(file => file.path))
    expect(ownerPaths({ kind: "group", name: "Delegated" }, files, groups, owners)).toBeNull()
    expect([...ownerPaths({ kind: "owner", name: "IJ Build Scripts Maintainers" }, files, groups, owners)!]).toEqual(["build/x.kts"])
    const renamedOwner = new Map(owners)
    renamedOwner.delete("plugins/air/B.kt")
    renamedOwner.set("plugins/air/Old.kt", "Air Plugin")
    expect([...ownerPaths({ kind: "owner", name: "Air Plugin" }, files, groups, renamedOwner)!]).toEqual(["plugins/air/A.kt", "plugins/air/B.kt"])
    expect(reviewFilterChips(files, [], renamedOwner).map(chip => [chip.label, chip.count])).toContainEqual(["Air Plugin", 2])
    expect([...ownerPaths({ kind: "unowned" }, files, groups, renamedOwner)!]).toEqual(["README.md"])
    expect([...ownerPaths({ kind: "unowned" }, files, groups, owners)!]).toEqual(["README.md"])
    expect(ownerPaths({ kind: "owner", name: "Design" }, files, groups, owners)!.size).toBe(0)
  })

  it("names the chosen owner, and says when it has no files in the part", () => {
    expect(ownerLabel({ kind: "owner", name: "Design" })).toBe("Design")
    expect(ownerLabel({ kind: "unowned" })).toBe("No Owner")
    expect(ownerLabel({ kind: "all" })).toBeNull()
    expect(ownerEmptyText({ kind: "group", name: "Air Plugin" })).toBe("No files of Air Plugin in this range")
    expect(ownerEmptyText({ kind: "unowned" })).toBe("Every file in this range has an owner")
    expect(ownerEmptyText({ kind: "all" })).toBeNull()
  })
})
