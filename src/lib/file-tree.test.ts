// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitFile, Concern, ConcernName, DependencyChange, FileFacts, FileStatus, MechanicalKind, ReviewCommit, ReviewUnit, StepCommit } from "@/lib/bindings"
import { fileMenuItems } from "@/lib/file-actions"
import {
  alsoRowKey,
  buildFileTree,
  buildReviewTree,
  commitRowKey,
  concernRowKey,
  dependencyRows,
  fileActionTarget,
  flattenTreeFiles,
  formatTreeFileLabel,
  foldedGroups,
  getInitiallyCollapsed,
  getVisibleRows,
  groupRowKey,
  listsCommits,
  MECHANICAL_GROUP_KEY,
  mechanicalKindsOf,
  OTHER_CHANGES_KEY,
  OTHER_COMMITS_KEY,
  revealInTree,
  sectionRows,
} from "@/lib/file-tree"
import type { FileTreeAlso, FileTreeCommit, FileTreeDirectory, FileTreeNode, ReviewCommitItem } from "@/lib/file-tree"
import { concernChapters, placeCommits, presentChapters, presentGroupCounts, presentOwnerMarks } from "@/presenters/review-steps"
import { moduleShape, reviewStep, stepGroup } from "@/test-support/review-step"

function file(path: string, additions = 1, deletions = 0, status: FileStatus = "modified", oldPath: string | null = null): CommitFile {
  return { path, status, oldPath, additions, deletions, binary: false, oldId: null, newId: null, submodule: false }
}

/** `name` per row, indented by depth, directories with a trailing slash, a file "also in" a concern with its words, a commit after "@" */
function outline(nodes: readonly FileTreeNode[], collapsed: ReadonlySet<string> = new Set()): string[] {
  return getVisibleRows(nodes, collapsed).map(
    node => `${"  ".repeat(node.depth)}${node.kind === "commit" ? "@ " : ""}${node.name}${node.kind === "directory" ? "/" : node.kind === "also" ? `, ${node.also}` : ""}`,
  )
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
    file("src/a/Old.lock", 0, 4, "deleted"),
    file("README.md"),
  ]
  const mechanical = new Map<string, MechanicalKind>([
    ["src/b/b.iml", "buildModel"],
    ["src/Base.kt", "whitespace"],
    ["src/a/Old.lock", "generated"],
  ])

  it("folds the mechanical files into one group at the end, a group per kind, each file by its path; J and K reach them last", () => {
    const tree = buildReviewTree(files, mechanical, [])
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
      "  Generated (1)/",
      "    src/a/Old.lock",
    ])
    expect(flattenTreeFiles(tree).map(it => it.path)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "README.md", "src/b/b.iml", "src/Base.kt", "src/a/Old.lock"])
    const group = tree.at(-1) as FileTreeDirectory
    expect([group.key, group.group, group.fileCount, group.additions, group.deletions]).toEqual([MECHANICAL_GROUP_KEY, "mechanical", 3, 4, 5])
    // a tree without a mechanical file has no group
    expect(outline(buildReviewTree(files.slice(0, 2), mechanical, []))).toEqual(outline(buildFileTree(files.slice(0, 2))))
    expect(outline(buildReviewTree([files[2]!], mechanical, []))).toEqual(["Mechanical (1)/", "  Build files (1)/", "    src/b/b.iml"])
  })

  it("orders the kinds as the units of the plan, the order Rust gave the story", () => {
    const facts = (path: string, mechanical: MechanicalKind | null) => ({ path, mechanical }) as FileFacts
    const unit = (mechanical: MechanicalKind) => ({ kind: { kind: "mechanical", mechanical } }) as ReviewUnit
    const kinds = mechanicalKindsOf({
      files: [facts("src/Base.kt", "whitespace"), facts("src/a/Reader.kt", null), facts("src/a/Old.lock", "generated"), facts("src/b/b.iml", "buildModel")],
      units: [unit("generated"), unit("buildModel"), unit("whitespace")],
    })
    expect([...kinds]).toEqual([
      ["src/a/Old.lock", "generated"],
      ["src/b/b.iml", "buildModel"],
      ["src/Base.kt", "whitespace"],
    ])
    expect(outline(buildReviewTree(files, kinds, [])).slice(-7)).toEqual([
      "Mechanical (3)/",
      "  Generated (1)/",
      "    src/a/Old.lock",
      "  Build files (1)/",
      "    src/b/b.iml",
      "  Whitespace only (1)/",
      "    src/Base.kt",
    ])
  })

  it("starts with the Mechanical group collapsed, and opens it to reveal a file in it", () => {
    const tree = buildReviewTree(files, mechanical, [])
    const collapsed = getInitiallyCollapsed(tree, files.length)
    expect([...collapsed]).toEqual([MECHANICAL_GROUP_KEY])
    expect(outline(tree, collapsed).at(-1)).toBe("Mechanical (3)/")
    expect([...revealInTree(collapsed, "src/Base.kt", tree)]).toEqual([])
    expect(revealInTree(collapsed, "src/a/Reader.kt", tree)).toBe(collapsed)
  })

  it("copies the paths of a group, which has no path of its own", () => {
    const group = buildReviewTree(files, mechanical, []).at(-1) as FileTreeDirectory
    expect(fileActionTarget(group)).toEqual({ kind: "group", files: ["src/b/b.iml", "src/Base.kt", "src/a/Old.lock"] })
    expect(fileMenuItems(fileActionTarget(group), { copyKey: true }).map(item => item.label)).toEqual(["Copy Changed Paths"])
  })

  it("shows the dependency changes of a build file on the row of its concern, else on the directory row of its module, else on the Mechanical group that folds it", () => {
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
    const tree = buildReviewTree([...files, file("Cargo.toml"), file("lib/x/x.iml")], new Map([...mechanical, ["Cargo.toml", "buildModel"], ["lib/x/x.iml", "buildModel"]]), [])
    const rows = dependencyRows(tree, [change("src/b/b.iml", "b"), change("Cargo.toml", "serde"), change("lib/x/x.iml", "x"), change("gone/y.iml", "y")])
    expect([...rows].map(([key, changes]) => [key, changes.map(it => it.name)])).toEqual([
      ["src/b/", ["b"]],
      ["//mechanical/", ["serde", "x"]],
    ])
    // a build file that follows the code of its module is held by the concern of that code, which shows its changes
    const holding: Concern = {
      id: "s",
      name: { source: "declaration", name: "Store" },
      parts: [
        { unit: "declaration:app/User.kt", path: "app/User.kt", blocks: null },
        { unit: "mechanical:buildModel", path: "app/app.iml", blocks: null },
      ],
      paths: ["app/User.kt", "app/app.iml"],
      elsewhere: [],
      evidence: [],
      fingerprint: "s",
    }
    // beside a second task: one task that covers the whole range is no story the tree tells
    const second: Concern = {
      ...holding,
      id: "t",
      name: { source: "token", token: "readme" },
      parts: [{ unit: "other:README.md", path: "README.md", blocks: null }],
      paths: ["README.md"],
      fingerprint: "t",
    }
    const withConcern = buildReviewTree([file("app/User.kt"), file("app/app.iml"), file("README.md")], new Map([["app/app.iml", "buildModel"]]), concernChapters([holding, second]))
    expect([...dependencyRows(withConcern, [change("app/app.iml", "store")]).keys()]).toEqual(["//concern/s/"])
    // the directory a merged row holds: `plugins/air/` is in the row `plugins/air/backend/`
    const merged = buildFileTree([file("plugins/air/backend/A.kt"), file("README.md")])
    expect([...dependencyRows(merged, [change("plugins/air/air.iml", "air")]).keys()]).toEqual(["plugins/air/backend/"])
  })
})

