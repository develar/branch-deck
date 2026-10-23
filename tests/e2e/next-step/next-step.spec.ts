// @spec spec/branch/next-step.spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusProfile, PatronusReviewRequirement, PullRequest } from "../../../src/lib/bindings"
import type { BranchReviewRequest } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { openActivityWindow } from "../helpers/activity-helpers"
import { waitForTransitionsSettled } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, getLandingNode } from "../helpers/branch-helpers"
import {
  APPROVED_BY_CODE_OWNER,
  AWAITING_CODE_OWNER,
  NO_REVIEW_NEEDED,
  NOT_EVALUATED,
  availableRobots,
  codeOwner,
  evaluatedGroup,
  robot,
  startRequest,
  setupApprovalsDismissedByPush,
} from "../helpers/patronus-helpers"
import {
  availablePullRequests,
  createPullRequestDialog,
  getOpenedUrls,
  gitHubRequests,
  gitHubWrites,
  pullRequest,
  repoPath,
  reviewer,
  skipCommandReplyWait,
} from "../helpers/pull-request-helpers"
import { pushedToast, routePush } from "../helpers/push-helpers"
import { addRecentProject, OTHER_REPOSITORY, switchRepository } from "../helpers/repository-switch-helpers"
import { availableReviewRequests } from "../helpers/review-request-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { chooseProfile, expectPopoverWithoutReview, openSafePushWithKey, popoverStart, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { openSubmenu } from "../helpers/selection-helpers"
import { getEmittedEvents } from "../helpers/sub-window-helpers"
import { emitTauriEvent, rewriteNextRemoteStatus } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommand, holdCommandCalls, recordCommand, routeCommand } from "../helpers/wait-helpers"

const BRANCH = "JIRA-456"

/** The pull request of the screenshot of 2026-10-17: the bot approved, the code owner is asked */
/** IJPL Code Platform has to approve, Ilya Korennoy is suggested and not asked */
const UNASKED_CODE_OWNER: PatronusReviewRequirement = {
  ...AWAITING_CODE_OWNER,
  groups: [evaluatedGroup("IJPL Code Platform", false, [codeOwner("Ilya Korennoy", "ikorennoy", { suggested: true, approvalState: "reviewing" })])],
}

const AWAITED = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "requested")] })

/**
 * issue_links mirrored to `acme/monorepo`, JIRA-456 pushed (its remote branch has its commits; not on the remote with
 * `unpushed`), with these pull requests; Patronus answers the review pre-check of JIRA-456 with `answer`
 */
async function setup(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  answer: PatronusReviewRequirement,
  pullRequests: PullRequest[],
  robots = availableRobots(),
  { unpushed = false, ...options }: { unpushed?: boolean } & Record<string, unknown> = {},
): Promise<TestRepositoryBuilder> {
  if (!unpushed) {
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === BRANCH ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } : status,
    )
  }
  const repo = await setupRepo("issue_links", { patronus: { ...robots, githubRepository: "acme/monorepo" }, pullRequests: gitHubOf(...pullRequests), ...options })
  await repo.setReviewAnswers({ answers: { [BRANCH]: answer } })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  return repo
}

/** The GitHub of these pull requests, the code owner Ilya Korennoy a collaborator; the next pull request created is #1079 */
function gitHubOf(...pullRequests: PullRequest[]) {
  return { ...availablePullRequests(...pullRequests), users: [{ login: "ikorennoy", name: "Ilya Korennoy" }], nextNumber: 1079 }
}

/**
 * JIRA-456 not on the remote (as `rewriteNextRemoteStatus` before it leaves it), its approvals missing, no pull
 * request: the pushes of its "Create pull request" dialog, answered in the page, and its creations, recorded on the way
 * to the fake GitHub
 */
async function setupUnpushedCreation(page: Page, setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>) {
  const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [], availableRobots(), { unpushed: true })
  const pushes = await routePush(page, {})
  const creates = await recordCommand(page, "create_pull_request")
  return { pushes, creates, repo }
}

function slot(page: Page) {
  return findBranchRow(page, BRANCH).getByTestId("next-step-slot")
}

/** A click on the Safe Push of JIRA-456's slot, which offers it, that has something to say: its popover at the slot */
async function clickSafePushForPopover(page: Page) {
  await expect(slot(page)).toHaveAttribute("data-step", "safePush")
  await slotSafePush(page, BRANCH).click()
  const popover = safePushPopover(page)
  await expect(popover).toHaveAttribute("data-decision", "confirm")
  return popover
}

/** "Safe Push" (its key S in the name), not "Safe Push With Profile…" nor "Safe Push Without Review…" */
const SAFE_PUSH_ITEM = { name: /^Safe Push( S)?$/ }

/**
 * The start of JIRA-456 from #1078 (`/patronus merge`): the fake GitHub's bot answers the comment with 👍; the robot
 * it started is in the list asked for after it
 */
async function routePullRequestStart(page: Page) {
  const started = robot(BRANCH, "pr-robot", { sourceRevision: null, startTime: Math.floor(Date.now() / 1000), pullRequest: { number: 1078, url: AWAITED.url } })
  await routeCommand(page, "get_patronus_robots", params => (params.since === null ? availableRobots() : availableRobots(started)))
}

