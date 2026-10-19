// @spec spec/repository/landing-overview.spec.md
import type { PatronusRobotDetails } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { AWAITING_CODE_OWNER, DAY_ROBOTS, NO_REVIEW_NEEDED, ROBOTS_NOW, availableRobots, minutes, robot, setupRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls, routeNewPullRequest } from "../helpers/pull-request-helpers"
import { pushedToast, routePush } from "../helpers/push-helpers"
import { EARLIER_COMMIT, landingRow, openLandingOverview } from "../helpers/review-status-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const NOW = ROBOTS_NOW
const { running: RUNNING, failed: FAILED, merged: MERGED } = DAY_ROBOTS
// a branch merged and archived since
const ARCHIVED = robot("billing", "archived-robot", { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - minutes(200), finishTime: NOW - minutes(160) })
// a week ago: not of the last day
const OLD = robot("feature-docs", "old-robot", { status: "canceled", startTime: NOW - minutes(3000), finishTime: NOW - minutes(2990) })

test.describe("Landing overview", () => {
  test("should count the user's moves, put the most urgent node in the header and group the work by where it is", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED, ARCHIVED, OLD)
    const indicator = page.getByTestId("landing-indicator")
    // ABC-123 needs a fix (its Safe Push failed on it as it is); the four branches GitHub lists no pull request of
    // are a Safe Push nobody judged; JIRA-456 is landing, feature-db is in master
    await expect(indicator).toHaveAccessibleName("On the way to master: 5 your move · 1 landing · 1 in master")
    await expect(indicator.getByTestId("landing-moves")).toHaveText("5")
    await expect(indicator).toHaveAttribute("data-urgent", "needsFix")
    await expect(indicator.getByTestId("robot-ring")).toBeVisible()

    const popover = await openLandingOverview(page)
    await expect(popover.getByRole("heading", { level: 2 })).toHaveText("On the way to master: 5 your move · 1 landing · 1 in master")
    // the most urgent first: the failed robot with its actions
    const moves = popover.getByTestId("landing-your-move")
    await expect(moves.getByTestId("robots-row").first()).toHaveAttribute("data-branch-name", "ABC-123")
    await expect(moves.getByTestId("robots-row").first().getByTestId("robots-row-state")).toHaveText("Safe Push · Failed · 50 minutes ago")
    await expect(moves.getByTestId("landing-row")).toHaveCount(4)
    const running = popover.getByTestId("landing-running").getByTestId("robots-row")
    await expect(running).toHaveAttribute("data-branch-name", "JIRA-456")
    await expect(running.getByTestId("robots-row-state")).toHaveText("Safe Push · 18m · ~22m left")
    await expect(running.getByTestId("robot-progress")).toHaveAttribute("aria-valuenow", "45")
    await expect(landingRow(popover.getByTestId("landing-in-master"), "feature-db").getByTestId("landing-row-reason")).toHaveText("Merged by Patronus")
    // the reason of a row is the hover of its node: the state in full, what it names opens from there
    await landingRow(popover.getByTestId("landing-in-master"), "feature-db").getByTestId("landing-row-reason").hover()
    await expect(page.getByTestId("lane-node-details")).toHaveText("Merged by Patronus")
    // the last day without what the In master row says: a branch archived since, no row to show
    const finished = popover.getByTestId("landing-finished").getByTestId("robots-row")
    await expect(finished).toHaveCount(1)
    await expect(finished).toHaveAttribute("data-branch-name", "billing")
    await expect(finished.getByRole("button", { name: "billing", exact: true })).toHaveCount(0)
    await finished.getByRole("button", { name: "Open Safe Push of billing in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([ARCHIVED.url])

    // a branch name shows its row
    await running.getByRole("button", { name: "JIRA-456", exact: true }).click()
    await expect(popover).toBeHidden()
    await verifyBranchState(findBranchRow(page, "JIRA-456"), "open")
  })

  test("should cancel a running robot from the overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))
    const popover = await openLandingOverview(page)
    await popover.getByRole("button", { name: "Cancel Safe Push of JIRA-456" }).click()
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })).toBeVisible()
    await expect(popover.getByTestId("landing-running").getByTestId("robots-row-state")).toHaveText("Safe Push · Canceling")
  })

  test("should restart a failed robot from the overview, and open the pull request it was started from", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const pullRequest = { number: 7, url: "https://github.com/acme/monorepo/pull/7" }
    await setupRobots(page, setupRepo, syncAndWaitForBranches, { ...FAILED, pullRequest })
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot", { startTime: NOW }))
    const popover = await openLandingOverview(page)
    const row = popover.getByTestId("landing-your-move").getByTestId("robots-row")
    // the actions of the branch menu and the robot details, in their order and under their names
    await expect(row.getByRole("button", { name: "Restart Safe Push of ABC-123" })).toBeVisible()
    expect(await row.locator("button[aria-label]").evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label")))).toEqual([
      "Restart Safe Push of ABC-123",
      "Copy Prompt for Agent of ABC-123",
      "Copy Prompt with CI Details of ABC-123",
      "Open Safe Push of ABC-123 in Patronus",
      "Open Pull Request #7 of ABC-123",
    ])
    await row.getByRole("button", { name: "Open Pull Request #7 of ABC-123" }).click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest.url])
    // the mode of the robot is a link to it
    await row.getByTestId("robots-row-mode").click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest.url, FAILED.url])
    await row.getByRole("button", { name: "Restart Safe Push of ABC-123" }).click()
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
  })

  test("should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await routeCommand(page, "get_patronus_failing_robots", () => ["running-robot"])
    await routeCommand(
      page,
      "get_patronus_robot_details",
      () =>
        ({
          robot: RUNNING,
          profileId: null,
          profileName: null,
          checks: { failed: 1, failedNames: ["Bazel Community Build Aggregator"], running: 2, passed: 9, canceled: 0 },
          problems: [],
          failing: true,
          progress: { remainingSeconds: 1500, checkName: "IntelliJ Aggregator", checkPercent: 36, overtimeSeconds: null, queued: 0, probablyHanging: false },
        }) satisfies PatronusRobotDetails,
    )
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    const toast = page.locator("[data-slot=toast]", { hasText: "A required check of JIRA-456 failed" })
    await expect(toast).toContainText("Bazel Community Build Aggregator · the Safe Push will fail")
    const indicator = page.getByTestId("landing-indicator")
    await expect(indicator).toHaveAttribute("data-urgent", "needsFix")
    const popover = await openLandingOverview(page)
    const running = popover.getByTestId("landing-running").getByTestId("robots-row")
    await expect(running.getByTestId("robots-row-state")).toHaveText("Safe Push · 18m · Failing")
    await expect(running.getByTestId("robots-row-failed")).toHaveText("Failed: Bazel Community Build Aggregator")
    await expect(running.getByTestId("robot-progress")).toHaveAttribute("data-failing", "true")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // no use waiting for it: the toast cancels it
    await toast.getByRole("button", { name: "Cancel Safe Push" }).click()
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })).toBeVisible()
  })

  test("should run the next step of a move from the overview at its row: a Safe Push, a pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a stack is one move: its root's row, the step of the whole stack
    const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
    await repo.setReviewAnswers({ answers: { docs: NO_REVIEW_NEEDED, "stack:auth": AWAITING_CODE_OWNER } })
    await syncAndWaitForBranches()
    await routePush(page, {})
    await routeNewPullRequest(page)
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("docs", "docs-robot", { startTime: NOW }))
    let popover = await openLandingOverview(page)
    await expect(landingRow(popover, "auth").getByText("4 branches · 4 commits")).toBeVisible()
    await expect(landingRow(popover, "login")).toHaveCount(0)

    // the Safe Push of docs has nothing to ask: it pushes and starts at once, the row shown
    const docs = landingRow(popover, "docs")
    await expect(docs).toHaveAttribute("data-kind", "ready")
    await docs.getByTestId("next-step").click()
    await expect(popover).toBeHidden()
    await expect.poll(() => starts.length).toBe(1)
    await expect(findBranchRow(page, "docs")).toBeInViewport()
    await page.mouse.move(0, 0)

    // the step of the whole auth stack: its pull request, whose approvals its Safe Push counts
    popover = await openLandingOverview(page)
    const step = landingRow(popover, "auth").getByTestId("next-step")
    await expect(step).toHaveText("Create Pull Request of the Stack…")
    await step.click()
    await expect(createPullRequestDialog(page)).toBeVisible()
  })

  test("should be hidden while nothing is on the way to master and nothing ran in the last day", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    await routePush(page, {})
    const indicator = page.getByTestId("landing-indicator")
    // not on Space, no pull requests GitHub lists: the push is the step
    await expect(indicator).toHaveAccessibleName("On the way to master: 1 your move")
    await getPushButton(findBranchRow(page, "test-branch")).click()
    await expect(pushedToast(page, "test-branch")).toBeVisible()
    // pushed: it lands through its pull request, which nobody opened; the work still waits
    await expect(indicator).toHaveAccessibleName("On the way to master: 1 waiting")
  })
})
