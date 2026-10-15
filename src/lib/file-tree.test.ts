// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vitest"
import type { CommitFile, FileStatus } from "@/lib/bindings"
import { buildFileTree, flattenTreeFiles, formatTreeFileLabel, getInitiallyCollapsed, getVisibleRows, revealInTree } from "@/lib/file-tree"
import type { FileTreeDirectory, FileTreeNode } from "@/lib/file-tree"

function file(path: string, additions = 1, deletions = 0, status: FileStatus = "modified", oldPath: string | null = null): CommitFile {
  return { path, status, oldPath, additions, deletions, binary: false }
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
    const collapsed = new Set(["plugins/air/backend/session/", "plugins/air/backend/session/catalog/", "plugins/air/backend/session/projection/"])
    expect([...revealInTree(collapsed, "plugins/air/backend/session/catalog/src/RowMapper.kt")]).toEqual(["plugins/air/backend/session/projection/"])
    expect(revealInTree(collapsed, "README.md")).toBe(collapsed)
    // a directory row stays collapsed itself
    expect([...revealInTree(collapsed, "plugins/air/backend/session/catalog/")]).toEqual(["plugins/air/backend/session/catalog/", "plugins/air/backend/session/projection/"])
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
