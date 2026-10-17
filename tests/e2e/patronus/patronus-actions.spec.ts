// @spec spec/branch/patronus.spec.md
import type { Page } from "@playwright/test"
import type { PatronusMergeRequest, PatronusReviewRequirement, PatronusRobot, PatronusRobotDetails, PatronusStartOptions, PullRequest } from "../../../src/lib/bindings"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot, setupRobots } from "../helpers/patronus-helpers"
import {
  availablePullRequests,
  createdPullRequest,
  createPullRequestDialog,
  getOpenedUrls,
  pullRequest,
  routeNewPullRequest,
  unavailablePullRequests,
} from "../helpers/pull-request-helpers"
import type { PushResult } from "../helpers/push-helpers"
import { confirmForcePush, pushedToast, pushOutcome, routeHeldPush, routePush } from "../helpers/push-helpers"
import { hoverStatusChip, openBranchPreview, pushBranchFromRow } from "../helpers/review-status-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { rewriteNextRemoteStatus, syncFromMenu } from "../helpers/sync-helpers"
import { routeCommand } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot")
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })
/** A Safe Push of JIRA-456 running on a commit the branch moved past */
const OLDER = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [
    { id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [] },
    { id: "dry-only", name: "Dry Only", description: null, allowedPushModes: ["dryRun"] },
  ],
  defaultProfileId: null,
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

/** The Safe Push form of JIRA-456, opened from its branch menu */
async function openSafePush(page: Page) {
  await findBranchRow(page, "JIRA-456").click({ button: "right" })
  await page.getByRole("menuitem", { name: "Safe Push…" }).click()
  return page.getByTestId("patronus-start-form")
}

/**
 * Answers the commands of a start that replaces the running robot `older` of JIRA-456 (its cancel, the push, a restart
 * and the new start), and records their order
 */
async function recordReplacingStart(page: Page, older: PatronusRobot) {
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
  return { order, starts }
}

/** Push and Restart Safe Push of the running robot of JIRA-456, from its menu: the one push into a branch whose Safe Push runs */
async function pushAndRestartFromMenu(page: Page) {
  await findBranchRow(page, "JIRA-456").click({ button: "right" })
  await page.getByRole("menuitem", { name: "Push and Restart Safe Push" }).click()
}

