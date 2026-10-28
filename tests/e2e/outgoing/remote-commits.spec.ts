// @spec spec/repository/outgoing.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton, getPushButton } from "../helpers/branch-helpers"
import { availableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest } from "../helpers/pull-request-helpers"
import { runAndWaitForSync, syncFromMenu } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast } from "../helpers/toast-helpers"
import { recordRewrites } from "../helpers/wait-helpers"

/** Three hours before the fixed clock of the page */
const THEIR_TIME = Date.parse("2024-01-02T07:00:00Z") / 1000

/** Lev pushes a commit onto the remote branch of `test-branch` and its pull request #800 shows it, then a sync lists it */
async function levPushes(page: Page, repo: TestRepositoryBuilder): Promise<string> {
  // the branch as it is here reaches the remote, then Lev pushes a commit onto it, which this clone has not fetched
  await repo.pushInTerminal("user-name/virtual/test-branch")
  const theirs = await repo.commitOnRemote("user-name/virtual/test-branch", {
    subject: "Change fix.txt",
    author: "Lev Leontev <lev@example.com>",
    timestamp: THEIR_TIME,
    files: { "fix.txt": "fix\n" },
  })
  await repo.setPullRequests(availablePullRequests(pullRequest("test-branch", 800, { headRefOid: theirs })))
  // a sync reads the remote branch as pushed; its listing of the pull requests shows the head the remote moved to
  await repo.advanceBackendClock(60_000)
  await syncFromMenu(page)
  return theirs
}

/** The `simple` template with the pull request #800 of `test-branch` (`options` more fixtures), synced, then Lev's push */
async function branchLevPushedTo(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  options: object = {},
) {
  const repo = await setupRepo("simple", { pullRequests: availablePullRequests(pullRequest("test-branch", 800)), ...options })
  await syncAndWaitForBranches()
  await levPushes(page, repo)
  return { repo, row: findBranchRow(page, "test-branch") }
}

function newsToast(page: Page) {
  return page.locator("[data-slot=toast]", { hasText: "Lev Leontev pushed 1 commit to test-branch" })
}

test.describe("Remote commits of others", () => {
  test("should fetch the branch of a pull request someone pushed to and name their commit before any push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { repo, row } = await branchLevPushedTo(page, setupRepo, syncAndWaitForBranches)

    await expect(newsToast(page)).toContainText("Your branch doesn't have it: a push drops it.")
    await expect(row).toContainText("1↓")
    // the push the row offers drops their commit, so it asks first
    await expect(getPushButton(row)).toHaveAccessibleName("Force Push Branch")
    await getPushButton(row).hover()
    await expect(page.getByTestId("push-description")).toContainText("The remote has 1 commit your branch doesn't (by Lev Leontev).")

    // the same head again fetches nothing more and tells nothing more
    await repo.advanceBackendClock(60_000)
    await syncFromMenu(page)
    await expect(newsToast(page)).toHaveCount(1)
  })

  test("should pick the commit someone pushed from the toast, after which the push drops nothing, and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { row } = await branchLevPushedTo(page, setupRepo, syncAndWaitForBranches)

    // the toast opens the question of the push button, which lists the commit with the pick beside the force push
    await newsToast(page).getByRole("button", { name: "Pick Remote Commits…" }).click()
    const question = page.getByTestId("push-description")
    await expect(question).toContainText("Lev Leontev")
    await expect(question.getByTestId("push-confirm")).toBeVisible()
    const picks = await recordRewrites(page, "pickRemote")
    await runAndWaitForSync(page, () => question.getByRole("button", { name: "Pick Remote Commits" }).click())

    const picked = page.locator("[data-slot=toast]", { hasText: "Picked the remote commits of test-branch" })
    await expect(picked).toContainText("1 commit into test-branch · its push drops nothing now")
    await expect(newsToast(page)).toHaveCount(0)
    await expect(row).not.toContainText("1↓")
    await expect(row).toContainText("2")

    // one rewrite of the engine, whose undo is the rewrite `undo` of its `pick:` token
    expect(picks.map(call => call.rewrite.branchName)).toEqual(["test-branch"])
    const undos = await recordRewrites(page, "undo")
    // the undo takes the commit out again: the remote has it, the branch lacks it
    await runAndWaitForSync(page, () => picked.getByRole("button", { name: "Undo" }).click())
    await expect(row).toContainText("1↓")
    expect(undos.map(call => (call.rewrite.token as { kind: string }).kind)).toEqual(["pick"])
  })

  test("should pick the commit someone pushed from the branch menu, through the question of the push button", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { row } = await branchLevPushedTo(page, setupRepo, syncAndWaitForBranches)
    await closeToast(newsToast(page))

    await getMoreButton(row).click()
    await page.getByRole("menuitem", { name: "Pick Remote Commits…" }).click()
    const question = page.getByTestId("push-description")
    await expect(question).toContainText("Lev Leontev")
    await runAndWaitForSync(page, () => question.getByRole("button", { name: "Pick Remote Commits" }).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Picked the remote commits of test-branch" })).toBeVisible()
    await expect(row).not.toContainText("1↓")
  })

  test("should make the pick the next step of a branch on Space, before its Safe Push, and run it from the slot", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { row } = await branchLevPushedTo(page, setupRepo, syncAndWaitForBranches, { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" } })
    await closeToast(newsToast(page))

    const slot = row.getByTestId("next-step-slot")
    await expect(slot).toHaveAttribute("data-step", "pickRemoteCommits")
    await slot.getByRole("button", { name: "Pick Remote Commits" }).click()
    const question = page.getByTestId("push-description")
    await expect(question).toContainText("Lev Leontev")
    await runAndWaitForSync(page, () => question.getByRole("button", { name: "Pick Remote Commits" }).click())
    await expect(slot).not.toHaveAttribute("data-step", "pickRemoteCommits")
  })
})
