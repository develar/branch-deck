// @spec openspec/specs/branch/review-tour/spec.md
import { writeFileSync } from "node:fs"
import path from "node:path"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { virtualBranch } from "../helpers/push-helpers"
import { openPlanWindow, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the `review_modules` template: the branch `media` of a plugin `player`, its second commit a large step that adds the
// module `view.gui.media`, cut into Contract, `view.gui`, `view.gui.media` (also `shared/code`) and its wiring

const ROOT = "plugins/player"
const GUI = `${ROOT}/view.gui/src/com/example/player/gui`
const MEDIA = `${ROOT}/view.gui.media/src/com/example/player/media`
const MEDIA_TESTS = `${ROOT}/view.gui.media/testSrc/com/example/player/media`
const DESCRIPTOR = `${ROOT}/view.gui.media/resources/intellij.player.view.gui.media.xml`
const DECODER = `${MEDIA}/avfoundation/AvFoundationDecoder.kt`
/** The registry key of inline playback, an extension of the descriptor of the new module */
const REGISTRY_KEY = {
  point: "com.intellij.registryKey",
  implementation: null,
  element: { name: "", attributes: { key: "player.media.inline.playback", defaultValue: "true", description: "Play the media embeds of the chat inline" }, children: [] },
}

/**
 * accio's answer for the descriptor the change adds, at the head the review reads (the copy the sync builds of the
 * branch): its application service, the `extensions` given, its class resolved there; the base has none. With
 * `surface`, its answer of what enters the new module there
 */
async function answerDescriptor(repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<unknown>, extensions: readonly unknown[], surface?: unknown): Promise<void> {
  const line = {
    path: DESCRIPTOR,
    root: "idea-plugin",
    services: [{ area: "app", interface: "com.example.player.media.MediaDecoder", implementation: "com.example.player.media.avfoundation.AvFoundationDecoder", preload: "FALSE" }],
    extensions,
    classes: [{ role: "services.implementation", name: "com.example.player.media.avfoundation.AvFoundationDecoder", resolved: "exact", path: DECODER, line: 3 }],
  }
  await syncAndWaitForBranches()
  const head = await repo.revision(virtualBranch("media"))
  const surfaces = surface === undefined ? {} : { [`${head}:${ROOT}/view.gui.media`]: surface }
  writeFileSync(path.join(repo.path, ".git", "branch-deck-accio.json"), JSON.stringify({ descriptors: { [`${head}:${DESCRIPTOR}`]: line }, surface: surfaces }))
}

test.describe("Review groups", () => {
  test(
    "should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("review_modules")
      turnOnReviewPlan(repo)
      const window = await openPlanWindow(page, syncAndWaitForBranches, "media")
      const { tree } = reviewSidebar(window)
      const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => `${item.getAttribute("aria-level")} ${item.getAttribute("aria-label")}`))
      const row = (name: string) => tree.getByRole("treeitem", { name, exact: true })

      // the small step a list as before; the large one its groups, each a heading of its files at their indent
      await expect
        .poll(rows)
        .toEqual([
          "1 1 · the embed names the extension of its file",
          `2 ${GUI}/embeds/EmbedSource.kt`,
          "1 2 · play the media embeds of the chat inline",
          "2 Contract",
          `3 ${ROOT}/spec/transcript-embeds.spec.md`,
          `3 ${ROOT}/AGENTS.md`,
          "2 view.gui",
          `3 ${ROOT}/view.gui/resources/messages/PlayerBundle.properties`,
          `3 ${GUI}/embeds/EmbedKind.kt`,
          `3 ${GUI}/embeds/EmbedModel.kt`,
          `3 ${GUI}/markdown/MarkdownEmbeds.kt`,
          `3 ${GUI}/embeds/EmbedView.kt`,
          `3 ${GUI}/embeds/EmbedViews.kt`,
          "3 Tests",
          `4 ${ROOT}/view.gui/testSrc/com/example/player/gui/embeds/EmbedModelTest.kt`,
          "2 view.gui.media",
          `3 ${ROOT}/shared/code/src/com/example/player/shared/NightlyBuild.kt`,
          `3 ${MEDIA}/MediaPlaybackFlag.kt`,
          `3 ${MEDIA}/MediaEmbeds.kt`,
          `3 ${ROOT}/view.gui.media/resources/intellij.player.view.gui.media.xml`,
          `3 ${MEDIA}/avfoundation/AvFoundationBridge.kt`,
          `3 ${MEDIA}/MediaTimeline.kt`,
          `3 ${MEDIA}/MediaDecoder.kt`,
          `3 ${MEDIA}/avfoundation/AvFoundationDecoder.kt`,
          `3 ${MEDIA}/MediaPlayer.kt`,
          `3 ${MEDIA}/MediaControls.kt`,
          `3 ${MEDIA}/MediaEmbedView.kt`,
          "3 Tests",
          `4 ${MEDIA_TESTS}/MediaTimelineTest.kt`,
          `4 ${MEDIA_TESTS}/MediaPlayerTest.kt`,
          "2 Wiring",
        ])
      const media = row("view.gui.media")
      // the new module says so, names the small module whose file it holds, its root in its hover; its build files are
      // the wiring, so their new module is no mark of their step
      await expect(media.getByTestId("review-group-state")).toHaveText("New module")
      await expect(media.getByTestId("review-group-also")).toHaveText("also shared/code")
      await expect(media).toHaveAttribute("aria-description", "New module, also shared/code, 13 files, +146 −2")
      await expect(media.getByTestId("review-group-name")).toHaveAttribute("title", `${ROOT}/view.gui.media`)
      await expect(row("2 · play the media embeds of the chat inline")).toHaveAttribute("aria-description", /^27 files, \+212 −4, commit [0-9a-f]+, \+1 dep$/)
      // a file by its name, then its directory below what the files of its module share there, a file of the small module
      // by that module; a file of the contract by its whole directory
      const directory = (path: string) => row(path).getByTestId("file-label-directory")
      await expect(directory(`${GUI}/embeds/EmbedKind.kt`)).toHaveText("embeds")
      await expect(directory(`${GUI}/markdown/MarkdownEmbeds.kt`)).toHaveText("markdown")
      await expect(directory(`${MEDIA}/avfoundation/AvFoundationDecoder.kt`)).toHaveText("avfoundation")
      await expect(directory(`${ROOT}/shared/code/src/com/example/player/shared/NightlyBuild.kt`)).toHaveText("shared/code")
      await expect(directory(`${ROOT}/AGENTS.md`)).toHaveText(ROOT)
      await expect(row(`${MEDIA}/MediaPlayer.kt`)).toBeVisible()
      await expect(directory(`${MEDIA}/MediaPlayer.kt`)).toHaveCount(0)
      await captureHtmlSnapshot(tree, "review-groups-tree")

      // a click on the heading of a group folds it and opens it again
      const kind = row(`${GUI}/embeds/EmbedKind.kt`)
      await row("view.gui").click()
      await expect(row("view.gui")).toHaveAttribute("aria-expanded", "false")
      await expect(kind).toHaveCount(0)
      await row("view.gui").click()
      await expect(row("view.gui")).toHaveAttribute("aria-expanded", "true")
      await expect(kind).toBeVisible()
      // ← from a file goes to the heading of its group, ← folds it, → opens it; Space and the chevron fold it as well
      await kind.click()
      // the line of the selected step runs from under its row, never across its chevron
      const line = tree.getByTestId("tree-step-line")
      await expect(line).toBeVisible()
      const stepBox = await row("2 · play the media embeds of the chat inline").boundingBox()
      const lineBox = await line.boundingBox()
      expect(lineBox!.y).toBeGreaterThanOrEqual(stepBox!.y + stepBox!.height - 1)
      await window.keyboard.press("ArrowLeft")
      await expect(row("view.gui")).toBeFocused()
      await window.keyboard.press("ArrowLeft")
      await expect(row("view.gui")).toHaveAttribute("aria-expanded", "false")
      await window.keyboard.press("ArrowRight")
      await expect(kind).toBeVisible()
      await window.keyboard.press(" ")
      await expect(kind).toHaveCount(0)
      await row("view.gui").getByTestId("tree-row-chevron").click()
      await expect(kind).toBeVisible()
      // the wiring opens by its chevron to its build files and its list of modules
      await row("Wiring").getByTestId("tree-row-chevron").click()
      await expect(row(`${ROOT}/view.gui.media/BUILD.bazel`)).toBeVisible()
      await expect(row("build/module-list.txt")).toBeVisible()

      // J walks the files across the groups: the last file of view.gui, its test, then the first file of the new module
      await row(`${GUI}/embeds/EmbedViews.kt`).click()
      await window.keyboard.press("j")
      await expect(row(`${ROOT}/view.gui/testSrc/com/example/player/gui/embeds/EmbedModelTest.kt`)).toHaveAttribute("aria-selected", "true")
      await window.keyboard.press("j")
      await expect(row(`${ROOT}/shared/code/src/com/example/player/shared/NightlyBuild.kt`)).toHaveAttribute("aria-selected", "true")
    },
  )

  test("should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs, and say in the bar above the diffs where the reader is", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_modules")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "media")
    const { tree } = reviewSidebar(window)
    const row = (name: string) => tree.getByRole("treeitem", { name, exact: true })
    const media = row("view.gui.media")
    const wiring = row("Wiring")
    const step = row("2 · play the media embeds of the chat inline")

    // the review opens on its overview: nothing seen yet, no count on the groups
    await expect(media).toHaveAttribute("aria-description", "New module, also shared/code, 13 files, +146 −2")
    await expect(tree.getByTestId("review-seen-progress")).toHaveCount(0)

    // a file of the new module shown from its row: seen, its group and its step count it
    await row(`${MEDIA}/MediaTimeline.kt`).click()
    await expect(media.getByTestId("review-seen-progress")).toHaveText(/^\d+ \/ 13 seen$/)
    await expect(step.getByTestId("review-seen-progress")).toHaveText(/^\d+ \/ 27 seen$/)
    await expect(media).toHaveAttribute("aria-description", /^New module, also shared\/code, 13 files, \+146 −2, \d+ \/ 13 seen$/)
    await expect(wiring.getByTestId("review-seen-progress")).toHaveCount(0)
    // the bar above the diffs says where the reader is: the step, the group of the section at the top and its part seen
    await expect(window.getByTestId("review-step-group")).toHaveText(/^›view\.gui\.media·\d+ \/ 13 seen$/)

    // the diffs scrolled to their end: the cards of the wiring passed through the view, seen, no row of them clicked
    await window.getByTestId("diff-view").hover()
    await window.mouse.wheel(0, 100_000)
    await expect(wiring.getByTestId("review-seen-progress")).toHaveText(/^[1-5] \/ 5 seen$/)
  })

  test("should name under the row of a module of the map what its descriptor registers, a click showing the class it registers", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_modules")
    turnOnReviewPlan(repo)
    await answerDescriptor(repo, syncAndWaitForBranches, [REGISTRY_KEY])
    const window = await openPlanWindow(page, syncAndWaitForBranches, "media")

    const media = window.getByTestId("review-map-module").filter({ has: window.getByTestId("review-map-label").getByText("view.gui.media", { exact: true }) })
    const registrations = media.getByTestId("review-map-registration")
    await expect(registrations).toHaveText(["+applicationServiceMediaDecoder → AvFoundationDecoder", "+registryKeyplayer.media.inline.playback = true"])
    await expect(registrations.nth(1)).toHaveAttribute("title", `Play the media embeds of the chat inline\n${DESCRIPTOR}`)
    await registrations.first().click()
    await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: DECODER, exact: true })).toHaveAttribute("aria-selected", "true")
  })

  test("should quote on a line of the map the texts of the UI the change adds, all of them under it on a click, a quote showing its entry in its bundle", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_modules")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "media")
    const bundle = `${ROOT}/view.gui/resources/messages/PlayerBundle.properties`

    // the counts and the first two quoted, the rest folded under "and 2 more"
    // the map's line (the opening of the group of view.gui has its own)
    const map = window.getByTestId("review-map")
    const line = map.getByTestId("review-map-texts")
    await expect(line).toContainText("4 new:")
    await expect(line.getByTestId("review-map-text-quote")).toHaveText(["“Video: {0}”", "“Sound: {0}”"])
    const texts = map.getByTestId("review-map-text")
    // folded until found: in the page for ⌘F, out of sight
    await expect(texts.first()).toBeHidden()
    await line.getByTestId("review-map-texts-more").click()
    // each its sign of an addition, its words and its key
    await expect(texts).toHaveText([
      "+“Video: {0}”embed.video.accessible.name",
      "+“Sound: {0}”embed.audio.accessible.name",
      "+“The chat cannot play this file”embed.media.cannot.play",
      "+“Download…”embed.media.download",
    ])
    await expect(texts.nth(2).getByTestId("review-map-text-quote")).toHaveAttribute("title", `embed.media.cannot.play\n${bundle}:6`)

    // a quote shows the bundle, its file selected in the tree
    await texts.nth(3).getByTestId("review-map-text-quote").click()
    await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: bundle, exact: true })).toHaveAttribute("aria-selected", "true")
  })

  test("should open each group of a module above its first card, what it is, its new API, its new uses between modules, what it registers and its texts of the UI, and leave them out of the opening of the step", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_modules")
    turnOnReviewPlan(repo)
    await answerDescriptor(repo, syncAndWaitForBranches, [])
    const window = await openPlanWindow(page, syncAndWaitForBranches, "media")

    // the two groups of modules of step 2 open, each above its first card; the contract and the wiring do not
    const openings = window.getByTestId("review-group-opening")
    await expect(openings.getByTestId("review-group-opening-name")).toHaveText(["view.gui", "view.gui.media"])
    const gui = openings.filter({ has: window.getByTestId("review-group-opening-name").getByText("view.gui", { exact: true }) })
    const media = openings.filter({ has: window.getByTestId("review-group-opening-name").getByText("view.gui.media", { exact: true }) })
    await expect(media.getByTestId("review-group-opening-state")).toHaveText("New module")
    await expect(media).toContainText("also shared/code")
    // what each group does to the rest: its new API by name, the uses between modules by their direction, what it
    // registers, its texts of the UI
    await expect(gui.getByTestId("review-step-new-api")).toHaveText(["VIDEO", "AUDIO", "playable", "supports"])
    await expect(gui.getByTestId("review-step-module-user")).toHaveText(/^view\.gui\.media \(\d+ links\)$/)
    await expect(gui.getByTestId("review-group-opening-texts")).toContainText("4 new:")
    await expect(media.getByTestId("review-step-module-use").first()).toHaveText(/^view\.gui( \(\d+ links\))?$/)
    await expect(media.getByTestId("review-step-opens-wider")).toHaveText("isNightlyBuild")
    await expect(media.getByTestId("review-map-registration")).toHaveText(["+applicationServiceMediaDecoder → AvFoundationDecoder"])
    // the opening of the step keeps its story and its contract, no new API of its own
    const opening = window.getByTestId("review-opening").filter({ hasText: "play the media embeds of the chat inline" })
    await expect(opening.getByTestId("review-brief-contract-entry").first()).toBeAttached()
    await expect(opening.getByTestId("review-step-new-api")).toHaveCount(0)
    await captureHtmlSnapshot(media, "review-group-opening")

    // a name of a group shows its declaration, its file selected in the tree
    await media.getByTestId("review-map-registration").click()
    await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: DECODER, exact: true })).toHaveAttribute("aria-selected", "true")
  })

  test("should name what enters a new module in the opening of its group, read the group from the file of its entry point, and show its declaration on a click", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_modules")
    turnOnReviewPlan(repo)
    // accio: the class the descriptor of the new module registers enters it
    const registration = {
      file: DESCRIPTOR,
      line: 7,
      label: "descriptor",
      point: "com.intellij.applicationService",
      of: "com.example.player.media.MediaDecoder",
      resolved: "typed",
      test: false,
    }
    await answerDescriptor(repo, syncAndWaitForBranches, [], {
      module: `${ROOT}/view.gui.media`,
      complete: true,
      declarations: [
        {
          file: DECODER,
          line: 8,
          kind: "class",
          name: "AvFoundationDecoder",
          qualified: "com.example.player.media.avfoundation.AvFoundationDecoder",
          used_by: [{ file: DESCRIPTOR, line: 7 }],
          uses: [registration],
        },
      ],
    })
    const window = await openPlanWindow(page, syncAndWaitForBranches, "media")

    const media = window.getByTestId("review-group-opening").filter({ has: window.getByTestId("review-group-opening-name").getByText("view.gui.media", { exact: true }) })
    const entry = media.getByTestId("review-step-entry")
    await expect(entry).toHaveText("AvFoundationDecoder (applicationService of MediaDecoder)")
    // the group of the new module reads from the file of its entry point
    const { tree } = reviewSidebar(window)
    const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => item.getAttribute("aria-label")))
    await expect
      .poll(async () => {
        const all = await rows()
        return all[all.indexOf("view.gui.media") + 1]
      })
      .toBe(DECODER)
    await entry.click()
    await expect(tree.getByRole("treeitem", { name: DECODER, exact: true })).toHaveAttribute("aria-selected", "true")
  })
})
