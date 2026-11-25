// @spec openspec/specs/branch/landing-plan/spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, getRowWait } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, mergeRunningRobot, robot, startRequest } from "../helpers/patronus-helpers"
import { openSafePushWithKey, safePushPopover, startAnywayFromMore } from "../helpers/safe-push-helpers"
import { runAndWaitForAnswer, runAndWaitForUpdate } from "../helpers/sync-helpers"
import { holdToast } from "../helpers/toast-helpers"
import { recordCommand } from "../helpers/wait-helpers"

// The `landed_need` template: flow uses Codec, which a commit of codec adds; the Safe Push of codec pushed it to the
// upstream, not fetched yet. While that robot runs, the start of flow lands after it
const CODEC = robot("codec", "codec-robot")

function step(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("next-step-slot")
}

test.describe("Lands after", () => {
  test(
    "should say on the row that a branch lands after the running Safe Push of what it needs, its slot empty and Start Anyway in the popover's ⋯",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("landed_need", { patronus: availableRobots(CODEC) })
      await syncAndWaitForBranches()
      // a wait: the Status cell says it, the slot offers nothing
      await expect(step(page, "flow")).toHaveAttribute("data-step", "landsAfter")
      await expect(step(page, "flow").getByTestId("next-step")).toHaveCount(0)
      const wait = getRowWait(findBranchRow(page, "flow"))
      await expect(wait).toHaveText("Lands after codec")
      await expect(wait).toHaveAttribute("data-tone", "quiet")
      await wait.hover()
      await expect(page.getByTestId("step-wait-hover")).toContainText(
        "Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt: codec is landing in its Safe Push, which tests it without this start: a start beside it fails too.",
      )

      // the popover says it in one headline and starts only from its "⋯": a start beside that robot fails
      const popover = await openSafePushWithKey(page, "flow")
      const needs = popover.getByTestId("safe-push-landing-needs")
      await expect(needs).toHaveAttribute("data-waits", "true")
      await expect(needs.getByTestId("safe-push-landing-headline")).toHaveText("Lands after codec")
      await expect(popover.getByTestId("safe-push-start")).toHaveCount(0)
      const starts = await recordCommand(page, "execute")
      await startAnywayFromMore(page, popover, "flow")
      expect(starts).toEqual([
        expect.objectContaining({ intent: expect.objectContaining({ subject: { kind: "branch", name: "flow" }, allow: expect.objectContaining({ allowMissing: true }) }) }),
      ])
      await expectPatronusStarts(repo, [startRequest("flow")])
    },
  )

  test("should offer the start of the branch that waited in the toast of the merge once its Update Project landed it, and start it on the press", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("landed_need", { patronus: availableRobots(CODEC) })
    await syncAndWaitForBranches()
    await expect(getRowWait(findBranchRow(page, "flow"))).toHaveText("Lands after codec")
    await mergeRunningRobot(page, repo, CODEC)
    const toasts = page.locator("[data-slot=toast]")
    const merged = await holdToast(toasts.filter({ hasText: "codec merged into master" }))
    // nothing starts by itself
    expect((await repo.getPatronusLog()).starts).toEqual([])
    await runAndWaitForUpdate(page, () => merged.getByRole("button", { name: "Update Project" }).click())
    // the same toast says what landed and offers the start that waited for it
    await expect(merged.locator("[data-slot=toast-title]")).toContainText("codec landed in origin/master")
    await expect(toasts).toHaveCount(1)
    await runAndWaitForAnswer(page, "execute", () => merged.getByRole("button", { name: "Start flow" }).click())
    await expectPatronusStarts(repo, [startRequest("flow")])
  })

  test(
    "should say a need nothing lands in one headline and a short detail, focus its fix before Start Anyway and keep the popover narrow",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("landed_need", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await expect(step(page, "flow")).toHaveAttribute("data-step", "needsBranch")
      await expect(step(page, "flow").getByTestId("next-step")).toHaveText("Needs codec")

      const popover = await openSafePushWithKey(page, "flow")
      const needs = popover.getByTestId("safe-push-landing-needs")
      await expect(needs.getByTestId("safe-push-landing-headline")).toHaveText("Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt")
      // short: the file by its name and line, its whole path in the hover; the commits by their hashes
      const detail = needs.getByTestId("safe-push-landing-detail")
      await expect(detail).toHaveText(/^[0-9a-f]{8} of flow uses Codec \(FlowCodecIntegrationEntryPoint\.kt:3\), which [0-9a-f]{8} of codec adds\.$/)
      await expect(detail.getByTitle("flow/src/main/kotlin/org/example/platform/landing/pipeline/codec/integration/FlowCodecIntegrationEntryPoint.kt:3")).toHaveText(
        "FlowCodecIntegrationEntryPoint.kt:3",
      )
      // the actions of the edge first, Land Together focused: Start Anyway stays, never the focused default
      await expect(needs.getByRole("button", { name: "Land Together" })).toBeFocused()
      await expect(needs.getByRole("button", { name: "Land codec First" })).toBeVisible()
      await expect(popover.getByTestId("safe-push-start")).toHaveText("Start Anyway")
      // nothing of the lines goes past the popover
      const box = (await safePushPopover(page).boundingBox())!
      for (const line of [detail, needs.getByTestId("safe-push-landing-headline")]) {
        const lineBox = (await line.boundingBox())!
        expect(lineBox.x + lineBox.width).toBeLessThanOrEqual(box.x + box.width)
        expect(await line.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
      }
    },
  )
})