/**
 * The comment of the start on #1078 reached the fake GitHub, the app read the bot's reply over its wait; then the
 * toast of the started robot, with Open in Patronus and Cancel Safe Push
 */
async function expectStartedFromPullRequest(page: Page, repo: TestRepositoryBuilder) {
  await expect.poll(() => gitHubRequests(repo, "POST", "issues", 1078, "comments")).toEqual([{ body: "/patronus merge --keep-source-branch" }])
  await skipCommandReplyWait(page, repo)
  await expectStartedToast(page)
}

/** The toast of a started Safe Push: Open in Patronus, and Cancel Safe Push below the text */
async function expectStartedToast(page: Page) {
  const toast = page.locator("[data-slot=toast]", { hasText: "Safe Push started" })
  await expect(toast).toBeVisible()
  await expect(toast.getByRole("button", { name: "Open in Patronus" })).toBeVisible()
  await expect(toast.getByRole("button", { name: "Cancel Safe Push" })).toBeVisible()
}

/** JIRA-456 with its open pull request #7 into `base`, a virtual branch: the step retargets it */
async function setupRetarget(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  base: string,
): Promise<TestRepositoryBuilder> {
  const repo = await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [pullRequest(BRANCH, 7, { baseRefName: base })])
  await expect(slot(page)).toHaveAttribute("data-step", "retarget")
  return repo
}

/**
 * GitHub got the retarget of pull request `number` into master, the toast said so, and the step of `stepSlot` is the
 * Safe Push that counts its approvals now
 */
async function expectRetargeted(page: Page, repo: TestRepositoryBuilder, number = 7, stepSlot = slot(page)) {
  await expect.poll(async () => gitHubWrites(await repo.getGitHubLog())).toEqual([["PATCH", repoPath("pulls", number), { base: "master" }]])
  await expect(page.locator("[data-slot=toast]", { hasText: `Retargeted #${number} to master` })).toBeVisible()
  await expect(stepSlot).toHaveAttribute("data-step", "safePush")
}

async function openMenu(page: Page) {
  await findBranchRow(page, BRANCH).click({ button: "right" })
  return page.getByRole("menu")
}

/**
 * The branch menu with Safe Push and Safe Push With Profile… disabled, and Dry Run `dryRun` (it needs no approval); its
 * Patronus › stays open
 */
async function expectSafePushItemsDisabled(page: Page, dryRun: "enabled" | "disabled"): Promise<Locator> {
  const menu = await openMenu(page)
  await expect(menu.getByRole("menuitem", SAFE_PUSH_ITEM)).toBeDisabled()
  await openSubmenu(page, "Patronus")
  await expect(menu.getByRole("menuitem", { name: "Safe Push With Profile…" })).toBeDisabled()
  const dryRunItem = menu.getByRole("menuitem", { name: "Dry Run" })
  await (dryRun === "enabled" ? expect(dryRunItem).toBeEnabled() : expect(dryRunItem).toBeDisabled())
  return menu
}

