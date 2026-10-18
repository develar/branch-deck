// @spec spec/branch/whole-stack.spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusReviewRequirement, PatronusRobot, PullRequest, PullRequestsResult } from "../../../src/lib/bindings"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { AWAITING_CODE_OWNER, NOT_EVALUATED, NO_REVIEW_NEEDED, availableRobots, robot, routeReviewRequirement } from "../helpers/patronus-helpers"
import { availablePullRequests, createdPullRequest, createPullRequestDialog, getOpenedUrls, pullRequest, reviewer } from "../helpers/pull-request-helpers"
import { pushOutcome, routeHeldPush, routePushBranches, wholeStackResultKey } from "../helpers/push-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { DEFAULT_CHECKS_ONLY, expectLandedActions, popoverStart, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { emitTauriEvent, rewriteNextRemoteStatus, rewriteNextSyncEvents } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommand } from "../helpers/wait-helpers"

/** The island of the stack on `root` */
function island(page: Page, root: string): Locator {
  return page.locator(`[data-testid="stack-island"][data-stack-root="${root}"]`)
}

/** Opens the "⋯" of the island of `root` */
async function openIslandMenu(page: Page, root: string): Promise<void> {
  await island(page, root)
    .getByRole("button", { name: `More actions for the ${root} stack` })
    .click()
}

/** Opens "Whole Stack ›" of the root's menu (the right-click on its row) */
async function openWholeStackSubmenu(page: Page, root: string): Promise<void> {
  await findBranchRow(page, root).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Whole Stack" }).click()
}

/** The open pull request of the stack branch of `root`; its empty head is the stack branch the sync built */
function stackPullRequest(root: string, number: number, patch: Partial<PullRequest> = {}): PullRequest {
  return pullRequest(root, number, { title: `The whole ${root} stack`, headRefName: `user-name/stack/${root}`, ...patch })
}

function withStackPullRequests(result: PullRequestsResult, stacks: Record<string, PullRequest>): PullRequestsResult {
  return { ...result, stackPullRequests: stacks }
}

/**
 * The next sync reports the stack branch of `root` on the remote at `stack-pushed`, rebuilt with a commit that remote
 * lacks: a pull request or a Safe Push put it there
 */
async function stackBranchAheadOfRemote(page: Page, root: string): Promise<void> {
  const remote = pushOutcome(root, { unpushedCommits: ["s1"], remoteTip: "stack-pushed" }).remoteStatus
  await rewriteNextSyncEvents(page, event => (event.type === "stackBranchUpdate" && event.data.root === root ? { ...event, data: { ...event.data, remote } } : event))
}

/** The push of the header of the island of `root` (Push with Children, and the push of its stack branch alone) */
function islandPush(page: Page, root: string): Locator {
  return island(page, root).getByTestId("stack-island-push")
}

/** A running Safe Push of the whole stack of `root`, on its stack branch as the sync built it */
function wholeStackRobot(root: string, id: string, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return robot(root, id, { name: `Safe Push of the whole ${root} stack`, branchName: null, stackRoot: root, ...patch })
}

/**
 * stack_island reviewed on GitHub, synced: Patronus asked about the stack branch says IJPL Code Platform has to approve
 * the whole stack, in a pull request it doesn't have yet. Returns the slot of the header, whose step creates it
 */