describe("the concerns in the tree of a review", () => {
  // the `review_concerns` template: a server dropped from three configs and the guide, grep replaced by accio in the
  // guide, its list of tools and AGENTS.md rendered from it; a change no evidence joined, a lock file
  const files = [
    file(".codex/config.toml", 0, 4),
    file(".mcp.json", 0, 5),
    file("AGENTS.md", 1, 1),
    file("docs/guide.md", 1, 2),
    file("docs/tools.md", 1, 1),
    file("pnpm-lock.yaml", 1, 0),
    file("src/App.kt", 1, 1),
    file("tools/codex/config.toml", 0, 4),
  ]
  const mechanical = new Map<string, MechanicalKind>([
    ["AGENTS.md", "generated"],
    ["pnpm-lock.yaml", "generated"],
  ])
  const concern = (id: string, name: ConcernName, paths: string[], elsewhere: string[] = []): Concern => ({
    id,
    name,
    parts: paths.map(path => ({ unit: `other:${path}`, path, blocks: null })),
    paths,
    elsewhere,
    evidence: name.source === "rest" ? [] : [{ kind: "token", detail: "ij-mcp-proxy", joins: 2 }],
    fingerprint: id,
  })
  const concerns = [
    concern("p", { source: "token", token: "ijproxy" }, [".codex/config.toml", "docs/guide.md", "tools/codex/config.toml", ".mcp.json"], ["docs/guide.md"]),
    concern("g", { source: "replacement", removed: "grep", added: "accio" }, ["docs/guide.md", "docs/tools.md", "AGENTS.md"]),
    concern("r", { source: "rest" }, ["src/App.kt"]),
  ]

  it("lists the concerns, each with its files by their path, a file of another concern also, its generated files folded; then Other changes and Mechanical", () => {
    const tree = buildReviewTree(files, mechanical, concernChapters(concerns))
    expect(outline(tree)).toEqual([
      "ijproxy/",
      "  .codex/config.toml",
      "  docs/guide.md, also in ijproxy",
      "  tools/codex/config.toml",
      "  .mcp.json",
      "grep → accio/",
      "  docs/guide.md",
      "  docs/tools.md",
      "  Generated (1)/",
      "    AGENTS.md",
      "Other changes/",
      "  src/",
      "    App.kt",
      "Mechanical (1)/",
      "  Generated (1)/",
      "    pnpm-lock.yaml",
    ])
    // one row of a file: the row "also in" counts in no group and no position, and has a key of its own
    expect(flattenTreeFiles(tree).map(it => it.path)).toEqual([
      ".codex/config.toml",
      "tools/codex/config.toml",
      ".mcp.json",
      "docs/guide.md",
      "docs/tools.md",
      "AGENTS.md",
      "src/App.kt",
      "pnpm-lock.yaml",
    ])
    const [proxy, replacement] = tree as FileTreeDirectory[]
    expect([proxy!.key, proxy!.group, proxy!.fileCount, proxy!.additions, proxy!.deletions, proxy!.title]).toEqual([
      concernRowKey("p"),
      "concern",
      3,
      0,
      13,
      "Joined by the identifier ij-mcp-proxy",
    ])
    const also = proxy!.children[1] as FileTreeAlso
    expect([also.key, also.concern, also.home]).toEqual([alsoRowKey("p", "docs/guide.md"), "p", "Its own row is under grep → accio"])
    expect(replacement!.fileCount).toBe(3)
    // the generated files of a concern and the Mechanical group start collapsed, the directories of Other changes open
    const collapsed = getInitiallyCollapsed(tree, files.length)
    expect([...collapsed]).toEqual([`${concernRowKey("g")}generated/`, MECHANICAL_GROUP_KEY])
    expect([...revealInTree(collapsed, "AGENTS.md", tree)]).toEqual([MECHANICAL_GROUP_KEY])
    expect(tree.find(node => node.key === OTHER_CHANGES_KEY)?.kind).toBe("directory")
  })

  it("shows a file of what no evidence joined that is also in a concern in its place under Other changes", () => {
    const rest = concern("r", { source: "rest" }, ["src/App.kt", "docs/tools.md"], ["docs/tools.md"])
    const tree = buildReviewTree(files, mechanical, concernChapters([concerns[0]!, concerns[1]!, rest]))
    expect(outline(tree).slice(10, 15)).toEqual(["Other changes/", "  docs/", "    tools.md, also in Other changes", "  src/", "    App.kt"])
    const other = tree.find(node => node.key === OTHER_CHANGES_KEY) as FileTreeDirectory
    const docs = other.children[0] as FileTreeDirectory
    expect([docs.fileCount, docs.additions, docs.deletions]).toEqual([0, 0, 0])

    const mixedTree = buildReviewTree([...files, file("docs/own.md", 4, 2)], mechanical, concernChapters([concerns[0]!, concerns[1]!, rest]))
    const mixedOther = mixedTree.find(node => node.key === OTHER_CHANGES_KEY) as FileTreeDirectory
    const mixedDocs = mixedOther.children[0] as FileTreeDirectory
    expect([mixedDocs.fileCount, mixedDocs.additions, mixedDocs.deletions]).toEqual([1, 4, 2])
  })

  it("is the tree without concerns while the plan has none but the rest, and leaves out a concern the filter empties", () => {
    expect(outline(buildReviewTree(files, mechanical, concernChapters([concerns[2]!])))).toEqual(outline(buildReviewTree(files, mechanical, [])))
    const filtered = files.filter(it => it.path.includes("tools"))
    expect(outline(buildReviewTree(filtered, mechanical, concernChapters(concerns)))).toEqual(["ijproxy/", "  tools/codex/config.toml", "grep → accio/", "  docs/tools.md"])
  })

  // the commits of the range, newest first, with full ids; the plan names them short
  const item = (id: string, subject: string): ReviewCommitItem => ({ kind: "commit", key: id, commit: { id, subject } as ReviewCommit })
  const items = [item("d4d4d4d4", "bump the lock"), item("c3c3c3c3", "tidy App"), item("b2b2b2b2", "drop grep"), item("a1a1a1a1", "drop the proxy")]
  const commitOf = (id: string, role: StepCommit["role"]): StepCommit => ({ id: id.slice(0, 4), title: items.find(it => it.key === id)!.commit.subject, role })
  const steps = [
    { id: "p", concern: "p", commits: [commitOf("a1a1a1a1", "starts"), commitOf("b2b2b2b2", "noStory")] },
    { id: "g", concern: "g", commits: [commitOf("b2b2b2b2", "starts")] },
    { id: "r", concern: "r", commits: [commitOf("c3c3c3c3", "noStory")] },
  ]

  it("lists the commits of each step above its files, and those of no group (what no evidence joined, no step) last in Other commits, as the range lists them", () => {
    const tree = buildReviewTree(files, mechanical, concernChapters(concerns), placeCommits(steps, items))
    expect(outline(tree)).toEqual([
      "ijproxy/",
      "  @ drop the proxy",
      "  .codex/config.toml",
      "  docs/guide.md, also in ijproxy",
      "  tools/codex/config.toml",
      "  .mcp.json",
      "grep → accio/",
      "  @ drop grep",
      "  docs/guide.md",
      "  docs/tools.md",
      "  Generated (1)/",
      "    AGENTS.md",
      "Other changes/",
      "  src/",
      "    App.kt",
      "Mechanical (1)/",
      "  Generated (1)/",
      "    pnpm-lock.yaml",
      "Other commits/",
      "  @ bump the lock",
      "  @ tidy App",
    ])
    // a commit row is no file: the files, their counts and what starts collapsed are those of the tree without commits
    const without = buildReviewTree(files, mechanical, concernChapters(concerns))
    expect(flattenTreeFiles(tree)).toEqual(flattenTreeFiles(without))
    expect((tree[0] as FileTreeDirectory).fileCount).toBe(3)
    expect([...getInitiallyCollapsed(tree, files.length)]).toEqual([...getInitiallyCollapsed(without, files.length)])
    const proxy = (tree[0] as FileTreeDirectory).children[0] as FileTreeCommit
    expect([proxy.key, proxy.chapter, proxy.commit.role, proxy.commit.item?.key]).toEqual([commitRowKey(concernRowKey("p"), "a1a1a1a1"), "p", "starts", "a1a1a1a1"])
    const other = tree.at(-1) as FileTreeDirectory
    expect([other.key, other.group, other.commitCount, other.fileCount]).toEqual([OTHER_COMMITS_KEY, "commits", 2, 0])
    expect(other.children.map(child => (child as FileTreeCommit).chapter)).toEqual([null, null])
  })

  it("keeps the group of a step that lands nothing for its commits, and lists every commit under Commits without a step", () => {
    const filtered = files.filter(it => it.path.startsWith("docs/tools"))
    expect(outline(buildReviewTree(filtered, mechanical, concernChapters(concerns), placeCommits(steps, items)))).toEqual([
      "ijproxy/",
      "  @ drop the proxy",
      "grep → accio/",
      "  @ drop grep",
      "  docs/tools.md",
      "Other commits/",
      "  @ bump the lock",
      "  @ tidy App",
    ])
    expect(outline(buildReviewTree(filtered, mechanical, [], placeCommits(steps, items)))).toEqual([
      "docs/",
      "  tools.md",
      "Commits/",
      "  @ bump the lock",
      "  @ tidy App",
      "  @ drop grep",
      "  @ drop the proxy",
    ])
  })

  it("lists under a search only the commits of the steps whose files it found, no Other commits, and says when it lists none", () => {
    const filtered = files.filter(it => it.path.startsWith("docs/tools"))
    const tree = buildReviewTree(filtered, mechanical, concernChapters(concerns), placeCommits(steps, items), true)
    expect(outline(tree)).toEqual(["grep → accio/", "  @ drop grep", "  docs/tools.md"])
    expect(listsCommits(tree)).toBe(true)
    // a search that finds files of no step: no commit row, the column lists the commits
    const unlisted = buildReviewTree(
      files.filter(it => it.path === "src/App.kt"),
      mechanical,
      concernChapters(concerns),
      placeCommits(steps, items),
      true,
    )
    expect(outline(unlisted)).toEqual(["Other changes/", "  src/", "    App.kt"])
    expect(listsCommits(unlisted)).toBe(false)
    expect(listsCommits(buildReviewTree(filtered, mechanical, [], placeCommits(steps, items), true))).toBe(false)
    expect(listsCommits(buildReviewTree(files, mechanical, concernChapters(concerns)))).toBe(false)
  })

  it("selects the row of a section in its concern, and shows the first open section of a row", () => {
    const tree = buildReviewTree(files, mechanical, concernChapters(concerns))
    const section = (key: string, path: string, concern: string | null, folded: string | null = null) => ({ key, path, concern, folded, ghost: null })
    const { rowOf, sectionOf } = sectionRows(
      [
        section("file:.codex/config.toml", ".codex/config.toml", "p"),
        section("file:docs/guide.md#1", "docs/guide.md", "p"),
        section("file:docs/guide.md", "docs/guide.md", "g"),
        section("group:mechanical:generated:AGENTS.md", "AGENTS.md", "g", "Generated"),
        section("group:mechanical:generated:pnpm-lock.yaml", "pnpm-lock.yaml", null, "Lock file"),
      ],
      tree,
    )
    expect([...rowOf]).toEqual([
      ["file:.codex/config.toml", ".codex/config.toml"],
      ["file:docs/guide.md#1", alsoRowKey("p", "docs/guide.md")],
      ["file:docs/guide.md", "docs/guide.md"],
      ["group:mechanical:generated:AGENTS.md", "AGENTS.md"],
      ["group:mechanical:generated:pnpm-lock.yaml", "pnpm-lock.yaml"],
    ])
    expect(sectionOf.get("docs/guide.md")).toBe("file:docs/guide.md")
    expect(sectionOf.get(alsoRowKey("p", "docs/guide.md"))).toBe("file:docs/guide.md#1")
    expect(sectionOf.get("AGENTS.md")).toBe("group:mechanical:generated:AGENTS.md")
  })
})

