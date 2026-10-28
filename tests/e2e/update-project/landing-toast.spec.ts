// @spec spec/repository/landed-commits.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { availableRobots, mergeRunningRobot, robot } from "../helpers/patronus-helpers"
import { addRecentProject, OTHER_REPOSITORY, switchRepository } from "../helpers/repository-switch-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdToast } from "../helpers/toast-helpers"
import { holdIntent, recordCommand } from "../helpers/wait-helpers"

// The `landed_upstream` template (tests/e2e/update-project/landed.spec.ts): a robot pushed `feature-auth` and the first
// `search` commit to origin/master, already fetched. Here the robot is a Safe Push of feature-auth the app watched run.
const RUNNING = robot("feature-auth", "auth-robot")

function toasts(page: Page): Locator {
  return page.locator("[data-slot=toast]")
}

/**
 * The robot of feature-auth merges while the app watches it: the toast of the merge, held by its element, which the
 * later states of the landing replace in place
 */
async function mergeFeatureAuth(
  page: Page,
  setupRepo: (template: string, options?: { patronus?: ReturnType<typeof availableRobots> }) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  { otherProject = false }: { otherProject?: boolean } = {},
): Promise<{ repo: TestRepositoryBuilder; toast: Locator }> {
  const repo = await setupRepo("landed_upstream", { patronus: availableRobots(RUNNING) })
  if (otherProject) {
    // a second recent project to switch to: the toast of the merge then fetches the baseline of its own repository
    await addRecentProject(page)
  }
  await syncAndWaitForBranches()
  // the robot runs: the focus polls only the running robots the session listed (the fetched baseline has feature-auth
  // already, so its row shows the Landed chip, not the robot's)
  await expect(page.getByTestId("landing-indicator")).toHaveAccessibleName(/· 1 landing ·/)
  await mergeRunningRobot(page, repo, RUNNING)
  const merged = toasts(page).filter({ hasText: "feature-auth merged into master" })
  await expect(merged.getByRole("button", { name: "Update Project" })).toBeVisible()
  await expect(merged.getByRole("button", { name: "Open in Patronus" })).toBeVisible()
  await expect(toasts(page)).toHaveCount(1)
  return { repo, toast: await holdToast(merged) }
}

/** The landed state of the toast offers neither the update nor the robot of the merge it took the place of */
async function expectNoActions(toast: Locator): Promise<void> {
  await expect(toast.getByRole("button", { name: /^(Update Project|Updating…|Fetch|Open in Patronus)$/ })).toHaveCount(0)
}

/** The toast says what the update landed, in place of the merge, and offers no Update Project: it is done */
async function expectLandedInPlace(page: Page, toast: Locator): Promise<void> {
  await expect(toast.locator("[data-slot=toast-title]")).toHaveText("2 branches landed in origin/master")
  await expect(toast.locator("[data-slot=toast-description]")).toHaveText("feature-auth and search · rebased 2 commits")
  await expectNoActions(toast)
  await expect(toasts(page)).toHaveCount(1)
}

test.describe("The toast of a landing", () => {
  test("should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { toast } = await mergeFeatureAuth(page, setupRepo, syncAndWaitForBranches)
    const { release } = await holdIntent(page, "updateProject")
    await toast.getByRole("button", { name: "Update Project" }).click()
    // the update runs: the same toast stays and offers no second one
    await expect(toast.getByRole("button", { name: "Updating…" })).toBeDisabled()
    await expect(toast.locator("[data-slot=toast-title]")).toHaveText("feature-auth merged into master")
    release()
    await expectLandedInPlace(page, toast)
  })

  test("should turn the toast of a merge into what the update landed, in place, when the update starts from the header", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { toast } = await mergeFeatureAuth(page, setupRepo, syncAndWaitForBranches)
    const { release } = await holdIntent(page, "updateProject")
    await page.getByTestId("update-project-button").click()
    // a surface of the same update: its Update Project shows it running
    await expect(toast.getByRole("button", { name: "Updating…" })).toBeDisabled()
    release()
    await expectLandedInPlace(page, toast)
    // the header's update archived the branch the robot landed
    await expect(page.getByTestId("landed-card")).toBeVisible()
  })

  test("should go on from the Fetch of the toast with an update pressed during it: the update checks and moves on that fetch", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const { repo, toast } = await mergeFeatureAuth(page, setupRepo, syncAndWaitForBranches, { otherProject: true })
    // another repository is shown: the toast fetches the baseline of its own, while an agent's `bd update` fetches it
    await switchRepository(page, OTHER_REPOSITORY)
    await repo.holdFetchLock(true)
    const executions = await recordCommand(page, "execute")
    await toast.getByRole("button", { name: "Fetch" }).click()
    await expect(toast.getByRole("button", { name: "Fetching…" })).toBeDisabled()

    // back in the repository of the merge, ⌘T during that fetch: the update goes on from it, no fetch of its own
    await switchRepository(page, repo.path)
    await emitTauriEvent(page, "update_project")
    await expect.poll(() => executions.length).toBe(2)
    await expect(toast.getByRole("button", { name: "Fetching…" })).toBeDisabled()
    await repo.holdFetchLock(false)
    await expectLandedInPlace(page, toast)
    expect(executions.map(call => call.intent)).toEqual([{ kind: "fetchBaseline" }, { kind: "updateProject", fetch: true }])
  })

  test("should say in the toast of a merge what a sync found landed after an update outside the app", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { repo, toast } = await mergeFeatureAuth(page, setupRepo, syncAndWaitForBranches)
    // `bd update` beside the app: the page learns it from the next sync
    await repo.updateOutsideTheApp()
    await syncAndWaitForBranches()
    // the landing of its branch, in place: the merge was its news
    await expect(toast.locator("[data-slot=toast-title]")).toHaveText("feature-auth landed in origin/master")
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("Found by a sync")
    await expectNoActions(toast)
    await expect(toasts(page)).toHaveCount(1)
  })
})
