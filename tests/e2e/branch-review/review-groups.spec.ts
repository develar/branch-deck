// @spec spec/branch/review-tour.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { openPlanWindow, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"

// the `review_modules` template: the branch `media` of a plugin `player`, its second commit a large step that adds the
// module `view.gui.media`, cut into Contract, `view.gui`, `view.gui.media` (also `shared/code`) and its wiring

const ROOT = "plugins/player"
const GUI = `${ROOT}/view.gui/src/com/example/player/gui`
const MEDIA = `${ROOT}/view.gui.media/src/com/example/player/media`
const MEDIA_TESTS = `${ROOT}/view.gui.media/testSrc/com/example/player/media`

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

  test("should count the files of a group the reader saw: a file shown, a card scrolled entirely through the diffs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
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

    // the diffs scrolled to their end: the cards of the wiring passed through the view, seen, no row of them clicked
    await window.getByTestId("diff-view").hover()
    await window.mouse.wheel(0, 100_000)
    await expect(wiring.getByTestId("review-seen-progress")).toHaveText(/^[1-5] \/ 5 seen$/)
  })
})