describe("the steps of the commits in the tree of a review", () => {
  // a branch of IJAI-1: the rule, a round of it and a commit without a body in its step, the paint, a commit of no step;
  // the range lists full ids, newest first, the plan names them short
  const files = [file("src/Rule.kt", 3, 1), file("src/Paint.kt", 2, 0)]
  const item = (id: string, subject: string): ReviewCommitItem => ({ kind: "commit", key: id, commit: { id, subject } as ReviewCommit })
  const items = [
    item("e5e5e5e5", "IJAI-1 tidy"),
    item("d4d4d4d4", "IJAI-1 wip"),
    item("c3c3c3c3", "IJAI-1 the rule names its flag"),
    item("b2b2b2b2", "IJAI-1 the paint"),
    item("a1a1a1a1", "IJAI-1 the rule"),
  ]
  // Rust titles a commit of a step by its subject without the issue key of the brief, in its author's case
  const commitOf = (id: string, role: StepCommit["role"]): StepCommit => ({
    id: id.slice(0, 4),
    title: items.find(it => it.key === id)!.commit.subject.replace("IJAI-1 ", ""),
    role,
  })
  const steps = [
    reviewStep(
      "a1a1",
      [commitOf("a1a1a1a1", "starts"), commitOf("c3c3c3c3", "followsUp"), commitOf("d4d4d4d4", "noStory")],
      [["u:rule", "src/Rule.kt"]],
      {},
      { title: "the rule" },
    ),
    reviewStep("b2b2", [commitOf("b2b2b2b2", "starts")], [["u:paint", "src/Paint.kt"]], {}, { title: "the paint" }),
  ]
  const chapters = presentChapters({
    steps: { source: { kind: "commits" }, told: true, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [] },
    concerns: [],
  })
  const places = placeCommits(steps, items)

  it("makes the group of a step the commit that starts it, and lists its rounds and the commits of no story without the issue key, in the case its author wrote it", () => {
    const tree = buildReviewTree(files, new Map(), chapters, places)
    expect(outline(tree)).toEqual([
      "1 · the rule/",
      "  @ the rule names its flag",
      "  @ wip",
      "  src/Rule.kt",
      "2 · the paint/",
      "  src/Paint.kt",
      "Other commits/",
      "  @ IJAI-1 tidy",
    ])
    const rule = tree[0] as FileTreeDirectory
    // the row of the step is its commit: by the key of the group, its subject whole in its hover
    expect([rule.commit?.key, rule.commit?.chapter, rule.commit?.commit.role, rule.commit?.commit.item?.key, rule.commit?.commit.subject]).toEqual([
      concernRowKey("a1a1"),
      "a1a1",
      "starts",
      "a1a1a1a1",
      "IJAI-1 the rule",
    ])
    expect(rule.children.map(child => (child.kind === "commit" ? [child.commit.role, child.commit.subject] : child.key))).toEqual([
      ["followsUp", "IJAI-1 the rule names its flag"],
      ["noStory", "IJAI-1 wip"],
      "src/Rule.kt",
    ])
    expect(listsCommits(tree)).toBe(true)
  })

  it("keeps the group of a step that lands nothing until narrowed, then lists only steps with shown files", () => {
    // a step of its commit alone that lands nothing in the scope: the group is its commit, and holds no row
    const alone = buildReviewTree(
      files.filter(it => it.path === "src/Rule.kt"),
      new Map(),
      chapters,
      places,
    )
    expect(outline(alone)).toEqual(["1 · the rule/", "  @ the rule names its flag", "  @ wip", "  src/Rule.kt", "2 · the paint/", "Other commits/", "  @ IJAI-1 tidy"])
    const paintStep = alone[1] as FileTreeDirectory
    expect([paintStep.commit?.commit.subject, paintStep.children, paintStep.fileCount]).toEqual(["IJAI-1 the paint", [], 0])
    const paint = files.filter(it => it.path === "src/Paint.kt")
    expect(outline(buildReviewTree(paint, new Map(), chapters, places))).toEqual([
      "1 · the rule/",
      "  @ the rule names its flag",
      "  @ wip",
      "2 · the paint/",
      "  src/Paint.kt",
      "Other commits/",
      "  @ IJAI-1 tidy",
    ])
    const narrowed = buildReviewTree(paint, new Map(), chapters, places, true)
    expect(outline(narrowed)).toEqual(["2 · the paint/", "  src/Paint.kt"])
    // the step alone is its commit: the tree lists a commit
    expect(listsCommits(narrowed)).toBe(true)
    expect(outline(buildReviewTree(files, new Map(), [], places))).toEqual([
      "src/",
      "  Paint.kt",
      "  Rule.kt",
      // the commits of the steps by the titles Rust gives them, a commit no step lists by its subject
      "Commits/",
      "  @ IJAI-1 tidy",
      "  @ wip",
      "  @ the rule names its flag",
      "  @ the paint",
      "  @ the rule",
    ])
  })

  it("names the step of the own row of a file shown in another step after its name", () => {
    // the paint changes the rule's file too, whose own row is in the first step: its row there says where it lives
    const shared = [
      reviewStep("a1a1", [commitOf("a1a1a1a1", "starts")], [["u:rule", "src/Rule.kt"]], {}, { title: "the rule" }),
      {
        ...reviewStep(
          "b2b2",
          [commitOf("b2b2b2b2", "starts")],
          [
            ["u:paint", "src/Paint.kt"],
            ["u:rule2", "src/Rule.kt"],
          ],
          {},
          { title: "the paint" },
        ),
        elsewhere: ["src/Rule.kt"],
      },
    ]
    const sharedChapters = presentChapters({
      steps: { source: { kind: "commits" }, told: true, steps: shared, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [] },
      concerns: [],
    })
    const tree = buildReviewTree(files, new Map(), sharedChapters)
    const also = (tree[1] as FileTreeDirectory).children.find(child => child.kind === "also")
    expect([also?.kind === "also" && also.homeTag, also?.kind === "also" && also.home, also?.kind === "also" && also.also]).toEqual([
      "in step 1",
      "Its own row is under 1 · the rule",
      "also in 2 · the paint",
    ])
  })
})

