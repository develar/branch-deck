// @spec openspec/specs/branch/branch-row/spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { availableRobots } from "../helpers/patronus-helpers"
import { recordRenders, renderCounts, renderedKeys, resetRenders } from "../helpers/render-helpers"
import type { RenderCounts } from "../helpers/render-helpers"
import { runAndWaitForSync, waitForSettledSession, waitForViewVersion } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { waitForStepFacts } from "../helpers/wait-helpers"

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
 * What rendered, by key, the next steps of the rows (`step:`) aside: a patch of the session renders each part it changed
 * once, and how the session batched the changes of a run into patches (16 ms each, `PATCH_INTERVAL`) is the load's, so
 * the keys are exact, not the counts
 */
function rendered(counts: RenderCounts): string[] {
  return Object.keys(counts)
    .filter(key => !key.startsWith("step:"))
    .sort()
}

// in WebKit too (`pnpm e2e:webkit`), the engine of the app, as `perf:table` counts the renders in both engines
test.describe("The renders of a sync", { tag: "@webkit" }, () => {
  test("should render only the row of the branch a sync changed, and of a sync that changes nothing the status of each row", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await recordRenders(page)
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "docs"), 1)
    // the facts of the first sync came: their patch of every next step renders the step of every row, no row
    await waitForStepFacts(page.getByTestId("next-step-slot"))

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

  test("should render the steps of the rows for the landing needs they wait for after a sync of one more commit on one branch, no other row, no card", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await recordRenders(page)
    // Patronus answers: after each sync that moved a tip the session computes the landing needs of the new tips, and the
    // next step of every branch waits for them meanwhile, then has them (a patch of every next step, twice)
    const repo = await setupRepo("stack_island", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await waitForSettledSession(page, repo)
    await waitForStepFacts(page.getByTestId("next-step-slot"))
    // every branch from `Created` to `Unchanged`, so the sync below changes the status of `docs` alone
    await syncRenders(page, repo)

    await resetRenders(page)
    await repo.commit("(docs) Document the sessions", { "sessions.md": "# Sessions\n" }, 1704300000)
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    await waitForSettledSession(page, repo)
    await waitForStepFacts(page.getByTestId("next-step-slot"))
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    const counts = await renderCounts(page)
    // the step of every row of the table and of the island rendered for the needs it waited for and then had
    expect(renderedKeys(counts, "step:").sort()).toEqual(BRANCHES.map(name => `step:${name}`))
    // the row of `docs` for its commits, its status for its sync; no other row, no status of the nine others, neither the
    // table nor the island: a row reads of its step the ring of its node, its status cell the status and what landed it,
    // the island the hold of its root, each kept while it says the same (B3)
    expect(rendered(counts)).toEqual(["row:docs", "status:docs"])
  })
})
