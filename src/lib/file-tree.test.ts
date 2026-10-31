// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitFile, DependencyChange, FileFacts, FileStatus, MechanicalKind, ReviewUnit } from "@/lib/bindings"
import { fileMenuItems } from "@/lib/file-actions"
import {
  buildFileTree,
  buildReviewTree,
  dependencyRows,
  fileActionTarget,
  flattenTreeFiles,
  formatTreeFileLabel,
  getInitiallyCollapsed,
  getVisibleRows,
  MECHANICAL_GROUP_KEY,
  mechanicalKindsOf,
  revealInTree,
} from "@/lib/file-tree"
import type { FileTreeDirectory, FileTreeNode } from "@/lib/file-tree"

function file(path: string, additions = 1, deletions = 0, status: FileStatus = "modified", oldPath: string | null = null): CommitFile {
  return { path, status, oldPath, additions, deletions, binary: false, oldId: null, newId: null, submodule: false }
}

/** `name` per row, indented by depth, directories with a trailing slash */
function outline(nodes: readonly FileTreeNode[], collapsed: ReadonlySet<string> = new Set()): string[] {
  return getVisibleRows(nodes, collapsed).map(node => `${"  ".repeat(node.depth)}${node.name}${node.kind === "directory" ? "/" : ""}`)
}

const AIR = [
  file("plugins/air/backend/session/catalog/src/RowMapper.kt", 4, 2),
  file("plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt", 13, 0),
  file("plugins/air/backend/session/projection/api/src/Models.kt", 2, 0),
  file("plugins/air/backend/session/projection/testData/approval.txt", 20, 20),
  file("README.md", 1, 1),
]

describe("buildFileTree", () => {
  it("merges a chain of directories with one subdirectory into one row", () => {
    expect(outline(buildFileTree(AIR))).toEqual([
      "plugins/air/backend/session/",
      "  catalog/",
      "    src/",
      "      RowMapper.kt",
      "    testSrc/",
      "      RowMapperTest.kt",
      "  projection/",
      "    api/src/",
      "      Models.kt",
      "    testData/",
      "      approval.txt",
      "README.md",
    ])
  })

  it("sums the files and the line counts of each directory over its subtree", () => {
    const [top] = buildFileTree(AIR) as FileTreeDirectory[]
    expect([top!.fileCount, top!.additions, top!.deletions]).toEqual([4, 39, 22])
    const projection = top!.children[1] as FileTreeDirectory
    expect([projection.name, projection.key, projection.fileCount, projection.additions]).toEqual(["projection", "plugins/air/backend/session/projection/", 2, 22])
  })

  it("puts directories first and sorts names as a person reads them", () => {
    const tree = buildFileTree([file("b.txt"), file("file10.txt"), file("A/x.txt"), file("file2.txt"), file("a.txt"), file("z/y.txt")])
    expect(outline(tree)).toEqual(["A/", "  x.txt", "z/", "  y.txt", "a.txt", "b.txt", "file2.txt", "file10.txt"])
  })

  it("keeps the files of a directory with one subdirectory apart from it", () => {
    expect(outline(buildFileTree([file("src/main/A.kt"), file("src/build.gradle")]))).toEqual(["src/", "  main/", "    A.kt", "  build.gradle"])
  })

  it("takes a path with spaces and letters beyond ASCII as it is", () => {
    expect(outline(buildFileTree([file("dir name/ü\tx.txt")]))).toEqual(["dir name/", "  ü\tx.txt"])
  })

  it("lists the files in the order of the tree", () => {
    expect(flattenTreeFiles(buildFileTree(AIR)).map(it => it.path)).toEqual(AIR.map(it => it.path))
  })
})

describe("tree rows", () => {
  it("hides the rows below a collapsed directory", () => {
    const tree = buildFileTree(AIR)
    expect(outline(tree, new Set(["plugins/air/backend/session/catalog/"]))).toEqual([
      "plugins/air/backend/session/",
      "  catalog/",
      "  projection/",
      "    api/src/",
      "      Models.kt",
      "    testData/",
      "      approval.txt",
      "README.md",
    ])
  })

  it("collapses the directories below the top level of a large commit only", () => {
    const tree = buildFileTree(AIR)
    expect(getInitiallyCollapsed(tree, AIR.length).size).toBe(0)
    expect([...getInitiallyCollapsed(tree, 101)]).toEqual([
      "plugins/air/backend/session/catalog/",
      "plugins/air/backend/session/catalog/src/",
      "plugins/air/backend/session/catalog/testSrc/",
      "plugins/air/backend/session/projection/",
      "plugins/air/backend/session/projection/api/src/",
      "plugins/air/backend/session/projection/testData/",
    ])
  })

  it("opens the directories above a row to reveal it, and only those", () => {
    const tree = buildFileTree(AIR)
    const collapsed = new Set(["plugins/air/backend/session/", "plugins/air/backend/session/catalog/", "plugins/air/backend/session/projection/"])
    expect([...revealInTree(collapsed, "plugins/air/backend/session/catalog/src/RowMapper.kt", tree)]).toEqual(["plugins/air/backend/session/projection/"])
    expect(revealInTree(collapsed, "README.md", tree)).toBe(collapsed)
    // a directory row stays collapsed itself
    expect([...revealInTree(collapsed, "plugins/air/backend/session/catalog/", tree)]).toEqual(["plugins/air/backend/session/catalog/", "plugins/air/backend/session/projection/"])
  })
})