describe("the groups of a large step in the tree of a review", () => {
  const ROOT = "plugins/player"
  const paths = {
    spec: `${ROOT}/spec/embeds.spec.md`,
    kind: `${ROOT}/view.gui/src/com/example/gui/embeds/EmbedKind.kt`,
    markdown: `${ROOT}/view.gui/src/com/example/gui/markdown/MarkdownEmbeds.kt`,
    bundle: `${ROOT}/view.gui/resources/messages/PlayerBundle.properties`,
    guiTest: `${ROOT}/view.gui/testSrc/com/example/gui/embeds/EmbedModelTest.kt`,
    nightly: `${ROOT}/shared/code/src/com/example/shared/NightlyBuild.kt`,
    player: `${ROOT}/view.gui.media/src/com/example/media/MediaPlayer.kt`,
    decoder: `${ROOT}/view.gui.media/src/com/example/media/avfoundation/Decoder.kt`,
    mediaTest: `${ROOT}/view.gui.media/testSrc/com/example/media/MediaPlayerTest.kt`,
    build: `${ROOT}/view.gui.media/BUILD.bazel`,
    list: "build/module-list.txt",
    lock: "pnpm-lock.yaml",
  }
  const files = [
    file(paths.spec, 6),
    file(paths.kind, 2),
    file(paths.markdown, 2),
    file(paths.bundle, 4),
    file(paths.guiTest, 5),
    file(paths.nightly, 2, 2),
    file(paths.player, 12, 0, "added"),
    file(paths.decoder, 14, 0, "added"),
    file(paths.mediaTest, 11, 0, "added"),
    file(paths.build, 20, 0, "added"),
    file(paths.list, 1),
  ]
  const modules = [
    moduleShape(`${ROOT}/view.gui.media`, "view.gui.media", { state: "added" }),
    moduleShape(`${ROOT}/view.gui`, "view.gui"),
    moduleShape(`${ROOT}/shared/code`, "shared/code"),
  ]
  const moduleOf: Record<string, number> = {
    [paths.kind]: 1,
    [paths.markdown]: 1,
    [paths.bundle]: 1,
    [paths.guiTest]: 1,
    [paths.nightly]: 2,
    [paths.player]: 0,
    [paths.decoder]: 0,
    [paths.mediaTest]: 0,
    [paths.build]: 0,
  }
  const kinds: Record<string, MechanicalKind> = { [paths.build]: "buildModel", [paths.list]: "moduleList", [paths.lock]: "generated" }
  // the folder of a file as Rust gives it: below what the code and the tests of its module share, the resources apart
  const folderOf: Record<string, string> = { [paths.kind]: "embeds", [paths.markdown]: "markdown", [paths.guiTest]: "embeds", [paths.decoder]: "avfoundation" }
  const facts = (list: readonly CommitFile[]) =>
    list.map(it => ({ path: it.path, mechanical: kinds[it.path] ?? null, module: moduleOf[it.path] ?? null, folder: folderOf[it.path] ?? "" }) as FileFacts)
  /** The step of Rust: its stops group by group, the imports alone of `EmbedKind.kt` a stop of the mechanical group */
  const step = (mechanical: readonly string[]) =>
    reviewStep(
      "media",
      [{ id: "media", title: "play the media embeds", role: "starts" }],
      [
        ["u:spec", paths.spec],
        ["u:bundle", paths.bundle],
        ["u:kind", paths.kind],
        ["u:markdown", paths.markdown],
        ["u:guiTest", paths.guiTest],
        ["u:nightly", paths.nightly],
        ["u:player", paths.player],
        ["u:decoder", paths.decoder],
        ["u:mediaTest", paths.mediaTest],
        ...mechanical.map((path): [string, string] => [`mechanical:${path}`, path]),
        ["mechanical:imports", paths.kind],
      ],
      {},
      {
        title: "play the media embeds",
        groups: [
          stepGroup("contract", null, [0, 1, 1]),
          stepGroup("module", 1, [1, 4, 5]),
          stepGroup("module", 0, [5, 8, 9], { also: [2] }),
          stepGroup("mechanical", null, [9, 10 + mechanical.length, 10 + mechanical.length]),
        ],
      },
    )
  const planOf = (mechanical: readonly string[], list: readonly CommitFile[] = files) => ({
    steps: { source: { kind: "commits" as const }, told: true, steps: [step(mechanical)], coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [] },
    concerns: [],
    modules,
    files: facts(list),
  })
  const plan = planOf([paths.build, paths.list])
  const chapters = presentChapters(plan)
  const mechanical = mechanicalKindsOf({ files: plan.files, units: [] })

  it("lists a group per section of the step, its files by name with the folder Rust gives them in their module, its tests last, the build files under Wiring", () => {
    const tree = buildReviewTree(files, mechanical, chapters)
    expect(outline(tree)).toEqual([
      "1 · play the media embeds/",
      "  Contract/",
      `    ${paths.spec}`,
      "  view.gui/",
      `    ${paths.bundle}`,
      `    ${paths.kind}`,
      `    ${paths.markdown}`,
      "    Tests/",
      `      ${paths.guiTest}`,
      "  view.gui.media/",
      `    ${paths.nightly}`,
      `    ${paths.player}`,
      `    ${paths.decoder}`,
      "    Tests/",
      `      ${paths.mediaTest}`,
      "  Wiring/",
      "    Build files (1)/",
      `      ${paths.build}`,
      "    Module lists (1)/",
      `      ${paths.list}`,
    ])
    const step = tree[0] as FileTreeDirectory
    const [contract, gui, media, wiring] = step.children as FileTreeDirectory[]
    expect([contract, gui, media, wiring].map(group => [group!.key, group!.group, group!.name, group!.code === true, group!.fileCount])).toEqual([
      [groupRowKey("media", 0), "section", "Contract", false, 1],
      [groupRowKey("media", 1), "section", "view.gui", true, 4],
      [groupRowKey("media", 2), "section", "view.gui.media", true, 4],
      [groupRowKey("media", 3), "section", "Wiring", false, 2],
    ])
    // the new module says so, and names the small module whose file it holds; its hover is its root
    expect([media!.section?.state, media!.section?.also, media!.title]).toEqual(["added", "also shared/code", `${ROOT}/view.gui.media`])
    // a file of a module by its folder below what the files of its module share (`FileFacts.folder`: a test in the
    // folder of its code), nothing in that folder itself, a file of the small module by that module; a file of the
    // contract by its whole path
    const directories = (group: FileTreeDirectory) =>
      flattenTreeNodes(group.children).flatMap(node => (node.kind === "file" ? [[node.name.slice(node.name.lastIndexOf("/") + 1), node.directory]] : []))
    expect(directories(gui!)).toEqual([
      ["PlayerBundle.properties", ""],
      ["EmbedKind.kt", "embeds"],
      ["MarkdownEmbeds.kt", "markdown"],
      ["EmbedModelTest.kt", "embeds"],
    ])
    expect(directories(media!)).toEqual([
      ["NightlyBuild.kt", "shared/code"],
      ["MediaPlayer.kt", ""],
      ["Decoder.kt", "avfoundation"],
      ["MediaPlayerTest.kt", ""],
    ])
    expect(directories(contract!)).toEqual([["embeds.spec.md", undefined]])
    // a file whose imports alone are a stop of the mechanical group keeps its row in its module; the Wiring starts folded
    expect(flattenTreeFiles([tree[0]!]).filter(it => it.path === paths.kind)).toHaveLength(1)
    expect(foldedGroups(tree)).toEqual([groupRowKey("media", 3)])
    // the tests of a group open with it in a large review; its other groups fold
    expect([...getInitiallyCollapsed(tree, 200)].toSorted()).toEqual(
      [
        groupRowKey("media", 0),
        groupRowKey("media", 1),
        groupRowKey("media", 2),
        groupRowKey("media", 3),
        `${groupRowKey("media", 3)}buildModel/`,
        `${groupRowKey("media", 3)}moduleList/`,
      ].toSorted(),
    )
    // a file of a group shows: its group and the step open
    expect([...revealInTree(new Set([groupRowKey("media", 2), concernRowKey("media")]), paths.mediaTest, tree)]).toEqual([])
  })

  it("lists a file of a group whose own row is in another step as a row also in that group, by its directory too", () => {
    const first = reviewStep("bundle", [{ id: "bundle", title: "name the texts", role: "starts" }], [["u:bundle", paths.bundle]], {}, { title: "name the texts" })
    const large = { ...step([paths.build, paths.list]), elsewhere: [paths.bundle] }
    const tree = buildReviewTree(files, mechanical, presentChapters({ ...plan, steps: { ...plan.steps, steps: [first, large] } }))
    const gui = (tree[1] as FileTreeDirectory).children[1] as FileTreeDirectory
    const also = gui.children[0]
    expect([also?.kind, also?.kind === "also" && also.homeTag, also?.kind === "also" && also.directory]).toEqual(["also", "in step 1", ""])
    // its own row in the first step, once
    expect(flattenTreeFiles([tree[0]!]).map(it => it.path)).toEqual([paths.bundle])
  })

  it("words the mechanical group Mechanical when it holds more than build files and lists of modules, and keeps a step without groups a list", () => {
    const withLock = planOf([paths.build, paths.lock], [...files.filter(it => it.path !== paths.list), file(paths.lock, 30)])
    const [chapter] = presentChapters(withLock)
    expect(chapter!.groups.map(group => group.name)).toEqual(["Contract", "view.gui", "view.gui.media", "Mechanical"])
    // a step Rust did not cut: no groups, the tree of today
    const plain = presentChapters({ ...plan, steps: { ...plan.steps, steps: [{ ...step([paths.build, paths.list]), groups: [] }] } })
    expect(plain[0]!.groups).toEqual([])
    expect(outline(buildReviewTree(files, mechanical, plain)).slice(0, 3)).toEqual(["1 · play the media embeds/", `  ${paths.spec}`, `  ${paths.bundle}`])
  })

  it("counts the files of an owner chip and the files seen on the step and on each of its groups", () => {
    const seen = new Set([paths.spec, paths.kind, paths.player])
    expect(presentOwnerMarks(chapters, seen).get("media")).toEqual({ shown: 3, total: 11 })
    expect(presentGroupCounts(chapters, seen)).toEqual([
      { chapter: "media", group: 0, shown: 1, total: 1 },
      { chapter: "media", group: 1, shown: 1, total: 4 },
      { chapter: "media", group: 2, shown: 1, total: 4 },
      { chapter: "media", group: 3, shown: 0, total: 2 },
    ])
  })

  it("shows the dependency changes of the build file of a module on the row of its group, the new module its badge alone", () => {
    const change = (kind: DependencyChange["kind"], name: string): DependencyChange => ({
      manifest: paths.build,
      module: "intellij.player.view.gui.media",
      kind,
      name,
      change: "added",
      versionBefore: null,
      versionAfter: null,
      scope: "main",
      scopeBefore: null,
    })
    const tree = buildReviewTree(files, mechanical, chapters)
    const rows = dependencyRows(tree, [change("newModule", "intellij.player.view.gui.media"), change("library", "jna")])
    expect([...rows].map(([key, changes]) => [key, changes.map(it => it.name)])).toEqual([[groupRowKey("media", 2), ["jna"]]])
  })
})

/** Every row of `nodes` with every directory open, in order */
function flattenTreeNodes(nodes: readonly FileTreeNode[]): FileTreeNode[] {
  return getVisibleRows(nodes, new Set())
}
