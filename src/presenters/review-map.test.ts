import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, ContractEntry, DependencyChange, FileFacts } from "@/lib/bindings"
import { MAP_ROWS, presentMap, specName } from "@/presenters/review-map"
import { moduleShape, reviewPlan } from "@/test-support/review-step"

function file(path: string, module: number | null): FileFacts {
  return {
    path,
    oldPath: null,
    mechanical: null,
    generated: null,
    uses: 0,
    usedBy: 0,
    newLinks: 0,
    breaks: 0,
    breaksComplete: true,
    untested: false,
    repeatedEdit: null,
    repeatedEditOnly: false,
    module,
  }
}

function entry(path: string, kind: ContractEntry["kind"], rules: number | null = null): ContractEntry {
  return {
    kind,
    unit: `other:${path}`,
    path,
    blocks: [0],
    heading: null,
    newSpec: rules === null ? null : { description: null, rules },
    deleted: false,
    paragraphs: [],
    provenBy: [],
  }
}

const widened: ApiChange = {
  path: "shared/src/Flag.kt",
  line: 3,
  name: "isNightly",
  key: "s.isNightly",
  kind: "added",
  widened: true,
  changes: ["reach"],
  before: "private fun isNightly(): Boolean",
  after: "fun isNightly(): Boolean",
  usages: null,
}

const external: DependencyChange = {
  manifest: "gui/showcase/intellij.gui.showcase.iml",
  module: "intellij.gui.showcase",
  kind: "module",
  name: "intellij.platform.ide.util.io",
  change: "added",
  versionBefore: null,
  versionAfter: null,
  scope: "main",
  scopeBefore: null,
}

describe("the map of a review", () => {
  it("names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code", () => {
    const plan = reviewPlan({
      modules: [
        moduleShape("gui/media", "view.gui.media", {
          state: "added",
          files: { code: 39, tests: 17, resources: 1, build: 4, prose: 0 },
          additions: 5980,
          deletions: 13,
          uses: [
            { module: 1, links: [0, 1], declared: true },
            { module: 3, links: [2], declared: true },
          ],
          contentOf: ["com.intellij.air"],
        }),
        moduleShape("gui", "view.gui", {
          files: { code: 36, tests: 9, resources: 2, build: 0, prose: 0 },
          additions: 3083,
          deletions: 129,
          directories: [
            { path: "ui/embeds", files: 29 },
            { path: "ui/markdown", files: 11 },
            { path: "ui/util", files: 2 },
            { path: "references", files: 2 },
          ],
        }),
        moduleShape("gui/showcase", "view.gui.showcase", { files: { code: 10, tests: 3, resources: 1, build: 2, prose: 0 }, additions: 957, deletions: 63, newDependencies: [0] }),
        moduleShape("shared", "shared/code", { files: { code: 1, tests: 1, resources: 0, build: 0, prose: 0 }, additions: 56, deletions: 2 }),
        // build files alone: no row of the shape
        moduleShape("", "(root)", { files: { code: 0, tests: 0, resources: 0, build: 1, prose: 0 }, additions: 2 }),
      ],
      files: [file("gui/media/src/Player.kt", 0), file("gui/src/Embed.kt", 1), file("gui/showcase/src/Clip.kt", 2), file("shared/src/Flag.kt", 3), file(".idea/modules.xml", 4)],
      order: ["gui/src/Embed.kt", "gui/media/src/Player.kt", "gui/showcase/src/Clip.kt", "shared/src/Flag.kt", ".idea/modules.xml"],
      api: [widened],
      verdict: "noUseBreaks",
      dependencies: [external],
      brief: {
        issue: null,
        substitutions: [],
        contract: [entry("spec/media.spec.md", "spec", 14), entry("spec/a.spec.md", "spec"), entry("spec/b.spec.md", "spec"), entry("docs/x.md", "doc")],
      },
    })
    const view = presentMap(plan)
    expect(view.total).toEqual({ files: 5, additions: 10078, deletions: 207 })
    expect(view.rows.map(row => [row.label, row.badge, row.files, row.detail, row.extra, row.place?.path ?? null])).toEqual([
      ["view.gui.media", "New module", 61, "uses view.gui and shared/code · content of com.intellij.air", null, "gui/media/src/Player.kt"],
      ["view.gui", null, 47, "ui/embeds 29 · ui/markdown 11 · ui/util 2", null, "gui/src/Embed.kt"],
      ["view.gui.showcase", null, 16, null, "+ depends on intellij.platform.ide.util.io", "gui/showcase/src/Clip.kt"],
      ["shared/code", null, 2, null, "Opens wider: isNightly", "shared/src/Flag.kt"],
    ])
    // the bar of each row: its changed lines against the largest row's
    expect(view.rows.map(row => Math.round(row.share * 100))).toEqual([100, 54, 17, 1])
    expect(view.more).toBeNull()
    expect(view.contract).toEqual({
      newSpecs: [{ key: "spec/media.spec.md", name: "Media", rules: 14, place: { path: "spec/media.spec.md", side: "new", line: null, name: null } }],
      changed: "2 specs and 1 doc changed",
    })
    expect(view.api).toEqual({
      widened: [{ key: "s.isNightly:shared/src/Flag.kt", name: "isNightly", place: { path: "shared/src/Flag.kt", side: "new", line: 3, name: "isNightly" } }],
      verdict: "No use left outside breaks",
    })
  })

  it("names the rows past its budget once, and the folders of a large change of one module; a small change has no shape", () => {
    const many = Array.from({ length: MAP_ROWS + 2 }, (_, index) =>
      moduleShape(`m${index}`, `module-${index}`, { files: { code: 2, tests: 0, resources: 0, build: 0, prose: 0 }, additions: 10 - index }),
    )
    const view = presentMap(reviewPlan({ modules: many }))
    expect([view.rows.length, view.more]).toEqual([MAP_ROWS, { count: 2, labels: "module-6, module-7" }])

    const one = moduleShape("gui", "gui", {
      files: { code: 20, tests: 5, resources: 0, build: 0, prose: 0 },
      directories: [
        { path: "ui/embeds", files: 14 },
        { path: "ui/markdown", files: 11 },
      ],
    })
    const folders = presentMap(reviewPlan({ modules: [one], order: ["gui/src/ui/embeds/A.kt", "gui/src/ui/markdown/B.kt"] }))
    expect(folders.rows.map(row => [row.label, row.files, row.place?.path ?? null])).toEqual([
      ["ui/embeds", 14, "gui/src/ui/embeds/A.kt"],
      ["ui/markdown", 11, "gui/src/ui/markdown/B.kt"],
    ])

    const small = presentMap(reviewPlan({ modules: [moduleShape("gui", "gui", { files: { code: 3, tests: 1, resources: 0, build: 0, prose: 0 } })] }))
    expect([small.rows, small.contract, small.api]).toEqual([[], null, null])
  })

  it("names a spec by its file", () => {
    expect([specName("plugins/air/spec/session-view/transcript-media-playback.spec.md"), specName("docs/guide_notes.md")]).toEqual(["Transcript Media Playback", "Guide Notes"])
  })
})