async function setupAwaitingCodeOwner(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<void>,
): Promise<Locator> {
  const repo = await setupRepo("stack_island", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
  await repo.setReviewAnswers({ answers: { "stack:air": AWAITING_CODE_OWNER } })
  await syncAndWaitForBranches()
  const slot = island(page, "air").getByTestId("stack-island-actions").getByTestId("next-step-slot")
  await expect(slot).toHaveAttribute("data-step", "createPullRequest")
  return slot
}

/**
 * stack_island reviewed on GitHub, air and bazel on the remote, the pull request #12 of the whole stack open (approved by
 * alice), Patronus answering `answer` for the stack branch (null: no code-owner review needed): the popover of the
 * Safe Push of the whole stack, opened by S on the header
 */
async function openWholeStackPopoverWithPullRequest(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<void>,
  answer: PatronusReviewRequirement | null,
): Promise<Locator> {
  await rewriteNextRemoteStatus(page, status => (status.branchName === "air" || status.branchName === "bazel" ? pushOutcome(status.branchName).remoteStatus : status))
  const stack = stackPullRequest("air", 12, { reviewers: [reviewer("alice", "approved")] })
  const repo = await setupRepo("stack_island", {
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    pullRequests: withStackPullRequests(availablePullRequests(), { air: stack }),
  })
  if (answer !== null) {
    await repo.setReviewAnswers({ answers: { "stack:air": answer } })
  }
  await syncAndWaitForBranches()
  await routeCommand(page, "get_patronus_start_options", () => DEFAULT_CHECKS_ONLY)
  await island(page, "air")
    .getByRole("button", { name: /^air Stack · 7 branches/ })
    .focus()
  await page.keyboard.press("s")
  const popover = safePushPopover(page)
  await expect(popover).toHaveAttribute("aria-label", "Safe Push of the whole air stack")
  return popover
}

test.describe("Whole stack", () => {
  test("should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stack_island", { patronus: availableRobots(), pullRequests: availablePullRequests() })
    await syncAndWaitForBranches()
    const { stackPushes, release } = await routeHeldPush(page, wholeStackResultKey("air"), 45)
    const created = stackPullRequest("air", 12)
    const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(created))
    const dialog = createPullRequestDialog(page)
    // the menus are built on open: Patronus and GitHub answered once a branch's menu offers its pull request
    await expect(island(page, "air").getByTestId("stack-island-actions").getByTestId("safe-push-button")).toBeVisible()
    await findBranchRow(page, "docs").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Create Pull Request…" })).toBeVisible()
    await page.keyboard.press("Escape")

    // the island's "⋯": the dialog of the stack branch, its text written from the branches of the stack
    await openIslandMenu(page, "air")
    await expect(page.getByRole("menu", { name: "More actions for the air stack" }).getByRole("menuitem")).toHaveText([
      "Safe Push Whole Stack",
      "Dry Run Whole Stack",
      "Create Pull Request of the Stack…",
      "Push with Children (7 branches)",
      "Review Whole Stack",
      "Collapse Stack",
    ])
    await page.getByRole("menuitem", { name: "Create Pull Request of the Stack…" }).click()
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole air stack → master")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveText(
      "Holds the commits of every branch of the stack: air, bazel, bridge and 4 more: 7 branches · 14 commits",
    )
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("air, with the 6 branches built on it")
    await expect(dialog.getByTestId("create-pull-request-body")).toHaveValue(/^The whole air stack in one Safe Push into master: 7 branches · 14 commits\./)
    await page.keyboard.press("Escape")
    await expect(dialog).toHaveCount(0)
    expect(stackPushes).toEqual([])

    // the root's menu: the same action under "Whole Stack"; Create pushes the stack branch alone, then creates it
    await openWholeStackSubmenu(page, "air")
    await page.getByRole("menuitem", { name: "Create Pull Request of the Stack…" }).click()
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole air stack → master")
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("air, with the 6 branches built on it")
    const submit = dialog.getByTestId("create-pull-request-submit")
    await submit.click()
    // the push of the stack branch it starts with shows in the button, as the push of a branch does
    await expect(submit).toContainText("Pushing 45%")
    expect(creates).toEqual([])
    release()
    await expect.poll(() => creates.length).toBe(1)
    expect(creates[0]).toMatchObject({ branchName: "air", head: "user-name/stack/air", base: "master", title: "air, with the 6 branches built on it", waitForMirror: true })
    expect(stackPushes).toEqual([[expect.objectContaining({ branchName: "air", wholeStack: true, expectedRemote: null })]])
    await expect(dialog).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Created pull request #12" })).toContainText("Whole air stack → master")

    // now the stack has it: "Open Pull Request #12" in both menus
    await openIslandMenu(page, "air")
    await page.getByRole("menuitem", { name: "Open Pull Request #12" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([created.url])
    await openWholeStackSubmenu(page, "air")
    await page.getByRole("menuitem", { name: "Open Pull Request #12" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([created.url, created.url])
  })

  test("should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const slot = await setupAwaitingCodeOwner(page, setupRepo, syncAndWaitForBranches)
    await routePushBranches(page, {})
    const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(stackPullRequest("air", 12)))

    // the step of the header opens the dialog of the stack branch, the code owner Patronus suggests asked to review
    await slot.getByTestId("next-step").click()
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole air stack → master")
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["ikorennoy"])
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect.poll(() => creates.length).toBe(1)
    expect(creates[0]).toMatchObject({ branchName: "air", head: "user-name/stack/air", reviewers: ["ikorennoy"] })
  })

  test("should show the push of the stack branch alone in the header's push when the restart of its robot pushes it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot of the whole stack failed on the stack branch the remote has; the sync rebuilt it since
    await stackBranchAheadOfRemote(page, "air")
    const failed = wholeStackRobot("air", "whole", { status: "failure", sourceRevision: "stack-pushed", finishTime: 1704142800 })
    await setupRepo("stack_island", { patronus: availableRobots(failed) })
    await syncAndWaitForBranches()
    const { stackPushes, release } = await routeHeldPush(page, wholeStackResultKey("air"), 45)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => wholeStackRobot("air", "restarted", { sourceRevision: "s1" }))
    const push = islandPush(page, "air")
    const rootPush = findBranchRow(page, "air").getByTestId("push-button")
    await expect(push).toBeEnabled()

    // the restart of its robot in the island's "⋯" pushes the stack branch first: the header's push shows it as the
    // pushes of the stack, and only there
    await openIslandMenu(page, "air")
    const restart = page.getByRole("menuitem", { name: "Restart Safe Push" })
    await restart.hover()
    // the tooltip of "⋯" may still be fading out: the item's by its text
    await expect(page.getByRole("tooltip", { name: "Pushes the whole air stack and starts it again on the pushed commit" })).toBeVisible()
    await restart.click()
    await expect(push).toHaveAccessibleName("Pushing the whole stack 45%")
    await expect(push).toHaveAttribute("aria-busy", "true")
    await expect(push).toBeDisabled()
    await push.hover()
    await expect(page.getByTestId("stack-island-push-description")).toHaveText("Writing objects:  45% (2/4)")
    await expect(rootPush).toBeVisible()
    await expect(rootPush).not.toHaveAttribute("aria-busy", "true")
    await page.mouse.move(0, 0)
    expect(restarts).toEqual([])

    release()
    await expect.poll(() => restarts.length).toBe(1)
    expect(stackPushes).toEqual([[expect.objectContaining({ branchName: "air", wholeStack: true, expectedRemote: "stack-pushed" })]])
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Safe Push restarted" })).toBeVisible()
    await expect(push).not.toHaveAttribute("aria-busy", "true")
  })

  test("should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the code owner of the whole stack has to approve in #12, and nobody asked him yet
    await stackBranchAheadOfRemote(page, "air")
    const repo = await setupRepo("stack_island", {
      patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
      pullRequests: withStackPullRequests(availablePullRequests(), { air: stackPullRequest("air", 12) }),
    })
    await repo.setReviewAnswers({ answers: { "stack:air": AWAITING_CODE_OWNER } })
    await syncAndWaitForBranches()
    const { stackPushes, release } = await routeHeldPush(page, wholeStackResultKey("air"), 45)
    const requests = await routeCommand(page, "request_pull_request_reviewers", () => ({ requested: ["ikorennoy"], refused: [] }))
    const slot = island(page, "air").getByTestId("stack-island-actions").getByTestId("next-step-slot")
    await expect(slot).toHaveAttribute("data-step", "requestReview")

    // the reviewers see the commits: the request pushes the stack branch first, its progress in the header's push
    await slot.getByTestId("next-step").click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #12")
    await expect(popover.getByTestId("request-review-candidate-ikorennoy")).toBeChecked()
    await popover.getByTestId("request-review-submit").click()
    const push = islandPush(page, "air")
    await expect(push).toHaveAccessibleName("Pushing the whole stack 45%")
    await expect(push).toHaveAttribute("aria-busy", "true")
    expect(requests).toEqual([])

    release()
    await expect(page.locator("[data-slot=toast]", { hasText: "Requested a review from Ilya Korennoy on #12" })).toBeVisible()
    expect(stackPushes).toEqual([[expect.objectContaining({ branchName: "air", wholeStack: true, expectedRemote: "stack-pushed" })]])
    expect(requests).toEqual([expect.objectContaining({ pullRequestNumber: 12, reviewers: ["ikorennoy"] })])
    await expect(push).not.toHaveAttribute("aria-busy", "true")
  })

  test("should say on S why the whole stack can't start without its pull request, and create it from the notice", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupAwaitingCodeOwner(page, setupRepo, syncAndWaitForBranches)

    // S on the focused header: the notice says where the approvals count, and its action creates that pull request
    await island(page, "air")
      .getByRole("button", { name: /^air Stack · 7 branches/ })
      .focus()
    await page.keyboard.press("s")
    const notice = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of the whole air stack needs a pull request" })
    await expect(notice.locator("[data-slot=toast-description]")).toHaveText("Code owners must approve in it: IJPL Code Platform")
    await notice.getByRole("button", { name: "Create Pull Request of the Stack…" }).click()
    await expect(createPullRequestDialog(page).getByTestId("create-pull-request-base")).toHaveText("Whole air stack → master")
  })

  test("should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const stack = stackPullRequest("air", 12)
    const repo = await setupRepo("stack_island", {
      patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
      pullRequests: withStackPullRequests(availablePullRequests(pullRequest("bazel", 5, { reviewDecision: "approved" })), { air: stack }),
    })
    // the code owner of bridge has to approve: in a pull request, and bridge has none of its own
    await repo.setReviewAnswers({ answers: { bridge: AWAITING_CODE_OWNER } })
    await syncAndWaitForBranches()

    // the header: where the approvals of the whole stack count, then the pull requests of the branches, counted apart,
    // " · " between them as between the parts of each
    const stackLine = island(page, "air").getByTestId("stack-island-stack-pull-request")
    const branchesLine = island(page, "air").getByTestId("stack-island-pull-requests")
    await expect(stackLine).toHaveText("Approvals count in #12, the pull request of the whole stack")
    await expect(branchesLine).toHaveText("1 pull request open · 1 approved")
    const separatorBefore = (line: Locator) => line.evaluate(element => getComputedStyle(element, "::before").content)
    expect(await separatorBefore(branchesLine)).toBe('"·"')
    expect(await separatorBefore(stackLine)).toBe("none")

    // bridge waits for approvals in a pull request: the one of the whole stack collects them for its Safe Push
    const slot = findBranchRow(page, "bridge").getByTestId("next-step-slot")
    await expect(slot).toHaveAttribute("data-step", "createPullRequest")
    const step = slot.getByTestId("next-step")
    await expect(step).toHaveText("In #12")
    await expect(step).toHaveAttribute("data-tone", "quiet")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(
      "Code owners must approve in a pull request: IJPL Code Platform\nApprovals in #12, the pull request of the whole air stack, count for its Safe Push only\nOpen Pull Request #12",
    )
    await step.click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([stack.url])

    // the branch's own Safe Push still needs approvals of its own: S says so, and where those of the whole stack count
    await findBranchRow(page, "bridge").click()
    await page.keyboard.press("s")
    const notice = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of bridge needs a pull request" })
    await expect(notice).toContainText(
      "Code owners must approve in it: IJPL Code Platform · Approvals in #12, the pull request of the whole air stack, count for its Safe Push only",
    )
  })

  test("should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const popover = await openWholeStackPopoverWithPullRequest(page, setupRepo, syncAndWaitForBranches, null)
    await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #12: Patronus reports there and closes it once merged.")
    await expect(popover.getByTestId("safe-push-review")).toHaveText("No code-owner review needed.")
    // no code owner has to approve: where approvals would count is no news
    await expect(popover.getByTestId("safe-push-whole-stack-approvals")).toHaveCount(0)
    await expect(popover.getByTestId("safe-push-stack-branch")).toHaveText("Pushes only its stack branch user-name/stack/air: air and bazel stay as they are on the remote.")
  })

  test("should say in the popover of the whole stack that its approvals count in its pull request while Patronus judges them", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const popover = await openWholeStackPopoverWithPullRequest(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED)
    await expect(popover.getByTestId("safe-push-review")).toHaveText("Patronus doesn't tell yet whose approvals count: Branch Deck can't check alice's approval.")
    await expect(popover.getByTestId("safe-push-whole-stack-approvals")).toHaveText("Approvals count in #12, the pull request of the whole stack.")
  })

  test("should review the whole stack in the window of its stack branch, from the island's menu and the root's menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()

    await openIslandMenu(page, "air")
    await page.getByRole("menuitem", { name: "Review Whole Stack" }).click()
    await expect.poll(async () => (await getOpenedSubWindows<BranchReviewWindowData>(page)).length).toBe(1)
    await openWholeStackSubmenu(page, "air")
    await page.getByRole("menuitem", { name: "Review Whole Stack" }).click()
    await expect.poll(async () => (await getOpenedSubWindows<BranchReviewWindowData>(page)).length).toBe(2)
    for (const opened of await getOpenedSubWindows<BranchReviewWindowData>(page)) {
      expect(opened).toMatchObject({ windowId: "branch-review", title: "Review the whole air stack", data: { chain: ["air"], wholeStack: true } })
    }
  })

  test("should show the robot of the whole stack on the rows of a small stack, hold their pushes and cancel it from the root's menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stacked_branches", { patronus: availableRobots(wholeStackRobot("auth", "whole")) })
    await syncAndWaitForBranches()
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => wholeStackRobot("auth", "whole", { status: "canceling", sourceRevision: "stack-tip" }))

    // every branch of the stack says what lands it; the branch outside it doesn't
    for (const name of ["auth", "login", "oauth", "session"]) {
      await expect(findBranchRow(page, name).getByTestId("landing-span")).toHaveText("lands with the whole auth stack")
    }
    await expect(findBranchRow(page, "docs")).toBeVisible()
    await expect(findBranchRow(page, "docs").getByTestId("landing-span")).toHaveCount(0)

    // a push of a branch of it won't land: the menu says why
    await findBranchRow(page, "login").click({ button: "right" })
    const push = page.getByRole("menuitem", { name: /^Push/ }).first()
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(page.getByRole("tooltip")).toHaveText("A Safe Push of the whole auth stack is running: it lands login with it")
    await page.keyboard.press("Escape")

    // the robots indicator names it as the whole stack
    await page.getByTestId("robots-indicator").hover()
    await expect(page.getByTestId("robots-popover").getByTestId("robots-row")).toHaveAttribute("data-branch-name", "Whole auth stack")
    await page.keyboard.press("Escape")

    // its actions are the whole stack's, in the root's menu
    await openWholeStackSubmenu(page, "auth")
    await page.getByRole("menuitem", { name: "Cancel Safe Push" }).click()
    await expect.poll(() => cancels.length).toBe(1)
    expect(cancels[0]).toMatchObject({ robotId: "whole" })
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Safe Push is canceling" })).toContainText("Whole auth stack → master")
  })

  test("should count the robot of the whole stack in the header of its island, a click on it showing the root", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island", { patronus: availableRobots(wholeStackRobot("air", "whole")) })
    await syncAndWaitForBranches()
    const air = island(page, "air")
    const states = air.getByTestId("stack-island-states")

    // it runs on the stack branch, no branch's: the header counts it as running, its ring on the count, and the line
    // below the name says what it lands
    await expect(air.getByTestId("stack-island-landing")).toHaveText("Safe Push of the whole stack lands 7 branches")
    await expect(states.getByRole("button")).toHaveText(["1 running"])
    await expect(states.getByTestId("robot-ring")).toHaveCount(1)
    await expect(air.getByTestId("stack-island-landing").getByTestId("robot-ring")).toHaveCount(0)

    // collapsed, the island still tells it, and the count shows the root, whose stack it lands
    await air.getByTestId("stack-island-root").click()
    await expect(air).toHaveAttribute("data-state", "closed")
    await states.getByRole("button", { name: "1 running" }).click()
    await expect(air).toHaveAttribute("data-state", "open")
    await expect(getBranchDetailsRow(page, "air")).toBeVisible()
  })

  test("should close the pull requests of every branch of the whole stack once its Safe Push merged", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const stack = stackPullRequest("auth", 12)
    // the branches of the stack are on the remote as the pull requests show them; oauth has none
    const own = [
      pullRequest("auth", 4, { headRefOid: "remote-auth" }),
      pullRequest("login", 5, { headRefOid: "remote-login" }),
      pullRequest("session", 6, { headRefOid: "remote-session" }),
    ]
    const running = wholeStackRobot("auth", "whole", { sourceRevision: "stack-tip", pullRequest: { number: 12, url: stack.url } })
    const repo = await setupRepo("stacked_branches", { patronus: availableRobots(running), pullRequests: withStackPullRequests(availablePullRequests(...own), { auth: stack }) })
    await rewriteNextSyncEvents(page, event => {
      if (event.type === "remoteStatusUpdate" && event.data.branchName !== "docs") {
        const remoteTip = `remote-${event.data.branchName}`
        return { ...event, data: { ...event.data, remoteExists: true, remoteTip, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } }
      }
      // the robot runs on the stack branch as the sync built it
      return event.type === "stackBranchUpdate" ? { ...event, data: { ...event.data, tip: "stack-tip" } } : event
    })
    await syncAndWaitForBranches()
    const closes = await routeCommand(page, "close_pull_request", () => null)
    await expect(findBranchRow(page, "login").getByTestId("landing-span")).toHaveText("lands with the whole auth stack")

    // it merged: the pull requests of the branches it landed are closed, saying with what; its own is Patronus'
    await repo.setPatronus(availableRobots({ ...running, status: "success", finishTime: 1704142800 }))
    await emitTauriEvent(page, "tauri://focus")
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Whole auth stack merged into master" })).toContainText(
      "closes the pull requests of 3 branches: they landed with the whole stack",
    )
    await expect.poll(() => closes.map(close => close.pullRequestNumber as number).sort((a, b) => a - b)).toEqual([4, 5, 6])
    expect(closes[0]!.comment).toContain("the Safe Push of the whole auth stack")
  })

  test("should say on every branch of the stack that the whole stack landed it until the update, and offer no Safe Push or push of them", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // auth forks: login and oauth on one side, session on the other
    const repo = await setupRepo("stacked_branches", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page, {})
    await routeCommand(page, "get_patronus_start_options", () => DEFAULT_CHECKS_ONLY)
    await routeReviewRequirement(page, () => NO_REVIEW_NEEDED)
    const started = wholeStackRobot("auth", "whole", { sourceRevision: null })
    const starts = await routeCommand(page, "start_patronus_robot", () => started)
    const members = ["auth", "login", "oauth", "session"]

    // each branch offers its own Safe Push, with what it is built on
    for (const [name, label] of [
      ["auth", "Safe Push"],
      ["login", "Safe Push · 2"],
      ["oauth", "Safe Push · 3"],
      ["session", "Safe Push · 2"],
    ]) {
      await expect(slotSafePush(page, name!)).toHaveText(label!)
    }
    // the Safe Push of the whole stack lands them all in one robot, from its stack branch
    await openWholeStackSubmenu(page, "auth")
    await page.getByRole("menuitem", { name: "Safe Push Whole Stack" }).click()
    await expect(safePushPopover(page)).toHaveAttribute("aria-label", "Safe Push of the whole auth stack")
    await popoverStart(page).click()
    await expect.poll(() => starts.length).toBe(1)
    expect(stackPushes).toHaveLength(1)
    await expect(findBranchRow(page, "session").getByTestId("landing-span")).toHaveText("lands with the whole auth stack")

    // it merged: every branch of the stack says so, until the update archives them
    await repo.setPatronus(availableRobots({ ...started, sourceRevision: "", status: "success", finishTime: 1704142800 }))
    await emitTauriEvent(page, "tauri://focus")
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Whole auth stack merged into master" })).toBeVisible()
    for (const name of members) {
      const row = findBranchRow(page, name)
      await expect(row.getByTestId("landed-with")).toHaveText("Landed with the whole auth stack")
      await expect(row.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await expect(row.getByTestId("branch-status")).toHaveText("landed")
    }
    await expect(findBranchRow(page, "docs").getByTestId("landed-with")).toHaveCount(0)
    await expect(slotSafePush(page, "docs")).toHaveText("Safe Push")
    // the whole stack has nothing left to start either
    await openWholeStackSubmenu(page, "auth")
    await expect(page.getByRole("menuitem", { name: "Safe Push Whole Stack" })).toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")
    await page.keyboard.press("Escape")

    const chip = findBranchRow(page, "oauth").getByTestId("landed-with")
    await chip.hover()
    await expect(page.getByTestId("landed-with-popover")).toContainText("In master with the Safe Push of the whole auth stack · the update removes the local copies")
    await page.mouse.move(0, 0)

    // a branch of it: no Safe Push, no Dry Run, no push in its menu, each saying why; S says it too
    await expectLandedActions(page, "oauth", "Landed in master with the whole auth stack")
    await expectLandedActions(page, "session", "Landed in master with the whole auth stack")
  })

  test("should say that the pull request of the whole stack no longer matches it once a piece landed the root, and close it only when asked", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const running = robot("login", "login-robot")
    const repo = await setupRepo("stacked_branches", {
      patronus: availableRobots(running),
      pullRequests: withStackPullRequests(availablePullRequests(), { auth: stackPullRequest("auth", 12) }),
    })
    await syncAndWaitForBranches()
    const closes = await routeCommand(page, "close_pull_request", () => null)

    // the Safe Push of login lands auth, the root: the pull request of the whole stack shows a stack that is no more
    await repo.setPatronus(availableRobots({ ...running, status: "success", finishTime: 1704142800 }))
    await emitTauriEvent(page, "tauri://focus")
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "login merged into master" })).toContainText("Safe Push · #12 no longer matches the auth stack: login landed")
    expect(closes).toEqual([])

    // the root's preview says it and asks in the root's row
    const preview = await openBranchPreview(page, "auth")
    const notice = preview.getByTestId("orphaned-stack-pull-request")
    await expect(notice).toContainText("#12 no longer matches the auth stack: login landed.")
    await notice.getByRole("button", { name: "Close Pull Request #12…" }).click()
    await expect(preview).toHaveCount(0)
    const confirm = page.locator('tr[data-branch-of="auth"]').filter({ has: page.getByRole("button", { name: "Close Pull Request", exact: true }) })
    await expect(confirm).toContainText("Close pull request #12")
    await page.keyboard.press("Escape")
    await expect(confirm).toHaveCount(0)
    expect(closes).toEqual([])

    // the root's menu: the same, and the close goes to GitHub only from the confirmation
    await openWholeStackSubmenu(page, "auth")
    await page.getByRole("menuitem", { name: "Close Pull Request #12…" }).click()
    await confirm.getByRole("button", { name: "Close Pull Request", exact: true }).click()
    await expect.poll(() => closes.length).toBe(1)
    expect(closes[0]).toMatchObject({ pullRequestNumber: 12, comment: "Closed in Branch Deck: #12 no longer matches the auth stack: login landed." })
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Closed pull request #12" })).toBeVisible()
  })
})