test.describe("Next step", () => {
  test("should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])

    // the quiet wait in the slot, which opens the pull request it waits in
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Awaiting Review")
    await expect(step).toHaveAttribute("data-tone", "quiet")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078O")
    // the chip of the pull request keeps its own state: the row doesn't say it twice
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).not.toContainText("Awaiting")
    await step.click()
    expect(await getOpenedUrls(page)).toEqual([AWAITED.url])

    // "⋯": Safe Push and its profile disabled with the reason, Dry Run needs no approval, Check Approvals Again
    const menu = await expectSafePushItemsDisabled(page, "enabled")
    await expect(menu.getByRole("menuitem", { name: "Check Approvals Again" })).toBeEnabled()
    await menu.getByRole("menuitem", SAFE_PUSH_ITEM).hover()
    await expect(page.getByRole("tooltip", { name: "Waits for" })).toHaveText("Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078")
    await page.keyboard.press("Escape")

    // S on the row: the same reason, a precondition, and no popover
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    const toast = page.locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} waits for Ilya Korennoy` })
    await expect(toast).toHaveAttribute("data-type", "info")
    await expect(toast).toContainText("IJPL Code Platform · requested on #1078")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    await expect(safePushPopover(page)).toHaveCount(0)
  })

  test("should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // a Patronus that doesn't evaluate the pull request yet: nobody can tell whether the bot's approval counts
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED, [AWAITED])
    // GitHub waits for ikorennoy: the slot says so, S still opens the start
    await expect(slot(page).getByTestId("next-step")).toHaveText("Awaiting Review")
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-review")).toHaveText("Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval.")
    await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #1078: Patronus reports there and closes it once merged.")
    await popover.getByRole("button", { name: "Cancel" }).click()
    await expect(popover).toHaveCount(0)

    // Patronus evaluates it now: Check Approvals Again asks again, not the kept answer
    await repo.setReviewAnswers({ answers: { [BRANCH]: AWAITING_CODE_OWNER } })
    const menu = await openMenu(page)
    await openSubmenu(page, "Patronus")
    await menu.getByRole("menuitem", { name: "Check Approvals Again" }).click()
    await expect.poll(async () => (await repo.getReviewReadinessCalls()).some(call => call.recheck && call.branches.includes(BRANCH))).toBe(true)
    await expect(slot(page).getByTestId("next-step")).toHaveText("Awaiting Review")
    await slot(page).getByTestId("next-step").hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078O")

    // the menu's Safe Push says why: Patronus would fail it
    const menuAgain = await openMenu(page)
    await expect(menuAgain.getByRole("menuitem", SAFE_PUSH_ITEM)).toBeDisabled()
    await page.keyboard.press("Escape")
    // S says why in a toast, and opens no popover
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    await expect(page.locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} waits for Ilya Korennoy` })).toHaveAttribute("data-type", "info")
    await expect(safePushPopover(page)).toHaveCount(0)
  })

  test("should wait for the code owner a stacked branch's own pull request asks, not for its root", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // oauth is built on auth and login; its pull request goes into master, its Safe Push lands the three of them
    const own = pullRequest("oauth", 8, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "requested")] })
    const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(own) })
    await repo.setReviewAnswers({ answers: { oauth: AWAITING_CODE_OWNER } })
    await syncAndWaitForBranches()

    const row = findBranchRow(page, "oauth")
    const oauthSlot = row.getByTestId("next-step-slot")
    await expect(oauthSlot).toHaveAttribute("data-step", "awaiting")
    await expect(oauthSlot.getByTestId("next-step")).toHaveText("Awaiting Review")
    await oauthSlot.getByTestId("next-step").hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #8O")
    // Patronus is asked for the stacked branch itself, its start the comment on #8
    expect((await repo.getReviewReadinessCalls()).some(call => call.branches.includes("oauth"))).toBe(true)
    // S says why, naming the branch, and opens no form
    await row.focus()
    await page.keyboard.press("s")
    const toast = page.locator("[data-slot=toast]", { hasText: "Safe Push of oauth waits for Ilya Korennoy" })
    await expect(toast).toHaveAttribute("data-type", "info")
    await expect(toast).toContainText("IJPL Code Platform · requested on #8")
    await expect(safePushPopover(page)).toHaveCount(0)
  })

  test("should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "approved")] })
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, APPROVED_BY_CODE_OWNER, [approved])
    await routePullRequestStart(page)
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await slotSafePush(page, BRANCH).hover()
    await expect(page.getByRole("tooltip", { name: "Lands" })).toHaveText(`Lands ${BRANCH} into master once the checks passS`)

    // S: the popover names whose approval counts and where the start comments
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover.getByTestId("safe-push-review")).toHaveText("Approved by Ilya Korennoy for IJPL Code Platform.")
    await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #1078: Patronus reports there and closes it once merged.")
    await expect(popoverStart(page)).toBeEnabled()
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    // nothing to say: the click starts at once, a comment on the pull request
    await slotSafePush(page, BRANCH).click()
    await expectStartedFromPullRequest(page, repo)
    await expect(safePushPopover(page)).toHaveCount(0)
  })

  test("should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const botApproved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")] })
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED, [botApproved])
    await routePullRequestStart(page)
    const popover = await clickSafePushForPopover(page)
    await expect(popover.getByTestId("safe-push-review")).toHaveText("Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval.")
    await expect(popoverStart(page)).toHaveText("Safe Push")
    await popoverStart(page).click()
    await expect(popover).toHaveCount(0)
    await expectStartedFromPullRequest(page, repo)
  })

  test("should wait for the reviewer GitHub asks while Patronus can't judge the approvals, keep Safe Push, and tell the approval", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the screenshot of 2026-10-18: a bot approved #1078, Patronus doesn't tell whose approvals count, ikorennoy is asked
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED, [AWAITED], availableRobots(), { reviewRequests: availableReviewRequests([]) })
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    await expect(slot(page).getByTestId("next-step")).toHaveText("Awaiting Review")
    // the node waits, never "Ready to land"
    const node = findBranchRow(page, BRANCH).getByTestId("lane-node")
    await expect(node).toHaveAttribute("data-kind", "waiting")
    await findBranchRow(page, BRANCH).getByTestId("lane-node-target").hover()
    await expect(page.getByTestId("lane-node-details").filter({ hasText: "Waits for ikorennoy" })).toHaveText(
      "Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them\nOpen Pull Request #1078",
    )
    await page.mouse.move(0, 0)
    // Patronus may still take it: S opens the start, which says that it checks the approvals
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-review")).toContainText("Patronus doesn't tell yet whose approvals count")
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    // ikorennoy approves: GitHub waits for nobody, Patronus still can't judge, the approval is news
    await repo.setPullRequests(availablePullRequests(pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "approved")] })))
    await page.clock.fastForward("05:00")
    const toast = page.locator("[data-slot=toast]", { hasText: `ikorennoy approved ${BRANCH}` })
    await expect(toast).toBeVisible()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    // nobody judged it yet: the user's step, not "Ready to land"
    await expect(node).toHaveAttribute("data-kind", "yourStep")
  })

  test("should offer Create Pull Request… with the code owner when approvals count only in one", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [])
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Create Pull Request…")

    const menu = await openMenu(page)
    await menu.getByRole("menuitem", SAFE_PUSH_ITEM).hover()
    await expect(page.getByRole("tooltip", { name: "Code owners" })).toHaveText("Code owners must approve in a pull request: IJPL Code Platform")
    await page.keyboard.press("Escape")

    await step.click()
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["ikorennoy"])
  })

  test("should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const { pushes, creates, repo } = await setupUnpushedCreation(page, setupRepo, syncAndWaitForBranches)
    // the step of the slot, never a "push first": the push is the dialog's to make
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toBeEnabled()
    await step.click()
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-submit")).toHaveText(/^Create Pull Request/)
    await page.keyboard.press("Escape")
    await expect(dialog).toHaveCount(0)
    expect(pushes).toEqual([])

    // the menu offers it too, and its Create pushes the branch, then creates the pull request
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Create Pull Request…" }).click()
    await expect(dialog).toBeVisible()
    const node = getLandingNode(findBranchRow(page, BRANCH))
    await expect(node).toHaveAttribute("data-kind", "yourStep")
    const releaseCreate = await holdCommand(page, "create_pull_request")
    await dialog.getByTestId("create-pull-request-submit").click()
    // the creation is the move made: the branch waits for GitHub from the click on, the push and the mirror included
    await expect(node).toHaveAttribute("data-kind", "waiting")
    releaseCreate()
    await expect(page.locator("[data-slot=toast]", { hasText: "Created pull request #1079" })).toBeVisible()
    await expect(dialog).toHaveCount(0)
    expect(pushes.map(push => push.branchName)).toEqual([BRANCH])
    expect(creates).toEqual([expect.objectContaining({ branchName: BRANCH, waitForMirror: true })])
    // GitHub got the pull request with the code owner Patronus suggests as its reviewer
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual([
      ["POST", repoPath("pulls"), expect.objectContaining({ head: `user-name/virtual/${BRANCH}`, base: "master" })],
      ["POST", repoPath("issues", 1079, "assignees"), { assignees: ["user-name"] }],
      ["POST", repoPath("pulls", 1079, "requested_reviewers"), { reviewers: ["ikorennoy"] }],
    ])
    // one action, one toast: the push says nothing of its own
    await expect(pushedToast(page, BRANCH)).toHaveCount(0)
  })

  test("should create the pull request asked from the review window in its repository when the main window opens another during its push", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the push goes to the test server: its real push into the template's own repository
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [], availableRobots(), { unpushed: true })
    const creates = await recordCommand(page, "create_pull_request")
    await addRecentProject(page)
    await syncAndWaitForBranches()
    const push = await holdCommandCalls(page, "push_branch")
    // Create of the dialog of the review window, asked of the main window over the link of the window
    const input = { title: "Add auth", body: "", reviewers: [], assignSelf: false, viewerLogin: null }
    await emitTauriEvent(page, "branch-review/request", { id: "create", repositoryPath: repo.path, branchName: BRANCH, dialog: "create", input } satisfies BranchReviewRequest)
    await expect.poll(() => push.calls.map(call => call.branchName)).toEqual([BRANCH])

    // the main window opens another repository while the push runs: the pull request still goes to the one asked
    await switchRepository(page, OTHER_REPOSITORY)
    push.release()
    await expect
      .poll(async () => (await getEmittedEvents<{ id: string; ok: boolean }>(page, "branch-review/reply")).filter(it => it.id === "create").map(it => it.ok))
      .toEqual([true])
    expect(creates).toEqual([expect.objectContaining({ repositoryPath: repo.path, branchName: BRANCH })])
    expect(gitHubWrites(await repo.getGitHubLog())).toContainEqual(["POST", repoPath("pulls"), expect.objectContaining({ head: `user-name/virtual/${BRANCH}` })])

    // back in it, the row has its pull request
    await switchRepository(page, repo.path)
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).toHaveText("#1079")
  })

  test("should open GitHub's form of a branch not on the remote yet from its preview, after the push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [], availableRobots(), { unpushed: true })
    const pushes = await routePush(page, {})
    const waits = await recordCommand(page, "wait_for_github_branch")
    const preview = await openBranchPreview(page, BRANCH)
    await preview.getByRole("button", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(page)
    await dialog.getByTestId("create-pull-request-on-github").click()
    await expect(dialog).toHaveCount(0)
    expect(pushes.map(push => push.branchName)).toEqual([BRANCH])
    expect(waits).toEqual([expect.objectContaining({ branch: `user-name/virtual/${BRANCH}` })])
    expect(await getOpenedUrls(page)).toEqual([expect.stringMatching(new RegExp(`/compare/master\\.\\.\\.user-name/virtual/${BRANCH}\\?`))])
  })

  test("should keep the dialog, with the reason, when the push would drop commits of the remote branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === BRANCH ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 2, fastForward: false, remoteTip: "theirs" } : status,
    )
    const { pushes, creates } = await setupUnpushedCreation(page, setupRepo, syncAndWaitForBranches)
    await slot(page).getByTestId("next-step").click()
    const dialog = createPullRequestDialog(page)
    await dialog.getByTestId("create-pull-request-submit").click()
    const error = page.locator("[data-slot=toast]", { hasText: "Cannot create pull request" })
    await expect(error).toContainText(`The remote branch of ${BRANCH} has 2 commits that ${BRANCH} lacks: push it from its row, which shows them`)
    await expect(dialog).toBeVisible()
    expect(pushes).toEqual([])
    expect(creates).toEqual([])
  })

  test("should start the Safe Push of a branch that needs no review with a click, and offer Create Pull Request… in its menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [])

    // S: the popover says that no review is needed
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover.getByTestId("safe-push-review")).toHaveText("No code-owner review needed.")
    await popover.getByRole("button", { name: "Cancel" }).click()
    await expect(popover).toHaveCount(0)
    expect((await repo.getPatronusLog()).starts).toEqual([])

    // the click has nothing to say: it starts at once, with the default checks; the toast cancels it
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await slotSafePush(page, BRANCH).click()
    await expectStartedToast(page)
    await expect(safePushPopover(page)).toHaveCount(0)
    expect((await repo.getPatronusLog()).starts).toEqual([startRequest(BRANCH)])
    await page.locator("[data-slot=toast]", { hasText: "Safe Push started" }).getByRole("button", { name: "Cancel Safe Push" }).click()
    // the robot Patronus started
    await expect.poll(async () => (await repo.getPatronusLog()).cancels).toEqual(["started-1"])

    // the pull request is the menu's
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Create Pull Request…" }).click()
    await expect(createPullRequestDialog(page)).toBeVisible()
  })

  test("should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the bot approved, nobody of IJPL Code Platform is asked
    const unasked = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")] })
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, UNASKED_CODE_OWNER, [unasked])
    await expect(slot(page)).toHaveAttribute("data-step", "requestReview")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Request Review…")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(
      "Waits for the approval of IJPL Code Platform: nobody of it is requested on #1078. Ask Ilya Korennoy to review #1078",
    )

    await step.click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #1078")
    await expect(popover.getByTestId("request-review-candidate-ikorennoy")).toBeChecked()
    // GitHub lists the request from now on
    await repo.setPullRequests(gitHubOf({ ...unasked, reviewers: [...unasked.reviewers, reviewer("ikorennoy", "requested")] }))
    const node = getLandingNode(findBranchRow(page, BRANCH))
    await expect(node).toHaveAttribute("data-kind", "yourStep")
    const releaseRequest = await holdCommand(page, "request_pull_request_reviewers")
    // Patronus answers again only when the test lets it: the wait shows before its new answer
    const releaseReadiness = await holdCommand(page, "get_branch_review_readiness")
    await popover.getByTestId("request-review-submit").click()
    // the request is the move made: the branch waits for the reviewer from the click on
    await expect(node).toHaveAttribute("data-kind", "waiting")
    releaseRequest()
    await expect(page.locator("[data-slot=toast]", { hasText: "Requested a review from Ilya Korennoy on #1078" })).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 1078, "requested_reviewers")).toEqual([{ reviewers: ["ikorennoy"] }])
    await expect(popover).toHaveCount(0)
    // the pull request asks him now: the wait, while Patronus is still to answer again
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    await expect(node).toHaveAttribute("data-kind", "waiting")
    releaseReadiness()
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
  })

  test("should push the commits the remote lacks as the first step of Request Review…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const unasked = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")] })
    // a commit the remote lacks: the reviewers would see the old commits without the push
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === BRANCH
        ? { ...status, remoteExists: true, unpushedCommits: [status.unpushedCommits[0] ?? "new"], myUnpushedCount: 1, commitsBehind: 0, fastForward: true }
        : status,
    )
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, UNASKED_CODE_OWNER, [unasked], availableRobots(), { unpushed: true })
    const pushes = await routePush(page, {})
    await expect(slot(page)).toHaveAttribute("data-step", "requestReview")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toBeEnabled()
    await step.click()
    const popover = page.getByTestId("request-review-popover")
    await popover.getByTestId("request-review-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Requested a review from Ilya Korennoy on #1078" })).toBeVisible()
    expect(pushes.map(push => push.branchName)).toEqual([BRANCH])
    expect(await gitHubRequests(repo, "POST", "pulls", 1078, "requested_reviewers")).toEqual([{ reviewers: ["ikorennoy"] }])
    // one action, one toast
    await expect(pushedToast(page, BRANCH)).toHaveCount(0)
  })

  test("should open Request Review… from the notice of S while nobody of a group is asked", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, UNASKED_CODE_OWNER, [pullRequest(BRANCH, 1078)])
    await expect(slot(page)).toHaveAttribute("data-step", "requestReview")
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    const notice = page.locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} waits for a review request` })
    await expect(notice).toContainText("nobody is requested on #1078")
    // the notice runs the step the slot offers
    await notice.getByRole("button", { name: "Request Review…" }).click()
    await expect(page.getByTestId("request-review-popover")).toContainText("Request a review on #1078")
  })

  test("should open Request Review… from the branch menu, at the row's next step", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Request Review…" }).click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #1078")
    // asked already: the code owner is no candidate, the search finds others
    await expect(popover.getByTestId("request-review-candidates")).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)
  })

  test("should retarget a pull request while a review is requested in it: both reach GitHub, neither refuses the other", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // #1078 goes into a virtual branch and nobody of the code owners is asked: a retarget and a review request
    const unasked = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")], baseRefName: "user-name/virtual/feature-auth" })
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, UNASKED_CODE_OWNER, [unasked])
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Request Review…" }).click()
    const popover = page.getByTestId("request-review-popover")
    // the approvals into a virtual branch don't count: no code owner to offer, the search finds whom to ask
    await popover.getByTestId("request-review-reviewer-search").fill("ikor")
    await page.getByRole("option", { name: /ikorennoy/ }).click()
    const request = await holdCommandCalls(page, "request_pull_request_reviewers")
    await popover.getByTestId("request-review-submit").click()
    await expect.poll(() => request.calls.length).toBe(1)

    // the retarget writes the base of the pull request, the request its reviewers: it runs while the request waits
    await page.keyboard.press("Escape")
    const retarget = slot(page).getByRole("button", { name: "Retarget #1078" })
    await expect(retarget).toBeEnabled()
    await retarget.click()
    await expect.poll(async () => gitHubWrites(await repo.getGitHubLog())).toEqual([["PATCH", repoPath("pulls", 1078), { base: "master" }]])
    await expect(page.locator("[data-slot=toast]", { hasText: "Retargeted #1078 to master" })).toBeVisible()
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot retarget" })).toHaveCount(0)

    request.release()
    await expect(page.locator("[data-slot=toast]", { hasText: "Requested a review from Ilya Korennoy on #1078" })).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 1078, "requested_reviewers")).toEqual([{ reviewers: ["ikorennoy"] }])
  })

  test("should retarget a pull request into the branch of a parent that landed, whose approvals don't count", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the base is the branch of a parent that is no branch of the table any more
    const repo = await setupRetarget(page, setupRepo, syncAndWaitForBranches, "user-name/virtual/landed-parent")
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", SAFE_PUSH_ITEM).hover()
    await expect(page.getByRole("tooltip", { name: "Approvals" })).toHaveText("Approvals in #7 don't count: its base is user-name/virtual/landed-parent")
    await page.keyboard.press("Escape")

    await slot(page).getByRole("button", { name: "Retarget #7" }).click()
    await expectRetargeted(page, repo)
  })

  test("should retarget a pull request into the virtual branch of a branch in the table, as a parent's, whose approvals don't count", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the base is the branch of another branch of the table: every pull request of a virtual branch goes into master
    const repo = await setupRetarget(page, setupRepo, syncAndWaitForBranches, "user-name/virtual/feature-auth")
    await slot(page).getByRole("button", { name: "Retarget #7" }).hover()
    await expect(page.getByTestId("next-step-hover")).toContainText("Approvals in #7 don't count: its base is user-name/virtual/feature-auth, a virtual branch.")
    // the menu offers the same retarget
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Retarget #7 to master" }).click()
    await expectRetargeted(page, repo)
  })

  test("should retarget the pull request of a stacked branch into its parent's branch from its slot, whose approvals don't count", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // an older Branch Deck opened #8 of oauth into the branch of login, its parent: every pull request goes into master
    const own = pullRequest("oauth", 8, { baseRefName: "user-name/virtual/login" })
    const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(own) })
    await repo.setReviewAnswers({ answers: { oauth: NO_REVIEW_NEEDED } })
    await syncAndWaitForBranches()

    const oauthSlot = findBranchRow(page, "oauth").getByTestId("next-step-slot")
    await expect(oauthSlot).toHaveAttribute("data-step", "retarget")
    await oauthSlot.getByRole("button", { name: "Retarget #8" }).click()
    await expectRetargeted(page, repo, 8, oauthSlot)
  })

  test("should open a pull request into another base that may be meant, and keep its retarget in the menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [pullRequest(BRANCH, 7, { baseRefName: "release-252" })])
    await expect(slot(page)).toHaveAttribute("data-step", "otherBase")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Open Pull Request")
    await expect(step).toHaveAttribute("data-tone", "quiet")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Approvals in #7 don't count: its base is release-252. Open Pull Request #7O")
    const menu = await openMenu(page)
    await expect(menu.getByRole("menuitem", { name: "Retarget #7 to master" })).toBeEnabled()
    await expect(menu.getByRole("menuitem", SAFE_PUSH_ITEM)).toBeDisabled()
    await page.keyboard.press("Escape")
    // S says it in a short title
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    const toast = page.locator("[data-slot=toast]", { hasText: `Approvals in #7 of ${BRANCH} don't count` })
    await expect(toast).toContainText("Its base is release-252, not master")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
  })

  test("should offer nothing after a merge, and say so in the menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const merged = robot(BRANCH, "merged-robot", { status: "success", finishTime: 1704142800 })
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [], availableRobots(merged))
    await expect(slot(page)).toHaveAttribute("data-step", "none")
    const menu = await expectSafePushItemsDisabled(page, "disabled")
    await menu.getByRole("menuitem", SAFE_PUSH_ITEM).hover()
    await expect(page.getByRole("tooltip", { name: "Merged" })).toHaveText("Merged by Patronus")
  })

  test("should tell the approval that arrives with a poll, with Safe Push of the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the review requests are polled: the poll asks for the user's pull requests too while a row waits
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED], availableRobots(), { reviewRequests: availableReviewRequests([]) })
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")

    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "approved")] })
    await repo.setPullRequests(availablePullRequests(approved))
    await repo.setReviewAnswers({ answers: { [BRANCH]: APPROVED_BY_CODE_OWNER } })
    await page.clock.fastForward("05:00")
    const toast = page.locator("[data-slot=toast]", { hasText: `Ilya Korennoy approved ${BRANCH}` })
    await expect(toast).toBeVisible()
    await expect(toast).toContainText("#1078 · IJPL Code Platform")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    // the row has the step already
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")

    // the toast's Safe Push has nothing to say: it starts at once, a comment on the pull request
    await routePullRequestStart(page)
    await toast.getByRole("button", { name: "Safe Push", exact: true }).click()
    await expectStartedFromPullRequest(page, repo)
    await expect(safePushPopover(page)).toHaveCount(0)
    // Activity keeps it
    const activity = await openActivityWindow(page)
    await expect(activity.getByTestId("activity-list")).toContainText(`Ilya Korennoy approved ${BRANCH}`)
  })

  test("should offer only the push when it dismisses the approvals Patronus counts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // JIRA-456 has a commit the remote lacks; the code owner approved #1078, and the rules dismiss stale reviews
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === BRANCH
        ? { ...status, remoteExists: true, unpushedCommits: [status.unpushedCommits[0] ?? "c1"], myUnpushedCount: 1, commitsBehind: 0, fastForward: true }
        : status,
    )
    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("ikorennoy", "approved")] })
    await setupApprovalsDismissedByPush(setupRepo, syncAndWaitForBranches, BRANCH, approved)
    // the remote still lacks a commit after each push: both surfaces can push
    const pushes = await routePush(page, { [BRANCH]: { unpushedCommits: ["c1"] } })
    const reason = "The push dismisses the approvals of #1078: push first, then ask for the review again"

    // S: the popover says why it can't start, and offers the push alone
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover.getByTestId("safe-push-status")).toHaveAttribute("data-kind", "refuse")
    await expect(popover.getByTestId("safe-push-status")).toHaveText(reason)
    await expect(popover.getByTestId("safe-push-dismisses")).toHaveText(
      "The push dismisses the approvals of #1078: the rules of the repository dismiss stale reviews when new commits are pushed.",
    )
    await expect(popoverStart(page)).toBeDisabled()
    await popover.getByTestId("safe-push-push-only").click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual([BRANCH])
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    // a click says it in a toast, whose action is the push alone
    await slotSafePush(page, BRANCH).click()
    const toast = page.locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} can't start` })
    await expect(toast).toHaveAttribute("data-type", "info")
    await expect(toast).toContainText(reason)
    await expect(safePushPopover(page)).toHaveCount(0)
    await toast.getByRole("button", { name: "Push", exact: true }).click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual([BRANCH, BRANCH])
  })

  test("should ask Patronus again from Check Approvals Again, after the pull request from GitHub", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    await repo.setReviewAnswers({ answers: { [BRANCH]: APPROVED_BY_CODE_OWNER } })
    const menu = await openMenu(page)
    await openSubmenu(page, "Patronus")
    await menu.getByRole("menuitem", { name: "Check Approvals Again" }).click()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    expect((await repo.getReviewReadinessCalls()).at(-1)).toEqual({ branches: [BRANCH], recheck: true })
  })

  test.describe("Safe Push without review", () => {
    const LSC = {
      id: "lsc",
      name: "Large Scale Change",
      description: "ALWAYS(!) READ FIRST: https://jb.gg/ulc. Disables mandatory review.",
      allowedPushModes: [],
      skipsReview: null,
    }
    const EMERGENCY: PatronusProfile[] = [{ id: "emergency", name: "Emergency", description: "For EMERGENCY USE ONLY, skips ALL checks", allowedPushModes: [], skipsReview: true }]
    /**
     * The merge profiles Patronus offers on master (`patronusProfiles`). Whether a regular one skips the review only the
     * options of the branch's last robot say (none here); a profile they say checks it is left out (`getBypassProfiles`)
     */
    const PROFILES = { profiles: [LSC], emergencyProfiles: EMERGENCY }

    /** Row 7 for JIRA-456 (the code owner must approve in a pull request it doesn't have), with `PROFILES`; the repository */
    async function setupRow7(page: Page, setupRepo: Parameters<typeof setup>[1], syncAndWaitForBranches: () => Promise<unknown>): Promise<TestRepositoryBuilder> {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [], availableRobots(), { patronusProfiles: PROFILES })
      return repo
    }

    /** The toast of the started Safe Push, after Patronus got one start of JIRA-456 with the merge profile `profileId` */
    async function expectStartedWith(page: Page, repo: TestRepositoryBuilder, profileId: string) {
      await expectStartedToast(page)
      expect((await repo.getPatronusLog()).starts).toEqual([startRequest(BRANCH, { mergeProfileId: profileId })])
    }

    /** The dialog of the pull request from the slot of row 7, and its Safe Push Without Review… */
    async function openWithoutReview(page: Page) {
      await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
      await slot(page).getByTestId("next-step").click()
      const dialog = createPullRequestDialog(page)
      return { dialog, withoutReview: dialog.getByRole("button", { name: "Safe Push Without Review…" }) }
    }

    test("should start from the pull request dialog with the profile the user chooses", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRow7(page, setupRepo, syncAndWaitForBranches)

      const { dialog, withoutReview } = await openWithoutReview(page)
      await withoutReview.hover()
      await expect(page.getByRole("tooltip", { name: "Starts the Safe Push" })).toHaveText(
        "Starts the Safe Push with a merge profile that skips the code-owner review, such as Large Scale Change",
      )
      await withoutReview.click()
      await expect(dialog).toHaveCount(0)

      // the popover at the row's slot, without review: no profile is chosen for the user
      const popover = await expectPopoverWithoutReview(page, "IJPL Code Platform")
      const start = popoverStart(page)
      await expect(start).toHaveText("Safe Push Without Review")
      await expect(start).toBeDisabled()
      await start.click({ force: true })
      expect((await repo.getPatronusLog()).starts).toEqual([])

      // the profiles that may skip the review, the emergency one apart; no default checks
      await popover.getByTestId("safe-push-profile").click()
      await expect(page.getByRole("option")).toHaveText(["Large Scale Change", "Emergency"])
      await page.getByRole("option", { name: "Large Scale Change" }).click()
      await expect(popover.getByTestId("safe-push-profile-description")).toHaveText(LSC.description)
      await expect(popover.getByTestId("safe-push-profile-note")).toHaveText(
        "Patronus tells only when the robot starts whether Large Scale Change skips the code-owner review: if it doesn't, the Safe Push fails at once.",
      )
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await start.click()
      await expect(popover).toHaveCount(0)
      await expectStartedWith(page, repo, "lsc")
    })

    test("should keep the pull request dialog's footer in one line inside the dialog", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRow7(page, setupRepo, syncAndWaitForBranches)
      const { dialog, withoutReview } = await openWithoutReview(page)
      const onGitHub = dialog.getByTestId("create-pull-request-on-github")
      const submit = dialog.getByTestId("create-pull-request-submit")
      const title = dialog.getByTestId("create-pull-request-title")
      for (const element of [withoutReview, onGitHub, submit, title]) {
        await expect(element).toBeVisible()
      }
      // the dialog zooms in as it opens: measure it settled
      await waitForTransitionsSettled(page)

      const box = async (element: Locator) => {
        const result = await element.boundingBox()
        expect(result).not.toBeNull()
        return result!
      }
      const expectInside = async () => {
        const frame = await box(dialog)
        for (const element of [withoutReview, onGitHub, submit, title]) {
          const inner = await box(element)
          expect(inner.x).toBeGreaterThanOrEqual(frame.x)
          expect(inner.x + inner.width).toBeLessThanOrEqual(frame.x + frame.width)
        }
        return frame
      }
      const center = (it: { y: number; height: number }) => it.y + it.height / 2

      // 1280 wide: the dialog is a form, its footer one line
      expect((await expectInside()).width).toBeCloseTo(48 * 16, 0)
      const line = center(await box(withoutReview))
      expect(Math.abs(center(await box(onGitHub)) - line)).toBeLessThanOrEqual(1)
      expect(Math.abs(center(await box(submit)) - line)).toBeLessThanOrEqual(1)

      // the review window at 150%: the dialog keeps its margin, the actions wrap below the button that can't
      await page.setViewportSize({ width: 640, height: 720 })
      await expect.poll(async () => (await box(dialog)).width).toBeLessThanOrEqual(640 - 2 * 16)
      const frame = await expectInside()
      expect(frame.x).toBeGreaterThanOrEqual(16)
      expect(frame.x + frame.width).toBeLessThanOrEqual(640 - 16)
      const button = await box(withoutReview)
      expect((await box(submit)).y).toBeGreaterThanOrEqual(button.y + button.height)
    })

    test("should say that an emergency profile skips all checks, and start with it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRow7(page, setupRepo, syncAndWaitForBranches)

      await (await openWithoutReview(page)).withoutReview.click()
      const popover = safePushPopover(page)
      await expect(popover).toBeVisible()
      await chooseProfile(page, "Emergency")
      const note = popover.getByTestId("safe-push-profile-note")
      await expect(note).toHaveAttribute("data-kind", "emergency")
      await expect(note).toHaveText("Emergency skips all checks of the Safe Push, not only the code-owner review: Patronus pushes into the target without testing it.")
      // the start takes Enter once it can run
      await expect(popoverStart(page)).toBeEnabled()
      await popoverStart(page).focus()
      await page.keyboard.press("Enter")
      await expectStartedWith(page, repo, "emergency")
    })

    test("should start with the profile of the last robot when its options said it skips the review", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // a Large Scale Change that failed on its tests: the approvals still missing don't hold its profile back
      const failed = robot(BRANCH, "lsc-robot", { status: "failure", finishTime: 1704142800, profile: { id: "lsc", name: "Large Scale Change", skipsReview: true } })
      const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [], availableRobots(failed), {
        patronusProfiles: PROFILES,
      })
      // the menu is built when it opens: Patronus has answered with the failed robot first
      await expect(findBranchRow(page, BRANCH).getByTestId("patronus-chip")).toBeVisible()

      // a profile other than the default checks: the menu's Safe Push opens the popover first
      const menu = await openMenu(page)
      await menu.getByRole("menuitem", SAFE_PUSH_ITEM).click()
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await expect(popover.getByTestId("safe-push-profile")).toContainText("Large Scale Change")
      await expect(popover.getByTestId("safe-push-profile-source")).toHaveText(`The profile of the last Safe Push of ${BRANCH}.`)
      await expect(popover.getByTestId("safe-push-review")).toHaveText("Starts without the approvals of IJPL Code Platform: Large Scale Change skips the code-owner review.")
      await popoverStart(page).click()
      await expect(popover).toHaveCount(0)
      await expectStartedWith(page, repo, "lsc")
    })

    test("should not offer it in the dialog of a branch whose Safe Push needs no pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [])
      const menu = await openMenu(page)
      await menu.getByRole("menuitem", { name: "Create Pull Request…" }).click()
      const dialog = createPullRequestDialog(page)
      await expect(dialog.getByTestId("create-pull-request-submit")).toBeVisible()
      await expect(dialog.getByRole("button", { name: "Safe Push Without Review…" })).toHaveCount(0)
    })
  })
})
