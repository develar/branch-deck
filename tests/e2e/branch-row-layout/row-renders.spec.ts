// @spec spec/branch/branch-row.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { recordRenders, renderCounts, resetRenders } from "../helpers/render-helpers"
import type { RenderCounts } from "../helpers/render-helpers"
import { runAndWaitForSync, waitForViewVersion } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

/**
 * The renders of the page (`countRender`) a sync from the Sync button made, read once the page holds the view in which
 * the session committed it
 */
async function syncRenders(page: Page, repo: TestRepositoryBuilder): Promise<RenderCounts> {
  await resetRenders(page)
  await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
  const view = await repo.sessionView()
  expect(view).not.toBeNull()
  await waitForViewVersion(page, view!.version)
  return renderCounts(page)
}

/** The branches of `stack_island`: `docs` and the small stack auth/login in the table, the stack of `air` an island */
const BRANCHES = ["air", "auth", "bazel", "bridge", "deploy", "docs", "login", "runtime", "side-fix", "side-test"]

/**
 * What rendered, by key: a patch of the session renders each part it changed once, and how the session batched the
 * changes of a run into patches (16 ms each, `PATCH_INTERVAL`) is the load's, so the keys are exact, not the counts
 */
function rendered(counts: RenderCounts): string[] {
  return Object.keys(counts).sort()
}

test.describe("The renders of a sync", () => {
  test("should render only the row of the branch a sync changed, and of a sync that changes nothing the status of each row", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await recordRenders(page)
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "docs"), 1)

    // every branch from `Created` to `Unchanged`: the status of each row renders, no row, no card (B5)
    const resync = await syncRenders(page, repo)
    expect(rendered(resync)).toEqual(BRANCHES.map(name => `status:${name}`))
    await expect(findBranchRow(page, "docs").getByTestId("branch-status")).toHaveText("not pushed")

    // a commit of `docs` in a terminal: its row renders for its count of commits and for the push of the two, its
    // status for its sync; no other row, neither the table nor the island (B3)
    await repo.commit("(docs) Document the sessions", { "sessions.md": "# Sessions\n" }, 1704300000)
    const oneBranch = await syncRenders(page, repo)
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    expect(rendered(oneBranch)).toEqual(["row:docs", "status:docs"])
  })
})
