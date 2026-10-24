// @spec spec/branch/next-step.spec.md
// @spec spec/repository/review-requests.spec.md
// @spec spec/branch/stack-island.spec.md
// @spec spec/branch/branch-preview.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import type { Box } from "../helpers/header-helpers"
import { boxOf } from "../helpers/header-helpers"
import { AWAITING_CODE_OWNER, availableRobots, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest, reviewer } from "../helpers/pull-request-helpers"
import { availableReviewRequests, getYourTurnCard, reviewRequest } from "../helpers/review-request-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { pushedOnEverySync, pushedOnNextSync } from "../helpers/sync-helpers"
import { animationsEnded, waitForStorePersistence } from "../helpers/wait-helpers"

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

test.describe("Late facts", () => {
  test("should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // pushed in both starts, whatever syncs run between them
    await pushedOnEverySync(page, BRANCH)
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

  test("should keep the sections and the push of the preview in their places while Patronus and GitHub answer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // ABC-123 pushed, with a pull request and a failed Safe Push; JIRA-456 not pushed yet
    await pushedOnNextSync(page, "ABC-123")
    const repo = await setupRepo("issue_links", {
      patronus: { ...availableRobots(robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })), githubRepository: "acme/monorepo" },
      pullRequests: availablePullRequests(pullRequest("ABC-123", 7)),
    })
    await repo.holdRemoteAnswers({ hold: ["pullRequests", "robots"] })
    await syncAndWaitForBranches()
    await expect.poll(() => repo.getArrivedAnswers()).toEqual(expect.arrayContaining(["pullRequests", "robots"]))

    // the sections of a pushed branch hold their places while Patronus and GitHub answer
    const first = await openBranchPreview(page, "ABC-123")
    await expect(first.getByTestId("branch-preview-pull-request-pending")).toHaveText("Checking GitHub for a pull request of ABC-123…")
    await expect(first.getByTestId("branch-preview-patronus-pending")).toHaveText("Asking Patronus for the Safe Push of ABC-123…")
    await page.keyboard.press("Escape")
    await expect(first).toHaveCount(0)

    // the first push of a branch whose landing may push it: in its place, waiting for the answer that tells
    const unpushed = await openBranchPreview(page, "JIRA-456")
    const push = unpushed.getByTestId("push-button")
    await expect(push).toBeDisabled()
    await push.hover()
    await expect(page.getByTestId("push-description")).toHaveText("Waits for the answer of Patronus")
    // where it stands once the preview opened: its animation scales the whole box
    await animationsEnded(unpushed)
    const pushBox = await boxOf(push)
    // Patronus answers: Safe Push and Create Pull Request… push it, the button stays where it was, disabled with why
    await repo.holdRemoteAnswers({ release: ["robots"] })
    await expect(page.getByTestId("push-description")).toHaveText('Safe Push and Create Pull Request… push JIRA-456 first: "⋯" pushes it alone')
    await expect(push).toBeDisabled()
    expect(await boxOf(push)).toEqual(pushBox)
    await page.keyboard.press("Escape")
    await expect(unpushed).toHaveCount(0)

    // the answer of GitHub fills the place of its section
    const preview = await openBranchPreview(page, "ABC-123")
    await expect(preview.getByTestId("branch-preview-patronus")).toBeVisible()
    const pending = preview.getByTestId("branch-preview-pull-request-pending")
    await expect(pending).toHaveText("Checking GitHub for a pull request of ABC-123…")
    await animationsEnded(preview)
    const top = (await boxOf(pending)).y
    await repo.holdRemoteAnswers({ release: ["pullRequests"] })
    expect((await boxOf(preview.getByTestId("branch-preview-pull-request"))).y).toBe(top)
    await expect(pending).toHaveCount(0)
  })

  test("should keep the second header line of a stack island in its place while the pull requests come back", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stack_island", { pullRequests: availablePullRequests(pullRequest("air", 812, { reviewDecision: "approved" })) })
    await syncAndWaitForBranches()
    const island = page.locator('[data-testid="stack-island"][data-stack-root="air"]')
    const lines = island.getByTestId("stack-island-lines")
    const firstRow = findBranchRow(page, "air")

    // the first start: the line says what GitHub answered, the view keeps that it had it
    await expect(lines.getByTestId("stack-island-pull-requests")).toHaveText("1 pull request open · 1 approved")
    const settled = await boxOf(firstRow)
    await waitForStorePersistence(page, "repositoryState", states =>
      Object.values(states as Record<string, { settledLayout?: { stackLines?: string[] } }>).some(state => state.settledLayout?.stackLines?.includes("air") === true),
    )

    // the next start: GitHub answers after the first rows, the line keeps its place empty
    await repo.holdRemoteAnswers({ hold: ["pullRequests"] })
    await page.reload()
    await expect.poll(() => repo.getArrivedAnswers()).toContain("pullRequests")
    await expect(lines).toHaveAttribute("data-reserved", "true")
    await expect(lines.getByTestId("stack-island-pull-requests")).toHaveCount(0)
    expect(await boxOf(firstRow)).toEqual(settled)

    // the answer fills the line where it stood: the rows below stay
    await repo.holdRemoteAnswers({ release: ["pullRequests"] })
    await expect(lines.getByTestId("stack-island-pull-requests")).toHaveText("1 pull request open · 1 approved")
    await expect(lines).not.toHaveAttribute("data-reserved")
    expect(await boxOf(firstRow)).toEqual(settled)
  })
})
