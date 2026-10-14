// @spec spec/branch/patronus.spec.md
import type { Page, Route } from "@playwright/test"
import type { PatronusMergeRequest, PatronusReviewRequirement, PatronusRobot, PatronusRobotDetails, PatronusStartOptions } from "../../../src/lib/bindings"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import type { PushResult } from "../helpers/push-helpers"
import { confirmForcePush, pushOutcome, routePush } from "../helpers/push-helpers"
import { hoverStatusChip, openBranchPreview } from "../helpers/review-status-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot")
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [
    { id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [] },
    { id: "dry-only", name: "Dry Only", description: null, allowedPushModes: ["dryRun"] },
  ],
  defaultProfileId: null,
}

/** Answers a Patronus command in the page and records the parameters of each call */
async function routeCommand(page: Page, command: string, answer: (params: Record<string, unknown>) => unknown): Promise<Record<string, unknown>[]> {
  const calls: Record<string, unknown>[] = []
  await page.route(`**/invoke/${command}`, async (route: Route) => {
    const params = route.request().postDataJSON() as Record<string, unknown>
    calls.push(params)
    await route.fulfill({ json: answer(params) })
  })
  return calls
}

/** The issue_links repository with these robots; the unassigned commits card renders after the branches and would move the rows away from the pointer */
async function setupRobots(page: Page, setupRepo: (template: string, options: { patronus: unknown }) => Promise<unknown>, syncAndWaitForBranches: () => Promise<unknown>, ...robots: PatronusRobot[]) {
  await setupRepo("issue_links", { patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  // the robots load after the sync: a push before they did would not know that one runs
  for (const it of robots) {
    if (it.branchName !== null) {
      await expect(chip(page, it.branchName)).toBeVisible()
    }
  }
}

/** Answers the question about the running Safe Push of JIRA-456 with Back: Escape, once the dialog has the focus */
async function answerPushGuardBack(page: Page): Promise<void> {
  const dialog = page.getByTestId("push-guard-dialog")
  await expect(dialog).toContainText("Safe Push of JIRA-456 is running")
  // the dialog moves the focus to its primary button; an Escape before that closes nothing
  await expect(dialog.getByRole("button", { name: "Cancel, Push and Restart" })).toBeFocused()
  await page.keyboard.press("Escape")
  await expect(dialog).toBeHidden()
}

function chip(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

/** The first push of JIRA-456 finds its remote branch moved (someone pushed a commit), the second one goes through */
const REMOTE_MOVED: PushResult[] = [
  {
    outcome: "remoteMoved",
    unpushedCommits: ["j1", "j2", "j3"],
    commitsBehind: 1,
    remoteTip: "moved-tip",
    droppedCommits: [{ id: "aaaa000011112222", subject: "Fix the pool", author: "Ann", authorTime: 1704178800 }],
  },
  {},
]

/** Pushes JIRA-456 into its running robot with `answer`; the remote moved, so the warning toast comes up */
async function pushIntoMovedRemote(page: Page, answer: "Push Anyway" | "Cancel, Push and Restart") {
  await getPushButton(findBranchRow(page, "JIRA-456")).click()
  await page.getByTestId("push-guard-dialog").getByRole("button", { name: answer }).click()
  const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
  await expect(toast).toBeVisible()
  return toast
}

test.describe("Patronus actions", () => {
  test("should start a Safe Push from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routePush(page, {})
    const options = await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }) satisfies PatronusRobot)

    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Safe Push…" }).click()
    const form = page.getByTestId("patronus-start-form")
    await expect(form.getByTestId("patronus-start-push-note")).toHaveText("JIRA-456 isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.")
    await form.getByTestId("patronus-profile").click()
    // a profile that allows only dry runs is not offered for a Safe Push
    await expect(page.getByRole("option", { name: "Dry Only" })).toHaveCount(0)
    await page.getByRole("option", { name: "Large Scale Change" }).click()
    await form.getByRole("button", { name: "Push and Start Safe Push" }).click()

    await expect(chip(page, "JIRA-456")).toHaveText("Running")
    await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
    expect(options[0]).toMatchObject({ baselineBranch: "origin/master", lastRobotId: null })
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash", profileId: "lsc" })])
  })

  test("should open the Safe Push form from the row button and from the \"⋯\" menu, and say why a start is not possible", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const form = page.getByTestId("patronus-start-form")

    // a robot runs on the commit JIRA-456 is at: Patronus would refuse a start
    const running = findBranchRow(page, "JIRA-456").getByRole("button", { name: "Safe Push…" })
    await expect(running).toBeDisabled()
    await running.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "is running" })).toHaveText("A Safe Push of the last commit is running")

    const row = findBranchRow(page, "ABC-123")
    const button = row.getByRole("button", { name: "Safe Push…" })
    await button.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "code-owner review" })).toHaveText("Checks the code-owner review, offers a merge request, then starts the robot.")
    await button.click()
    await expect(form).toBeVisible()
    await expect(form.getByTestId("patronus-start-explanation")).toContainText("Patronus rebases ABC-123 onto master")
    // the button opens the form, it doesn't toggle the row
    await verifyBranchState(row, "closed")
    await form.getByRole("button", { name: "Cancel" }).click()
    await expect(form).toHaveCount(0)

    // "⋯" has the menu of the right-click
    await row.getByRole("button", { name: "More actions for ABC-123" }).click()
    const rowMenu = page.getByRole("menu", { name: "More actions for ABC-123" })
    await expect(rowMenu.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(rowMenu.getByRole("menuitem", { name: "Create Merge Request…" })).toBeEnabled()
    await rowMenu.getByRole("menuitem", { name: "Safe Push…" }).click()
    await expect(form).toBeVisible()
    await verifyBranchState(row, "closed")

    // the menu of the branch with the running robot says why too
    await findBranchRow(page, "JIRA-456").getByRole("button", { name: "More actions for JIRA-456" }).click()
    // the menu of ABC-123 may still be closing
    const menu = page.getByRole("menu", { name: "More actions for JIRA-456" })
    const menuItem = menu.getByRole("menuitem", { name: "Safe Push…" })
    await expect(menuItem).toBeDisabled()
    await menuItem.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "is running" })).toHaveText("A Safe Push of the last commit is running")
    // Escape closes the menu with its description
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)
  })

  test("should create a merge request from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const pushes = await routePush(page, {})
    const mergeRequest: PatronusMergeRequest = { projectKey: "IJ", reviewKey: "IJ-CR-8", url: "https://jetbrains.team/p/ij/reviews/8" }
    const creates = await routeCommand(page, "create_patronus_merge_request", () => mergeRequest)

    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Create Merge Request…" }).click()

    // Space opens a merge request of a pushed branch
    const toast = page.locator("[data-slot=toast]", { hasText: "Created merge request IJ-CR-8" })
    await expect(toast).toBeVisible()
    expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
    expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", baselineBranch: "origin/master" })])
    expect(await getOpenedUrls(page)).toEqual([mergeRequest.url])
    await toast.getByRole("button", { name: "Open merge request" }).click()
    expect(await getOpenedUrls(page)).toEqual([mergeRequest.url, mergeRequest.url])
  })

  test("should cancel the robot for a new Safe Push from the form, and not restart it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot runs on an older commit, so the form can start one on the branch as it is
    const older = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const order: string[] = []
    const record = (call: string, answer: unknown) => () => {
      order.push(call)
      return answer
    }
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    await routeCommand(page, "get_patronus_robot_status", () => "running")
    await routeCommand(page, "cancel_patronus_robot", record("cancel", { ...older, status: "canceling" }))
    await routeCommand(page, "push_branch", record("push", pushOutcome("JIRA-456")))
    await routeCommand(page, "restart_patronus_robot", record("restart", robot("JIRA-456", "restarted-robot")))
    const starts = await routeCommand(page, "start_patronus_robot", record("start", robot("JIRA-456", "started-robot", { sourceRevision: null })))

    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Safe Push…" }).click()
    await page.getByTestId("patronus-start-form").getByRole("button", { name: "Push and Start Safe Push" }).click()

    // the form's start is the new robot: no restart of the old one
    const dialog = page.getByTestId("push-guard-dialog")
    await expect(dialog).toContainText("The new Safe Push takes the pushed commit.")
    await expect(dialog.getByRole("button", { name: "Cancel, Push and Restart" })).toHaveCount(0)
    await dialog.getByRole("button", { name: "Cancel and Push" }).click()

    await expect.poll(() => order).toEqual(["cancel", "push", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    await expect(chip(page, "JIRA-456")).toHaveText("Running")
  })

  test("should restart a failed robot and cancel a running one from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))

    let popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Cancel Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Restart Safe Push" }).click()
    await expect(chip(page, "ABC-123")).toHaveText("Running")
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await page.keyboard.press("Escape")

    popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])
  })

  test("should push and restart a failed robot whose branch changed since, from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // failed on an older commit; the fix is not on the remote yet
    const older = robot("ABC-123", "failed-robot", { status: "failure", sourceRevision: "older-commit", finishTime: 1704142800 })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const order: string[] = []
    await routeCommand(page, "push_branch", (params) => {
      order.push("push")
      return pushOutcome(params.branchName as string)
    })
    await routeCommand(page, "restart_patronus_robot", (params) => {
      order.push(`restart ${params.robotId as string}`)
      return robot("ABC-123", "restarted-robot")
    })

    const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByTestId("patronus-outdated")).toContainText("Push and restart to test the current branch.")
    await popover.getByRole("button", { name: "Push and Restart" }).click()

    // the restart takes the commit just pushed
    await expect(page.getByText("Safe Push restarted")).toBeVisible()
    expect(order).toEqual(["push", "restart failed-robot"])
    await expect(chip(page, "ABC-123")).toHaveText("Running")
  })

  test("should show the profile, the checks and the problems in the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
    await routeCommand(page, "get_patronus_robot_details", () => ({
      robot: { ...FAILED, sourceRevision: null },
      profileId: "lsc",
      profileName: "Large Scale Change",
      checks: { failed: 1, running: 0, passed: 9, canceled: 0, failedNames: ["Fast Tests"] },
      problems: [{ title: "Tests failed", subsystem: "TEAMCITY" }],
    }) satisfies PatronusRobotDetails)

    const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByTestId("patronus-profile-name")).toHaveText("Large Scale Change")
    await expect(popover.getByTestId("patronus-checks")).toHaveText("1 failing (Fast Tests), 9 passed")
    await expect(popover.getByTestId("patronus-problems")).toHaveText("Tests failed")
  })

  test.describe("code-owner review", () => {
    const MERGE_REQUEST: PatronusMergeRequest = { projectKey: "IJ", reviewKey: "IJ-CR-7", url: "https://jetbrains.team/p/ij/reviews/7" }
    const REQUIRED: PatronusReviewRequirement = {
      required: true,
      mergeRequest: null,
      groups: [{ name: "Air Plugin Release Team", reviewers: [{ fullName: "Alice", username: "alice", available: true, suggested: true }] }],
      skipped: false,
    }

    test("should name the missing approvals, create the merge request and start with it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", params => params.recheck === true ? { ...REQUIRED, required: false, groups: [], mergeRequest: MERGE_REQUEST } : REQUIRED)
      const creates = await routeCommand(page, "create_patronus_merge_request", () => MERGE_REQUEST)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }))

      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Safe Push…" }).click()
      const review = page.getByTestId("patronus-review")
      await expect(review).toHaveAttribute("data-status", "required")
      await expect(review.getByTestId("patronus-review-groups")).toHaveText("Air Plugin Release Team (suggested: Alice)")

      await review.getByRole("button", { name: "Create Merge Request" }).click()
      // pushed first: Space opens a merge request of a pushed branch
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      await expect(review).toHaveAttribute("data-status", "approved")
      await expect(review).toHaveText("Approved in IJ-CR-7.")
      expect(await getOpenedUrls(page)).toEqual([MERGE_REQUEST.url])
      expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", baselineBranch: "origin/master" })])
      expect(checks.map(params => params.recheck)).toEqual([false, true])

      await page.getByTestId("patronus-start-form").getByRole("button", { name: /Start Safe Push/ }).click()
      await expect(chip(page, "JIRA-456")).toHaveText("Running")
      expect(starts).toEqual([expect.objectContaining({ mergeRequest: MERGE_REQUEST })])
    })

    test("should not check the review of a dry run", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", () => REQUIRED)
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Dry Run…" }).click()
      await expect(page.getByTestId("patronus-start-explanation")).toContainText("without pushing into master")
      await expect(page.getByTestId("patronus-review")).toHaveCount(0)
      expect(checks).toHaveLength(0)
    })

    test("should say \"Needs review\" for a robot that failed for missing approvals, and offer a new Safe Push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")
      await expect(chip(page, "ABC-123")).toHaveAttribute("data-color", "warning")

      const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
      await expect(popover.getByTestId("patronus-needs-review")).toBeVisible()
      await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
      await popover.getByRole("button", { name: "Safe Push…" }).click()
      await expect(page.getByTestId("patronus-start-form")).toBeVisible()
    })

    test("should close the branch preview for the Safe Push form of its Patronus section", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")

      const preview = await openBranchPreview(page, "ABC-123")
      await preview.getByTestId("branch-preview-patronus").getByRole("button", { name: "Safe Push…" }).click()
      // the form is below the row, behind the modal preview
      await expect(preview).toHaveCount(0)
      await expect(page.getByTestId("patronus-start-form")).toBeVisible()
    })
  })

  test.describe("pushing into a branch whose Safe Push runs", () => {
    test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    })

    test("should ask first, and push only when told to", async ({ page }) => {
      const pushes = await routeCommand(page, "push_branch", params => pushOutcome(params.branchName as string))
      const statuses = await routeCommand(page, "get_patronus_robot_status", () => "running")
      const dialog = page.getByTestId("push-guard-dialog")

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await answerPushGuardBack(page)
      expect(pushes).toHaveLength(0)
      expect(statuses).toHaveLength(1)

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await dialog.getByRole("button", { name: "Push Anyway" }).click()
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      expect(pushes).toHaveLength(1)
    })

    test("should cancel the robot, push and restart it on the pushed commit", async ({ page }) => {
      const order: string[] = []
      await routeCommand(page, "get_patronus_robot_status", () => "running")
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        return { ...RUNNING, status: "canceling" }
      })
      await routeCommand(page, "push_branch", (params) => {
        order.push("push")
        return pushOutcome(params.branchName as string)
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await page.getByTestId("push-guard-dialog").getByRole("button", { name: "Cancel, Push and Restart" }).click()
      await expect(page.getByText("Safe Push restarted")).toBeVisible()
      expect(order).toEqual(["cancel", "push", "restart"])
      await expect(chip(page, "JIRA-456")).toHaveText("Running")
    })

    test("should push at once when the robot finished meanwhile", async ({ page }) => {
      await routePush(page, {})
      await routeCommand(page, "get_patronus_robot_status", () => "failure")
      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      await expect(page.getByTestId("push-guard-dialog")).toHaveCount(0)
      await expect(chip(page, "JIRA-456")).toHaveText("Failed")
    })

    test("should ask again before the push of the remote-moved toast", async ({ page }) => {
      const statuses = await routeCommand(page, "get_patronus_robot_status", () => "running")
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      const toast = await pushIntoMovedRemote(page, "Push Anyway")
      await expect(toast).not.toContainText("canceled")

      // the push drops Ann's commit: it confirms in the row first
      await toast.getByRole("button", { name: "Force Push" }).click()
      await confirmForcePush(page, "Force Push (3)")

      // the robot still runs: the same question, and Back pushes nothing
      await answerPushGuardBack(page)
      expect(pushes.map(push => push.expectedRemote)).toEqual([null])
      expect(statuses).toHaveLength(2)
    })

    test("should restart a robot canceled for a push the remote refused once the toast pushes", async ({ page }) => {
      const order: string[] = []
      let canceled = false
      await routeCommand(page, "get_patronus_robot_status", () => canceled ? "canceled" : "running")
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        canceled = true
        return { ...RUNNING, status: "canceling" }
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      const toast = await pushIntoMovedRemote(page, "Cancel, Push and Restart")
      await expect(toast).toContainText("The Safe Push is canceled: the push restarts it.")
      expect(order).toEqual(["cancel"])

      await toast.getByRole("button", { name: "Force Push" }).click()
      await confirmForcePush(page, "Force Push (3)")

      // canceled now: no question, and the restart takes the pushed commit
      await expect(page.getByText("Safe Push restarted")).toBeVisible()
      expect(order).toEqual(["cancel", "restart"])
      expect(pushes.map(push => push.expectedRemote)).toEqual([null, "moved-tip"])
      await expect(page.getByTestId("push-guard-dialog")).toHaveCount(0)
    })
  })
})