test.describe("Patronus actions", () => {
  test("should start a Safe Push from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const { pushes, release } = await routeHeldPush(page, "JIRA-456", 45)
    const options = await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }) satisfies PatronusRobot)

    const form = await openSafePush(page)
    await expect(form.getByTestId("patronus-start-push-note")).toHaveText("JIRA-456 isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.")
    await form.getByTestId("patronus-profile").click()
    // a profile that allows only dry runs is not offered for a Safe Push
    await expect(page.getByRole("option", { name: "Dry Only" })).toHaveCount(0)
    await page.getByRole("option", { name: "Large Scale Change" }).click()
    await form.getByRole("button", { name: "Push and Start Safe Push" }).click()
    // the button clicked shows the push, its first step; the push button of the row above doesn't repeat it
    await expect(form.getByRole("button", { name: "Pushing 45%" })).toHaveAttribute("aria-busy", "true")
    const rowPush = getPushButton(findBranchRow(page, "JIRA-456"))
    await expect(rowPush).toBeDisabled()
    await expect(rowPush.getByRole("status")).toHaveCount(0)
    release()

    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
    expect(options[0]).toMatchObject({ baselineBranch: "origin/master", lastRobotId: null })
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash", profileId: "lsc" })])

    // the toast names the branch, the target and the profile, never the ref the robot runs on, and opens the robot
    const started = page.locator("[data-slot=toast]", { hasText: "Safe Push started" })
    await expect(started.locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master · profile Large Scale Change")
    // the push is a step of the start: one action, one toast
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    await started.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://patronus.labs.jb.gg/robot/started-robot"])
  })

  test("should start a branch whose remote branch has its commits under other hashes without a push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    // the remote branch has the commits of JIRA-456 by content, at another commit: the sync rebuilt them on a newer baseline
    await pushBranchFromRow(page, "JIRA-456")
    const pushes = await routePush(page, {})
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }))
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)

    const form = await openSafePush(page)
    // Patronus rebases the remote branch itself: a push would only replace it with copies of the same commits
    await expect(form.getByTestId("patronus-start-no-push-note")).toHaveText(
      "The remote branch of JIRA-456 has the same commits, only on another baseline: nothing is pushed, Patronus starts from it.",
    )
    await expect(form.getByTestId("patronus-start-push-note")).toHaveCount(0)
    await form.getByRole("button", { name: "Start Safe Push", exact: true }).click()

    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    expect(pushes).toEqual([])
  })

  test('should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible', async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const form = page.getByTestId("patronus-start-form")

    // a robot runs on the commit JIRA-456 is at: the slot is the robot, the menu says why a start is not possible
    await expect(chip(page, "JIRA-456")).toHaveAttribute("data-kind", "running")
    await expect(findBranchRow(page, "JIRA-456").getByRole("button", { name: "Safe Push…" })).toHaveCount(0)

    const row = findBranchRow(page, "ABC-123")
    const button = row.getByRole("button", { name: "Safe Push…" })
    await button.hover()
    // S opens the form from the focused row
    const description = page.getByRole("tooltip").filter({ hasText: "code-owner review" })
    await expect(description).toContainText("Checks the code-owner review, then starts the robot.")
    await expect(description.locator("kbd")).toHaveText("S")
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
    // the approvals go into the GitHub pull request: no Space merge request is created (deprecated)
    await expect(rowMenu.getByRole("menuitem", { name: "Create Space Merge Request…" })).toHaveCount(0)
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

  test("should open the Safe Push form from the branch preview with S, where its button is enabled", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const form = page.getByTestId("patronus-start-form")

    // a robot runs on the commit JIRA-456 is at: the button is disabled, S opens nothing
    let preview = await openBranchPreview(page, "JIRA-456")
    await expect(preview.getByTestId("safe-push-button")).toBeDisabled()
    await page.keyboard.press("s")
    await expect(form).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)

    preview = await openBranchPreview(page, "ABC-123")
    await expect(preview.getByTestId("safe-push-button").locator("kbd")).toHaveText("S")
    await page.keyboard.press("s")
    // the form is a form of the row: the preview closes for it
    await expect(preview).toHaveCount(0)
    await expect(form.getByTestId("patronus-start-explanation")).toContainText("Patronus rebases ABC-123 onto master")
  })

  test("should cancel the robot for a new Safe Push from the form, and not restart it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot runs on an older commit, so the form can start one on the branch as it is
    const older = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const { order, starts } = await recordReplacingStart(page, older)

    await openSafePush(page)
    // the form says that its start replaces the running robot (before the start: the form closes once it started)
    await expect(page.getByTestId("patronus-start-replaces")).toHaveText("A Safe Push of JIRA-456 runs on older-co: the start cancels it.")
    await page.getByTestId("patronus-start-form").getByRole("button", { name: "Push and Start Safe Push" }).click()

    // and asks nothing more: no restart of the old one
    await expect.poll(() => order).toEqual(["cancel", "push", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should cancel the robot it replaces without a push when the remote branch has the commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a Safe Push of an older commit runs; the remote branch has the commits of JIRA-456, rebuilt since on a newer baseline
    const older = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === "JIRA-456" ? { ...status, remoteExists: true, remoteTip: "JIRA-456-pushed", unpushedCommits: [], myUnpushedCount: 0, fastForward: false } : status,
    )
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const { order, starts } = await recordReplacingStart(page, older)

    const form = await openSafePush(page)
    await expect(form.getByTestId("patronus-start-replaces")).toHaveText("A Safe Push of JIRA-456 runs on older-co: the start cancels it.")
    await expect(form.getByTestId("patronus-start-no-push-note")).toBeVisible()
    await form.getByRole("button", { name: "Start Safe Push", exact: true }).click()
    // no push: the start cancels the robot itself, and never restarts it
    await expect.poll(() => order).toEqual(["cancel", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
  })

  test("should wait to start until the remote status of the branch is loaded", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the first sync brings no remote status of JIRA-456
    await rewriteNextRemoteStatus(page, status => (status.branchName === "JIRA-456" ? null : status))
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)

    const form = await openSafePush(page)
    // nothing is claimed about the remote while it is unknown, and Start waits
    await expect(form.getByTestId("patronus-start-remote-loading")).toHaveText("Comparing JIRA-456 with its remote branch…")
    await expect(form.getByTestId("patronus-start-push-note")).toHaveCount(0)
    await expect(form.getByRole("button", { name: "Start Safe Push", exact: true })).toBeDisabled()

    await syncFromMenu(page)
    await expect(form.getByTestId("patronus-start-remote-loading")).toHaveCount(0)
    await expect(form.getByTestId("patronus-start-push-note")).toHaveText("JIRA-456 isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.")
    await expect(form.getByRole("button", { name: "Push and Start Safe Push" })).toBeEnabled()
  })

  test("should restart a failed robot and cancel a running one from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))

    let popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Cancel Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Restart Safe Push" }).click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await page.keyboard.press("Escape")

    popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])

    // each toast names the branch and the target, and opens its robot
    const restarted = page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })
    await expect(restarted.locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
    await expect(restarted.getByRole("button", { name: "Open in Patronus" })).toHaveCount(1)
    const canceling = page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })
    await expect(canceling.locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
  })

  test("should push and restart a failed robot whose branch changed since, from the branch preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // failed on an older commit; the fix is not on the remote yet
    const older = robot("ABC-123", "failed-robot", { status: "failure", sourceRevision: "older-commit", finishTime: 1704142800 })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const order: string[] = []
    await routeCommand(page, "push_branch", params => {
      order.push("push")
      return pushOutcome(params.branchName as string)
    })
    await routeCommand(page, "restart_patronus_robot", params => {
      order.push(`restart ${params.robotId as string}`)
      return robot("ABC-123", "restarted-robot")
    })

    // the row offers a new Safe Push; the preview has the robot, with the restart that tests the branch as it is
    await expect(chip(page, "ABC-123")).toHaveCount(0)
    const preview = await openBranchPreview(page, "ABC-123")
    const details = preview.getByTestId("branch-preview-patronus")
    await expect(details.getByTestId("patronus-outdated")).toContainText("Push and Restart Safe Push to test the current branch.")
    await details.getByRole("button", { name: "Push and Restart Safe Push" }).click()

    // the restart takes the commit just pushed; it is the action, its toast the one toast
    await expect(page.locator("[data-slot=toast-title]", { hasText: "Safe Push restarted" })).toBeVisible()
    expect(order).toEqual(["push", "restart failed-robot"])
    await expect(pushedToast(page, "ABC-123")).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should show the profile, the checks and the problems in the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
    await routeCommand(
      page,
      "get_patronus_robot_details",
      () =>
        ({
          robot: { ...FAILED, sourceRevision: null },
          profileId: "lsc",
          profileName: "Large Scale Change",
          checks: { failed: 1, running: 0, passed: 9, canceled: 0, failedNames: ["Fast Tests"] },
          problems: [{ title: "Tests failed", subsystem: "TEAMCITY" }],
          failing: false,
          progress: null,
        }) satisfies PatronusRobotDetails,
    )

    const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByTestId("patronus-profile-name")).toHaveText("Large Scale Change")
    await expect(popover.getByTestId("patronus-checks")).toHaveText("1 failing (Fast Tests), 9 passed")
    await expect(popover.getByTestId("patronus-problems")).toHaveText("Tests failed")
  })

  /** One group must approve, without the files it reviews: Patronus doesn't say, no OWNERSHIP file names it */
  const REVIEW_REQUIRED: PatronusReviewRequirement = {
    required: true,
    mergeRequest: null,
    groups: [
      {
        name: "Air Plugin Release Team",
        reviewers: [{ fullName: "Alice", username: "alice", githubLogin: null, available: true, suggested: true }],
        paths: [],
        branchWide: false,
        pathsSource: "unknown",
      },
    ],
    skipped: false,
  }

  test.describe("code-owner review", () => {
    const MERGE_REQUEST: PatronusMergeRequest = { projectKey: "IJ", reviewKey: "IJ-CR-7", url: "https://jetbrains.team/p/ij/reviews/7" }

    test("should list the files of each group and review them in the window beside the form", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(
        page,
        "get_patronus_review_requirement",
        () =>
          ({
            ...REVIEW_REQUIRED,
            groups: [
              { ...REVIEW_REQUIRED.groups[0]!, paths: ["plugins/air/A.kt", "plugins/air/B.kt"], pathsSource: "patronus" },
              { name: "Bazel QA", reviewers: [], paths: [], branchWide: true, pathsSource: "patronus" },
              { name: "Delegated Team", reviewers: [], paths: [], branchWide: false, pathsSource: "unknown" },
            ],
          }) satisfies PatronusReviewRequirement,
      )

      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Safe Push…" }).click()
      const review = page.getByTestId("patronus-review")
      await expect(review.getByTestId("patronus-review-group")).toHaveText([
        "Air Plugin Release Team (suggested: Alice)· 2 files",
        "Bazel QA· all files",
        // Patronus doesn't tell, and no owner has its name
        "Delegated Team",
      ])
      await review.getByTestId("patronus-review-group-toggle").click()
      const files = review.getByTestId("patronus-review-group-files")
      await expect(files.getByRole("button")).toHaveText(["plugins/air/A.kt", "plugins/air/B.kt"])

      // a file reviews it in the review window, with what Patronus checks: the whole stack, the files of the group; the
      // form stays in the main window
      await files.getByRole("button", { name: "plugins/air/B.kt" }).click()
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
      await expect(review).toBeVisible()
      // all the changes, the groups of the check for its chips
      await review.getByRole("button", { name: "Review Changes" }).click()
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(2)
      const [ofFile, ofAll] = await getOpenedSubWindows<BranchReviewWindowData>(page)
      expect(ofFile).toMatchObject({
        windowId: "branch-review",
        data: { focus: { range: "all", filter: { kind: "group", name: "Air Plugin Release Team" }, path: "plugins/air/B.kt" } },
      })
      expect(ofFile!.data.groups.map(group => group.name)).toContain("Bazel QA")
      expect(ofAll!.data.focus).toMatchObject({ range: "all", filter: { kind: "all" }, path: null })
      await expect(review).toBeVisible()

      // the window of the file: the template has none of the paths Patronus names here, the filter says so
      await openSubWindowPage(page, ofFile!)
      const window = page.getByTestId("branch-review")
      await expect(window.getByTestId("branch-review-owner").filter({ hasText: "Air Plugin Release Team" })).toHaveAttribute("aria-pressed", "true")
      await expect(window).toContainText("No files of Air Plugin Release Team in this range")
      await expect(window.getByTestId("branch-review-owner").filter({ hasText: "Bazel QA" })).toBeVisible()
    })

    test("should name the missing approvals and the GitHub pull request they count in, and start with a merge request found open", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", params =>
        params.recheck === true ? { ...REVIEW_REQUIRED, required: false, groups: [], mergeRequest: MERGE_REQUEST } : REVIEW_REQUIRED,
      )
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }))

      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Safe Push…" }).click()
      const review = page.getByTestId("patronus-review")
      await expect(review).toHaveAttribute("data-status", "required")
      await expect(review.getByTestId("patronus-review-groups")).toHaveText("Air Plugin Release Team (suggested: Alice)")

      // the block names the system the approvals count in, and how to make it known; a Space merge request is never created
      await expect(review.getByTestId("patronus-review-github-unknown")).toHaveText(
        "Patronus counts code-owner approvals in the GitHub pull request of the branch. The GitHub repository of the branches is unknown: run git config branchdeck.githubRepository <owner>/<name>.",
      )
      await expect(review.getByRole("button", { name: /Merge Request/ })).toHaveCount(0)

      // a merge request opened elsewhere still counts: Recheck finds it approved, and the start links it
      await review.getByRole("button", { name: "Recheck" }).click()
      await expect(review).toHaveAttribute("data-status", "approved")
      await expect(review).toHaveText("Approved in IJ-CR-7.")
      expect(checks.map(params => params.recheck)).toEqual([false, true])

      await page
        .getByTestId("patronus-start-form")
        .getByRole("button", { name: /Start Safe Push/ })
        .click()
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      // the branch isn't on the remote yet: the start pushes it first, without a toast of its own
      expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
      await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
      expect(starts).toEqual([expect.objectContaining({ mergeRequest: MERGE_REQUEST })])
    })

    test("should not check the review of a dry run", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", () => REVIEW_REQUIRED)
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Dry Run…" }).click()
      await expect(page.getByTestId("patronus-start-explanation")).toContainText("without pushing into master")
      await expect(page.getByTestId("patronus-review")).toHaveCount(0)
      expect(checks).toHaveLength(0)
    })

    test('should say "Needs review" for a robot that failed for missing approvals, and offer a new Safe Push', async ({ page, setupRepo, syncAndWaitForBranches }) => {
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

  test.describe("branches reviewed in GitHub pull requests", () => {
    const NO_REVIEW: PatronusReviewRequirement = { required: false, mergeRequest: null, groups: [], skipped: false }

    /** issue_links mirrored to the GitHub repository `acme/monorepo`, with these pull requests */
    async function setupGitHub(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<unknown>,
      syncAndWaitForBranches: () => Promise<unknown>,
      ...pullRequests: PullRequest[]
    ) {
      await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(...pullRequests) })
      await syncAndWaitForBranches()
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      for (const it of pullRequests) {
        await expect(findBranchRow(page, it.headRefName.split("/").pop()!).getByTestId("pull-request-chip")).toBeVisible()
      }
    }

    /** JIRA-456 with the open pull request #7 and no review needed; its push is answered in the page */
    async function setupPullRequestStart(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<unknown>,
      syncAndWaitForBranches: () => Promise<unknown>,
      patch: Partial<PullRequest> = {},
    ) {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, patch))
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(page, "get_patronus_review_requirement", () => NO_REVIEW)
    }

    /**
     * Runs the fake clock over the app's wait before it reads the bot's reply to the posted comment
     * (`COMMAND_REPLY_INTERVAL_MS`, 3 s, in src/stores/patronus.ts) until it read it, instead of waiting it out
     */
    async function skipReplyWait(page: Page, posts: unknown[], replies: unknown[]) {
      await expect.poll(() => posts.length).toBe(1)
      // the wait starts once the app has the posted comment, so a step can come before it: step until the read
      await expect
        .poll(async () => {
          await page.clock.fastForward(3000)
          return replies.length
        })
        .toBeGreaterThan(0)
    }

    test("should start a Safe Push from the pull request with a /patronus merge comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "4402154133", url: "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133" }))
      const replies = await routeCommand(page, "read_patronus_command_reply", () => ({ status: "accepted" }))
      const started = robot("JIRA-456", "pr-robot", {
        sourceRevision: null,
        startTime: Math.floor(Date.now() / 1000),
        pullRequest: { number: 7, url: "https://github.com/acme/monorepo/pull/7" },
      })
      // the robot the comment started is in the list asked for after the 👍
      const lists = await routeCommand(page, "get_patronus_robots", params => (params.since === null ? availableRobots() : availableRobots(started)))
      const rest = await routeCommand(page, "start_patronus_robot", () => started)

      const form = await openSafePush(page)
      await expect(form.getByTestId("patronus-start-explanation")).toContainText("Comments /patronus merge on pull request #7:")
      await expect(form.getByTestId("patronus-start-profile-note")).toBeVisible()
      await expect(form.getByTestId("patronus-profile")).toHaveCount(0)
      await form.getByRole("button", { name: "Push and Start Safe Push" }).click()
      await skipReplyWait(page, posts, replies)

      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      expect(posts).toEqual([expect.objectContaining({ pullRequestNumber: 7, mode: "rebaseAutosquash" })])
      expect(replies).toEqual([expect.objectContaining({ pullRequestNumber: 7, commentId: "4402154133" })])
      expect(lists.at(-1)?.since).not.toBeNull()
      expect(rest).toHaveLength(0)

      const popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
      await popover.getByRole("button", { name: "Open Pull Request #7" }).click()
      expect(await getOpenedUrls(page)).toContain("https://github.com/acme/monorepo/pull/7")
    })

    test("should say why Patronus refused the comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))
      const replies = await routeCommand(page, "read_patronus_command_reply", () => ({
        status: "rejected",
        reason: "Patronus did not start the robot due to an error. A robot is already running for the branch.",
      }))

      const form = await openSafePush(page)
      await form.getByRole("button", { name: "Push and Start Safe Push" }).click()
      await skipReplyWait(page, posts, replies)
      const error = page.locator("[data-slot=toast]", { hasText: "Cannot start the Safe Push" })
      await expect(error).toContainText("A robot is already running for the branch.")
      await expect(chip(page, "JIRA-456")).toHaveCount(0)
    })

    test("should warn before a push that dismisses the approvals of the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches, { reviewers: [{ login: "alice", state: "approved" }] })
      const rules = await routeCommand(page, "get_branch_rules", () => ({ dismissStaleReviewsOnPush: true }))

      const form = await openSafePush(page)
      await expect(form.getByTestId("patronus-start-dismisses-approvals")).toHaveText(
        "The push dismisses the approvals of #7: the rules of the repository dismiss stale reviews when new commits are pushed.",
      )
      await expect(form.getByRole("button", { name: "Push and Start Safe Push" })).toBeEnabled()
      expect(rules).toEqual([expect.objectContaining({ branch: "master" })])
    })

    test("should not merge a draft pull request, and count the approvals in it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { isDraft: true, reviewers: [{ login: "alice", state: "approved" }] }))
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(page, "get_patronus_review_requirement", () => REVIEW_REQUIRED)

      const form = await openSafePush(page)
      await expect(form.getByTestId("patronus-start-draft")).toHaveText("Pull request #7 is a draft: Patronus merges it once it is ready for review.")
      await expect(form.getByRole("button", { name: /Start Safe Push/ })).toBeDisabled()

      const review = form.getByTestId("patronus-review")
      await expect(review).toHaveAttribute("data-status", "required")
      await expect(review.getByTestId("patronus-review-groups")).toHaveText("Air Plugin Release Team (suggested: Alice)")
      await expect(review.getByTestId("patronus-review-github-approvals")).toHaveText("Approved on GitHub by alice; the Safe Push counts them, the check before it can't.")
      await expect(review).toContainText("Patronus counts code-owner approvals in pull request #7.")
      await review.getByRole("button", { name: "Open Pull Request #7" }).click()
      expect(await getOpenedUrls(page)).toEqual([pullRequest("JIRA-456", 7).url])
    })

    test("should offer to create the pull request the approvals count in, and start a branch without one through the API", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches)
      const { pushes, release } = await routeHeldPush(page, "JIRA-456", 45)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(page, "get_patronus_review_requirement", () => REVIEW_REQUIRED)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }))

      await routeNewPullRequest(page)
      const form = await openSafePush(page)
      const review = form.getByTestId("patronus-review")
      await expect(review).toContainText("Patronus counts code-owner approvals in the pull request of the branch; the Safe Push starts from it.")
      await review.getByRole("button", { name: "Create Pull Request…" }).click()
      // one indicator of its push: the button clicked; the start waits without a spinner, as the row's push
      await expect(review.getByTestId("patronus-create-pull-request")).toHaveText("Pushing 45%")
      await expect(form.getByRole("button", { name: /Start Safe Push/ })).toBeDisabled()
      await expect(form.getByRole("button", { name: /Start Safe Push/ }).getByRole("status")).toHaveCount(0)
      await expect(getPushButton(findBranchRow(page, "JIRA-456")).getByRole("status")).toHaveCount(0)
      release()
      // pushed first, without a toast of its own (it would offer the dialog that is opening): GitHub opens a pull request of a pushed branch
      const dialog = createPullRequestDialog(page)
      await expect(dialog).toBeVisible()
      expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
      await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
      // Patronus names the code owner without a GitHub login: the user finds the person
      const suggestions = dialog.getByTestId("create-pull-request-suggestions")
      await expect(suggestions.getByRole("button")).toHaveText(["Alice"])
      const searches = await routeCommand(page, "search_github_users", () => [{ login: "alice-gh", name: "Alice" }])
      await suggestions.getByRole("button", { name: "Alice" }).click()
      await expect(dialog.getByTestId("create-pull-request-reviewer-search")).toHaveValue("Alice")
      await page.getByRole("option", { name: /alice-gh/ }).click()
      expect(searches.at(-1)).toEqual(expect.objectContaining({ query: "Alice" }))
      await page.keyboard.press("Escape")
      await expect(dialog).toHaveCount(0)
      expect(await getOpenedUrls(page)).toEqual([])

      await form.getByRole("button", { name: /Start Safe Push/ }).click()
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mergeRequest: null })])
    })

    test.describe("the pull request the approvals count in", () => {
      const CODE_OWNERS: PatronusReviewRequirement = {
        ...REVIEW_REQUIRED,
        groups: [{ ...REVIEW_REQUIRED.groups[0]!, reviewers: [{ fullName: "Alice", username: "alice", githubLogin: "alice-gh", available: true, suggested: true }] }],
      }

      test("should ask the code owner Patronus names, and the Safe Push form then starts from the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
        await syncAndWaitForBranches()
        await routePush(page, {})
        await routeNewPullRequest(page)
        await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
        await routeCommand(page, "get_patronus_review_requirement", () => CODE_OWNERS)
        const created = pullRequest("JIRA-456", 7, { reviewers: [{ login: "alice-gh", state: "requested" }] })
        const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(created))

        const form = await openSafePush(page)
        await form.getByTestId("patronus-review").getByRole("button", { name: "Create Pull Request…" }).click()
        const dialog = createPullRequestDialog(page)
        // one suggested reviewer of each group is chosen at once
        await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["alice-gh"])
        await dialog.getByTestId("create-pull-request-submit").click()
        await expect(dialog).toHaveCount(0)
        expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", reviewers: ["alice-gh"], assignSelf: true })])
        // the approvals count in it: the form starts from it with a comment
        await expect(form.getByTestId("patronus-review")).toContainText("pull request #7")
      })

      test("should open GitHub's form without a token GitHub accepts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: unavailablePullRequests({ status: "signedOut" }) })
        await syncAndWaitForBranches()
        await routePush(page, {})
        await routeNewPullRequest(page, { viewerLogin: null })
        await getPushButton(findBranchRow(page, "JIRA-456")).click()
        await pushedToast(page, "JIRA-456").getByRole("button", { name: "Create Pull Request…" }).click()
        const dialog = createPullRequestDialog(page)
        await expect(dialog.getByTestId("create-pull-request-unavailable")).toContainText("No GitHub token to see pull requests: GitHub's form opens the pull request instead.")
        await expect(dialog.getByTestId("create-pull-request-submit")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-reviewers")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-assign")).toHaveCount(0)
        await dialog.getByTestId("create-pull-request-on-github").click()
        await expect(dialog).toHaveCount(0)
        expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/compare/master...user-name/virtual/JIRA-456?expand=1&title=Add+auth&body=-+**Add+login**"])
      })
    })
  })

  test.describe("pushing into a branch whose Safe Push runs", () => {
    test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // the robot runs on an older commit: the branch changed since, Push and Restart tests it as it is
      await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
    })

    test("should offer no push: it won't land, and the menu says why", async ({ page }) => {
      const pushes = await routeCommand(page, "push_branch", params => pushOutcome(params.branchName as string))
      const row = findBranchRow(page, "JIRA-456")
      // the place of the push stays empty, as the Safe Push slot's does; the Status cell shows the robot
      await expect(getPushButton(row)).toHaveCount(0)
      await expect(row.getByTestId("push-placeholder")).toHaveCount(1)
      await expect(row.getByTestId("safe-push-slot")).toHaveAttribute("data-step", "none")
      // P does nothing on the row
      await row.focus()
      await page.keyboard.press("p")

      await row.getByRole("button", { name: "More actions for JIRA-456" }).click()
      const push = page.getByRole("menu", { name: "More actions for JIRA-456" }).getByRole("menuitem", { name: "Push P" })
      await expect(push).toBeDisabled()
      await push.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "is running" })).toHaveText("A Safe Push of JIRA-456 is running: wait until it merges, or push and restart it")
      await page.keyboard.press("Escape")
      expect(pushes).toHaveLength(0)
    })

    test("should cancel the robot, push and restart it on the pushed commit, from Push and Restart Safe Push", async ({ page }) => {
      const order: string[] = []
      await routeCommand(page, "get_patronus_robot_status", () => "running")
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        return { ...OLDER, status: "canceling" }
      })
      await routeCommand(page, "push_branch", params => {
        order.push("push")
        return pushOutcome(params.branchName as string)
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })

      await pushAndRestartFromMenu(page)
      // the restart is the action: one toast says it, and opens the restarted robot
      const toast = page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })
      await expect(toast).toBeVisible()
      await expect.poll(() => order).toEqual(["cancel", "push", "restart"])
      await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
      await toast.getByRole("button", { name: "Open in Patronus" }).click()
      expect(await getOpenedUrls(page)).toEqual([expect.stringContaining("restarted-robot")])
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    })

    test("should restart a robot canceled for a push the remote refused once the toast pushes", async ({ page }) => {
      const order: string[] = []
      let canceled = false
      await routeCommand(page, "get_patronus_robot_status", () => (canceled ? "canceled" : "running"))
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        canceled = true
        return { ...OLDER, status: "canceling" }
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      await pushAndRestartFromMenu(page)
      const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
      await expect(toast).toContainText("The Safe Push is canceled: the push restarts it.")
      expect(order).toEqual(["cancel"])

      await toast.getByRole("button", { name: "Force Push" }).click()
      await confirmForcePush(page, "Force Push (3)")

      // canceled now: the restart takes the pushed commit, in the toast of the push
      await expect(pushedToast(page, "JIRA-456").locator("[data-slot=toast-description]")).toHaveText(/ · Safe Push restarted$/)
      expect(order).toEqual(["cancel", "restart"])
      expect(pushes.map(push => push.expectedRemote)).toEqual([null, "moved-tip"])
    })
  })
})
