// @spec spec/branch/next-step.spec.md
// @spec spec/repository/review-requests.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import type { Box } from "../helpers/header-helpers"
import { boxOf } from "../helpers/header-helpers"
import { AWAITING_CODE_OWNER, availableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest, reviewer } from "../helpers/pull-request-helpers"
import { availableReviewRequests, getYourTurnCard, reviewRequest } from "../helpers/review-request-helpers"
import { rewriteNextRemoteStatus } from "../helpers/sync-helpers"
import { waitForStorePersistence } from "../helpers/wait-helpers"

const BRANCH = "JIRA-456"

/** The box of a track of a row, also while it holds no item yet: as wide as its column, as high as its content */
async function trackBox(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

/** Where the row of JIRA-456 is, and where its commits, its pull request and its step stand */
async function rowLayout(page: Page): Promise<{ row: Box; commits: Box; pullRequest: { x: number; width: number }; step: Box }> {
  const row = findBranchRow(page, BRANCH)
  const { x, width } = await trackBox(row.locator("[data-track=pr]"))
  return {
    row: await boxOf(row),
    commits: await boxOf(row.locator("[data-track=commits]")),
    pullRequest: { x, width },
    step: await trackBox(row.locator("[data-track=step]")),
  }
}

/** JIRA-456 pushed, as the remote status of the next sync says */
async function pushedOnNextSync(page: Page): Promise<void> {
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === BRANCH ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } : status,
  )
}

test.describe("Late facts", () => {
  test("should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await pushedOnNextSync(page)
    const awaited = pullRequest(BRANCH, 1078, { reviewers: [reviewer("ikorennoy", "requested")] })
    const repo = await setupRepo("issue_links", {
      patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
      pullRequests: { ...availablePullRequests(awaited), users: [{ login: "ikorennoy", name: "Ilya Korennoy" }] },
    })
    await repo.setReviewAnswers({ answers: { [BRANCH]: AWAITING_CODE_OWNER } })
    await repo.setReviewRequests(availableReviewRequests([reviewRequest("review-me", 101, 30)]))
    await syncAndWaitForBranches()

    // the first start: every fact came, the view keeps its layout for the next one
    await expect(getYourTurnCard(page)).toBeVisible()
    await expect(findBranchRow(page, BRANCH).getByTestId("next-step-slot")).toHaveAttribute("data-step", "awaiting")
    const settled = await rowLayout(page)
    await waitForStorePersistence(page, "repositoryState", states =>
      Object.values(states as Record<string, { settledLayout?: { yourTurn: number; tracks: Record<string, number> } }>).some(
        state => (state.settledLayout?.yourTurn ?? 0) > 0 && (state.settledLayout?.tracks["pr"] ?? 0) > 0,
      ),
    )

    // the next start: GitHub answers after the first rows
    await repo.holdRemoteAnswers({ hold: ["pullRequests", "reviewRequests"] })
    await pushedOnNextSync(page)
    // a repository synced before syncs when the window opens it; GitHub is asked once that sync ended
    await page.reload()
    await expect.poll(() => repo.getArrivedAnswers()).toEqual(expect.arrayContaining(["pullRequests", "reviewRequests"]))
    // Your Turn holds its place as it checks, the columns their widths, no pull request and no step known yet
    await expect(page.getByTestId("your-turn-checking")).toHaveText("Your TurnChecking GitHub…")
    await expect(findBranchRow(page, BRANCH).getByTestId("next-step-slot")).toHaveAttribute("data-step", "pending")
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).toHaveCount(0)
    const before = await rowLayout(page)
    expect(before).toEqual(settled)

    // the answers come: the island, the chip and the step fill the places they had, nothing moves
    await repo.holdRemoteAnswers({ release: ["pullRequests", "reviewRequests"] })
    await expect(getYourTurnCard(page).getByTestId("review-request-row")).toHaveCount(1)
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).toBeVisible()
    await expect(findBranchRow(page, BRANCH).getByTestId("next-step-slot")).toHaveAttribute("data-step", "awaiting")
    await expect(page.getByTestId("your-turn-checking")).toHaveCount(0)
    expect(await rowLayout(page)).toEqual(settled)
  })
})
