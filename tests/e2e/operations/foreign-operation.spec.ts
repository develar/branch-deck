// @spec spec/branch/patronus.spec.md
import type { Locator, Page } from "@playwright/test"
import type { Intent } from "../../../src/lib/bindings"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, pushFromRowMenu } from "../helpers/branch-helpers"
import { setupRobots, startRequest } from "../helpers/patronus-helpers"
import { GITHUB_REPOSITORY, getOpenedUrls } from "../helpers/pull-request-helpers"
import { expectPushedToRemote, pushedBranches } from "../helpers/push-helpers"
import { safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { runAndWaitForAnswer, runAndWaitForSync } from "../helpers/sync-helpers"

/** The Safe Push of `branch` that an agent's `bd safe-push` runs */
function safePushOf(branch: string): Intent {
  return { kind: "land", subject: { kind: "branch", name: branch }, mode: "rebaseAutosquash", profile: { kind: "default" } }
}

function toastWith(page: Page, text: string): Locator {
  return page.locator("[data-slot=toast]", { hasText: text })
}

test.describe("Operations of another process of the repository", () => {
  test("should show the Safe Push bd runs on its row, refuse a start and a push of it, and push another branch beside it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const agent = await repo.setForeignOperation(safePushOf("JIRA-456"))
    const slot = slotSafePush(page, "JIRA-456")
    // the window watches the operations registry of the repository: the row shows what bd runs, busy, named by its pid
    await expect(slot).toHaveText("bd: Safe Push of JIRA-456…")
    await expect(slot).toHaveAttribute("aria-busy", "true")
    await slot.hover()
    await expect(page.getByRole("tooltip")).toHaveText(`\`bd\` (pid ${agent!.pid}) runs the Safe Push of JIRA-456, which holds what it needs until it ends`)

    // S says why it can't start, and opens no popover
    await findBranchRow(page, "JIRA-456").focus()
    await page.keyboard.press("s")
    await expect(toastWith(page, "Safe Push of JIRA-456 can't start")).toContainText(`\`bd\` (pid ${agent!.pid}) runs the Safe Push of JIRA-456`)
    await expect(safePushPopover(page)).toHaveCount(0)

    // a push of the branch is refused in Rust by the operations registry, naming the holder; another branch pushes
    await runAndWaitForAnswer(page, "execute", () => pushFromRowMenu(page, "JIRA-456"))
    await expect(toastWith(page, "Cannot push JIRA-456")).toContainText(new RegExp(`^.*\`bd\` \\(pid ${agent!.pid}\\) has run the Safe Push of JIRA-456 for \\d+ s`))
    await runAndWaitForAnswer(page, "execute", () => pushFromRowMenu(page, "ABC-123"))
    await expectPushedToRemote(repo, ["ABC-123"])
    expect(await pushedBranches(repo)).toEqual(["ABC-123"])
    expect((await repo.getPatronusLog()).starts).toEqual([])
  })

  test("should start the Safe Push of a branch once the operation of bd that held it ended", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await repo.setForeignOperation(safePushOf("JIRA-456"))
    const slot = slotSafePush(page, "JIRA-456")
    await expect(slot).toHaveText("bd: Safe Push of JIRA-456…")

    // its record left the registry: the row offers the start again, which pushes and starts once
    await repo.setForeignOperation(null)
    await expect(slot).not.toHaveAttribute("aria-busy", "true")
    await expect(slot).toHaveText(/^Safe Push/)
    await runAndWaitForAnswer(page, "execute", () => slot.click())
    await expect(findBranchRow(page, "JIRA-456").getByTestId("patronus-chip")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456")])
    await expectPushedToRemote(repo, ["JIRA-456"])
  })

  test("should show a start whose answer was lost on its row, refuse a start of it, and start again once it is forgotten", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await repo.setForeignOperation(safePushOf("JIRA-456"), undefined, true)
    const row = findBranchRow(page, "JIRA-456")
    const slot = slotSafePush(page, "JIRA-456")
    // no process runs it any more, and Patronus can't tell yet whether its robot started: it holds the branch
    await expect(slot).toHaveText("Start Unknown")
    // Open in Patronus opens where its robot would be
    expect(await getOpenedUrls(page)).toEqual([])
    await row.getByTestId("open-unknown-start").click()
    await expect.poll(() => getOpenedUrls(page)).toEqual(["https://patronus.test/"])
    await slot.hover()
    await expect(page.getByRole("tooltip")).toHaveText(
      "Cannot tell whether the Safe Push of JIRA-456 started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start; Patronus may still run it",
    )

    // S says why it can't start, and opens no popover
    await row.focus()
    await page.keyboard.press("s")
    await expect(toastWith(page, "Safe Push of JIRA-456 can't start")).toContainText("Cannot tell whether the Safe Push of JIRA-456 started")
    await expect(safePushPopover(page)).toHaveCount(0)

    // Forget This Start lets it go: nothing is asked of Patronus, and the toast says it may still run it
    await runAndWaitForAnswer(page, "execute", () => row.getByTestId("forget-start").click())
    await expect(toastWith(page, "Forgot the start of JIRA-456")).toContainText("Patronus may still run it")
    expect((await repo.getPatronusLog()).starts).toEqual([])
    await expect(row.getByTestId("forget-start")).toHaveCount(0)
    await expect(slot).toHaveText(/^Safe Push/)
    await runAndWaitForAnswer(page, "execute", () => slot.click())
    await expect(row.getByTestId("patronus-chip")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456")])
  })

  test("should open the pull request of a comment start whose answer was lost", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const url = `https://github.com/${GITHUB_REPOSITORY.owner}/${GITHUB_REPOSITORY.name}/pull/7`
    await repo.setForeignOperation(safePushOf("JIRA-456"), undefined, true, { number: 7, url })
    const row = findBranchRow(page, "JIRA-456")
    await expect(slotSafePush(page, "JIRA-456")).toHaveText("Start Unknown")
    expect(await getOpenedUrls(page)).toEqual([])
    await row.getByRole("button", { name: "Open Pull Request #7" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([url])
  })

  test("should sync when bd touched the stamp of the repository after a mutation, without a focus", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await expect(findBranchRow(page, "JIRA-456")).toBeVisible()
    // an agent's bd adds a branch behind the app's back, and touches the stamp after its mutation
    await repo.commit("(agent-docs) Add the docs of the agent", { "agent-docs.md": "docs\n" }, 1_704_200_000)
    await expect(findBranchRow(page, "agent-docs")).toHaveCount(0)
    await runAndWaitForSync(page, () => repo.touchStamp())
    await expect(findBranchRow(page, "agent-docs")).toBeVisible()
  })
})