describe("formatTreeFileLabel", () => {
  it("names a file by its name in its directory", () => {
    expect(formatTreeFileLabel(file("src/main/A.kt"))).toEqual({ name: "A.kt", from: null })
  })

  it("names where a renamed file came from, relative to the directory both paths share", () => {
    expect(formatTreeFileLabel(file("src/New.kt", 0, 0, "renamed", "src/Old.kt"))).toEqual({ name: "New.kt", from: "Old.kt" })
    expect(formatTreeFileLabel(file("src/impl/A.kt", 0, 0, "renamed", "src/api/A.kt"))).toEqual({ name: "A.kt", from: "api/A.kt" })
    expect(formatTreeFileLabel(file("A.kt", 0, 0, "copied", "lib/A.kt"))).toEqual({ name: "A.kt", from: "lib/A.kt" })
  })
})

describe("the tree of a review with its plan", () => {
  const files = [
    file("src/a/Reader.kt"),
    file("src/b/Cache.kt"),
    file("src/b/b.iml", 3, 0, "added"),
    file("src/Base.kt", 1, 1),
    file("src/a/Old.kt", 0, 4, "deleted"),
    file("README.md"),
  ]
  const mechanical = new Map<string, MechanicalKind>([
    ["src/b/b.iml", "buildModel"],
    ["src/Base.kt", "whitespace"],
    ["src/a/Old.kt", "deleted"],
  ])

  it("folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last", () => {
    const tree = buildReviewTree(files, mechanical)
    expect(outline(tree)).toEqual([
      "src/",
      "  a/",
      "    Reader.kt",
      "  b/",
      "    Cache.kt",
      "README.md",
      "Mechanical (3)/",
      "  Build files (1)/",
      "    src/b/b.iml",
      "  Whitespace only (1)/",
      "    src/Base.kt",
      "  Deleted (1)/",
      "    src/a/Old.kt",
    ])
    expect(flattenTreeFiles(tree).map(it => it.path)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "README.md", "src/b/b.iml", "src/Base.kt", "src/a/Old.kt"])
    const group = tree.at(-1) as FileTreeDirectory
    expect([group.key, group.group, group.fileCount, group.additions, group.deletions]).toEqual([MECHANICAL_GROUP_KEY, "mechanical", 3, 4, 5])
    // a tree without a mechanical file has no group
    expect(outline(buildReviewTree(files.slice(0, 2), mechanical))).toEqual(outline(buildFileTree(files.slice(0, 2))))
  })

  it("orders the kinds as the units of the plan, the order Rust gave the story", () => {
    const facts = (path: string, mechanical: MechanicalKind | null) => ({ path, mechanical }) as FileFacts
    const unit = (mechanical: MechanicalKind) => ({ kind: { kind: "mechanical", mechanical } }) as ReviewUnit
    const kinds = mechanicalKindsOf({
      files: [facts("src/Base.kt", "whitespace"), facts("src/a/Reader.kt", null), facts("src/a/Old.kt", "deleted"), facts("src/b/b.iml", "buildModel")],
      units: [unit("deleted"), unit("buildModel"), unit("whitespace")],
    })
    expect([...kinds]).toEqual([
      ["src/a/Old.kt", "deleted"],
      ["src/b/b.iml", "buildModel"],
      ["src/Base.kt", "whitespace"],
    ])
    expect(outline(buildReviewTree(files, kinds)).slice(-7)).toEqual([
      "Mechanical (3)/",
      "  Deleted (1)/",
      "    src/a/Old.kt",
      "  Build files (1)/",
      "    src/b/b.iml",
      "  Whitespace only (1)/",
      "    src/Base.kt",
    ])
  })

  it("starts with the Mechanical group collapsed, and opens it to reveal a file in it", () => {
    const tree = buildReviewTree(files, mechanical)
    const collapsed = getInitiallyCollapsed(tree, files.length)
    expect([...collapsed]).toEqual([MECHANICAL_GROUP_KEY])
    expect(outline(tree, collapsed).at(-1)).toBe("Mechanical (3)/")
    expect([...revealInTree(collapsed, "src/Base.kt", tree)]).toEqual([])
    expect(revealInTree(collapsed, "src/a/Reader.kt", tree)).toBe(collapsed)
  })

  it("copies the paths of a group, which has no path of its own", () => {
    const group = buildReviewTree(files, mechanical).at(-1)!
    expect(fileActionTarget(group)).toEqual({ kind: "group", files: ["src/b/b.iml", "src/Base.kt", "src/a/Old.kt"] })
    expect(fileMenuItems(fileActionTarget(group), { copyKey: true }).map(item => item.label)).toEqual(["Copy Changed Paths"])
  })

  it("shows the dependency changes of a build file on the directory row of its module, else on the row of the build file", () => {
    const change = (manifest: string, name: string): DependencyChange => ({
      manifest,
      module: name,
      kind: "module",
      name,
      change: "added",
      versionBefore: null,
      versionAfter: null,
      scope: "main",
      scopeBefore: null,
    })
    const tree = buildReviewTree([...files, file("Cargo.toml"), file("lib/x/x.iml")], new Map([...mechanical, ["Cargo.toml", "buildModel"], ["lib/x/x.iml", "buildModel"]]))
    const rows = dependencyRows(tree, [change("src/b/b.iml", "b"), change("Cargo.toml", "serde"), change("lib/x/x.iml", "x"), change("gone/y.iml", "y")])
    expect([...rows].map(([key, changes]) => [key, changes.map(it => it.name)])).toEqual([
      ["src/b/", ["b"]],
      ["Cargo.toml", ["serde"]],
      ["lib/x/x.iml", ["x"]],
    ])
    // the directory a merged row holds: `plugins/air/` is in the row `plugins/air/backend/`
    const merged = buildFileTree([file("plugins/air/backend/A.kt"), file("README.md")])
    expect([...dependencyRows(merged, [change("plugins/air/air.iml", "air")]).keys()]).toEqual(["plugins/air/backend/"])
  })
})
