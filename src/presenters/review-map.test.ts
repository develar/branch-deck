import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, ContractEntry, DependencyChange, Registration, UiText } from "@/lib/bindings"
import { MAP_ROWS, presentMap, specName } from "@/presenters/review-map"
import { fileFacts as file, moduleShape, reviewPlan, reviewStep } from "@/test-support/review-step"

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
    // the row of shared/code names what it opens wider; the API line the verdict once, the changes it covers under it
    expect([view.api?.widened, view.api?.verdict?.label, view.api?.verdict?.entries.map(entry => [entry.kind, entry.name])]).toEqual([
      [],
      "No use left outside breaks · 1 API change",
      [["Opens wider", "isNightly"]],
    ])
  })

  it("names the rows past its budget once, and the folders of a large change of one module; a small change has no shape", () => {
    const many = Array.from({ length: MAP_ROWS + 2 }, (_, index) =>
      moduleShape(`m${index}`, `module-${index}`, { files: { code: 2, tests: 0, resources: 0, build: 0, prose: 0 }, additions: 10 - index }),
    )
    // a module past the budget has no row to name what it opens wider: the API line names it, at its declaration; no
    // verdict without Rust's
    const view = presentMap(reviewPlan({ modules: many, files: [file("m7/src/Flag.kt", 7)], api: [{ ...widened, path: "m7/src/Flag.kt" }], verdict: "pending" }))
    expect([view.rows.length, view.more]).toEqual([MAP_ROWS, { count: 2, labels: "module-6, module-7" }])
    expect(view.api).toEqual({
      widened: [{ key: "s.isNightly:m7/src/Flag.kt", name: "isNightly", place: { path: "m7/src/Flag.kt", side: "new", line: 3, name: "isNightly" } }],
      verdict: null,
    })

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

  it("shows the first file of a row as the story shows it: of its first step, a file of no step after them", () => {
    const plan = reviewPlan({
      modules: [
        moduleShape("media", "media", { state: "added", files: { code: 2, tests: 0, resources: 1, build: 1, prose: 0 } }),
        moduleShape("gui", "gui", { files: { code: 2, tests: 0, resources: 0, build: 0, prose: 0 } }),
      ],
      files: [file("media/resources/media.xml", 0), file("media/src/Player.kt", 0), file("gui/src/Embed.kt", 1), file("gui/src/Views.kt", 1), file("media/BUILD.bazel", 0)],
      // the dependency order: the descriptor and the build file first
      order: ["media/BUILD.bazel", "media/resources/media.xml", "gui/src/Views.kt", "media/src/Player.kt", "gui/src/Embed.kt"],
      steps: {
        ...reviewPlan().steps,
        steps: [
          reviewStep("a1", [], [["Embed", "gui/src/Embed.kt"]]),
          reviewStep(
            "b2",
            [],
            [
              ["Player", "media/src/Player.kt"],
              ["Views", "gui/src/Views.kt"],
              ["media.xml", "media/resources/media.xml"],
            ],
          ),
        ],
      },
    })
    expect(presentMap(plan).rows.map(row => [row.label, row.place?.path ?? null])).toEqual([
      ["media", "media/src/Player.kt"],
      ["gui", "gui/src/Embed.kt"],
    ])
  })

  it("names a spec by its file", () => {
    expect([specName("plugins/air/spec/session-view/transcript-media-playback.spec.md"), specName("docs/guide_notes.md")]).toEqual(["Transcript Media Playback", "Guide Notes"])
  })

  it("names under the row of a module what its descriptors register by simple names, a click at the class it registers; a content module none, past the budget a count", () => {
    const registration = (registered: Registration["registered"], more: Partial<Registration> = {}): Registration => ({
      descriptor: "gui/media/resources/media.xml",
      module: 0,
      change: "added",
      registered,
      before: null,
      changed: [],
      classes: [],
      ...more,
    })
    const service = registration(
      {
        kind: "service",
        area: "app",
        interface: "a.gui.Players",
        implementation: "a.media.PlayersImpl",
        testImplementation: null,
        headlessImplementation: null,
        client: null,
        os: null,
        preload: null,
        overrides: false,
        open: false,
      },
      { classes: [{ role: "implementation", name: "a.media.PlayersImpl", found: { resolved: "declared", path: "gui/media/src/PlayersImpl.kt", line: 4 } }] },
    )
    const key = registration(
      {
        kind: "registryKey",
        key: "media.inline",
        defaultValue: "NIGHTLY_ENABLED",
        options: ["ENABLED", "NIGHTLY_ENABLED", "DISABLED"],
        description: "Plays inline",
        restartRequired: false,
      },
      { change: "changed", changed: ["defaultValue"] },
    )
    const content = registration({ kind: "contentModule", name: "intellij.gui.media", loading: "optional", reviewModule: 0 })
    const actions = Array.from({ length: 4 }, (_, index) => registration({ kind: "action", id: `Media.Action${index}`, class: null, text: null, group: false }, { module: 1 }))
    const view = presentMap(
      reviewPlan({
        modules: [
          moduleShape("gui/media", "view.gui.media", { state: "added", files: { code: 3, tests: 0, resources: 1, build: 1, prose: 0 } }),
          moduleShape("gui", "view.gui", { files: { code: 2, tests: 0, resources: 1, build: 0, prose: 0 } }),
        ],
        registrations: [service, key, content, ...actions],
      }),
    )
    expect(view.rows.map(row => [row.label, row.registrations.map(it => `${it.sign} ${it.kind} ${it.name}`), row.moreRegistrations])).toEqual([
      ["view.gui.media", ["+ applicationService Players → PlayersImpl", "~ registryKey media.inline = NIGHTLY_ENABLED (defaultValue changed)"], 0],
      ["view.gui", ["+ action Media.Action0", "+ action Media.Action1", "+ action Media.Action2"], 1],
    ])
    expect(view.rows[0]!.registrations[0]!.place).toEqual({ path: "gui/media/src/PlayersImpl.kt", side: "new", line: 4, name: null })
    expect(view.rows[0]!.registrations[1]!.title).toBe("Plays inline\ngui/media/resources/media.xml")
  })

  it("counts the texts of the UI a change adds, changes and removes, each its words at its entry, a removed one at its old line; no bundle, no line", () => {
    const text = (key: string, change: UiText["change"], before: string | null, after: string | null, line: number): UiText => ({
      path: "gui/resources/messages/PlayerBundle.properties",
      key,
      change,
      before,
      after,
      line,
      module: 0,
    })
    const view = presentMap(
      reviewPlan({
        texts: [
          text("media.play", "added", null, "Play", 4),
          text("media.download", "added", null, "Download…", 5),
          text("media.cannot", "changed", "Cannot play", "The chat cannot play this file", 6),
          text("media.open", "removed", "Open", null, 2),
        ],
      }),
    )
    expect(view.texts?.counts).toBe("2 new, 1 changed and 1 removed")
    expect(view.texts?.texts.map(it => [it.name, it.change, it.words, it.before, it.place.side, it.place.line])).toEqual([
      ["media.play", "added", "Play", null, "new", 4],
      ["media.download", "added", "Download…", null, "new", 5],
      ["media.cannot", "changed", "The chat cannot play this file", "Cannot play", "new", 6],
      ["media.open", "removed", "Open", null, "old", 2],
    ])
    expect(view.texts?.texts[0]?.title).toBe("media.play\ngui/resources/messages/PlayerBundle.properties:4")
    expect(presentMap(reviewPlan()).texts).toBeNull()
  })
})
