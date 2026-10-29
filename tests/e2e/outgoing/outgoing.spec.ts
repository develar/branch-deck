// @spec spec/repository/outgoing.spec.md
import type { Locator, Page } from "@playwright/test"
import type { BranchReviewRequest } from "../../../src/types/sub-window"
import { testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { expectNothingToPush, findBranchRow, getPushButton } from "../helpers/branch-helpers"
import { confirmForcePush, expectPushedToast, expectPushedToRemote, holdRemotePushes, pushedBefore, pushedToast, remotePushes, virtualBranch } from "../helpers/push-helpers"
import { addRecentProject, OTHER_REPOSITORY, switchRepository } from "../helpers/repository-switch-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { getEmittedEvents } from "../helpers/sub-window-helpers"
import { emitTauriEvent, pressSyncHeld, releaseHeldSync, syncFromMenu } from "../helpers/sync-helpers"
import type { RemoteCommit, TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast, holdToast } from "../helpers/toast-helpers"
import { recordCommand } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed; JIRA-456 has 3 commits, ABC-123 4, the others 1

/**
 * The push of JIRA-456 pressed while a sync of the session runs, held until the test releases it (`pressSyncHeld`): the
 * push waits in the session for that sync, its button busy. The row
 */
async function pushDuringHeldSync(page: Page, repo: TestRepositoryBuilder): Promise<Locator> {
  await pressSyncHeld(page, repo)
  const row = findBranchRow(page, "JIRA-456")
  const button = getPushButton(row)
  await button.click()
  await expect(button).toHaveAttribute("aria-busy", "true")
  return row
}

test.describe("Push progress", () => {
  test("should show the percent on the push button and git's last line on hover while the push runs", async ({ page, issueLinksRepo }) => {
    // the push waits at the remote, which has its pack: the progress is git's
    const held = await holdRemotePushes(issueLinksRepo)
    const row = findBranchRow(page, "JIRA-456")
    const button = getPushButton(row)
    // the row shows the count, the label is its name and leads its description
    await expect(button).toHaveText("3")
    await expect(button).toHaveAccessibleName("Push Branch (3)")
    await button.hover()
    await expect(page.getByTestId("push-description")).toContainText("Push Branch (3)")

    // read, then pushed: the click, later than half a second after the hover opened, leaves it open with the progress
    await page.clock.fastForward(1000)
    await button.click()
    await held.arrived()
    await expect(button).toHaveText(/^\d+%$/)
    await expect(button).toHaveAccessibleName(/^Pushing \d+%$/)
    await expect(button).toHaveAttribute("aria-busy", "true")
    // the pointer stays on the button after the click: its popover is open
    await expect(page.getByTestId("push-description")).toHaveAttribute("data-open", "")
    // git's last line says the amount and the rate already: no second line repeats them
    await expect(page.getByTestId("push-progress")).toHaveText(/^Writing objects: 100% \(\d+\/\d+\), [^|]+ \| [^|]+\/s, done\.$/)

    await held.release()
    await expectPushedToast(page, "JIRA-456")
    await expectPushedToRemote(issueLinksRepo, ["JIRA-456"])
    // done: the button gives way to its empty place, the status says the branch is up to date; the focus it had goes to
    // the row, which keeps its keys
    await expectNothingToPush(row)
    await expect(row).toBeFocused()
  })
})

/** A commit someone else pushed to the remote branch, three hours before the fixed clock of the page, writing `file` */
function theirCommit(subject: string, author: string, file: string): RemoteCommit {
  return { subject, author: `${author} <${author.toLowerCase()}@example.com>`, timestamp: Date.parse("2024-01-02T07:00:00Z") / 1000, files: { [file]: `${subject}\n` } }
}

test.describe("Force push", () => {
  test("should force push a branch the remote has commits for, leased on the remote head the row shows", async ({ page, issueLinksRepo }) => {
    // ABC-123 was pushed without its newest commit, then someone else pushed a commit it lacks, which a fetch brought
    await issueLinksRepo.pushInTerminal(virtualBranch("ABC-123"), { revision: `${virtualBranch("ABC-123")}~1` })
    const theirs = await issueLinksRepo.commitOnRemote(virtualBranch("ABC-123"), theirCommit("Fix login", "Ann", "login.txt"))
    await issueLinksRepo.fetchInTerminal()
    await syncFromMenu(page)
    const row = findBranchRow(page, "ABC-123")
    const forcePush = row.getByRole("button", { name: "Force Push Branch (1)" })
    await expect(forcePush).toBeEnabled()
    // the description says what the force push does, then lists what it drops
    await forcePush.hover()
    await expect(page.getByText("Force push replaces the remote branch. 1 commit to push. The remote has 1 commit your branch doesn't (by Ann).")).toBeVisible()
    const dropped = page.getByTestId("dropped-commits")
    await expect(dropped).toContainText("Fix login")
    await expect(dropped).toContainText(`Ann · ${theirs.slice(0, 8)} · 3 hours ago`)
    // only the hover: no confirm before the click
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)

    // it drops Ann's commit: the click asks first, the Force Push in the description pushes
    await forcePush.click()
    await expect(forcePush).toHaveAttribute("aria-expanded", "true")
    expect(await remotePushes(issueLinksRepo)).toEqual([])
    await confirmForcePush(page, "Force Push Branch (1)")

    // one push, leased on the remote head the row shows: the remote took it over Ann's commit
    await expectPushedToRemote(issueLinksRepo, ["ABC-123"])
    expect(await remotePushes(issueLinksRepo)).toEqual([["ABC-123"]])
    await expectNothingToPush(row)
  })

  test("should show what the remote has when it moved since the status, and force push over it from the toast", async ({ page, issueLinksRepo }) => {
    // the remote branch of JIRA-456 has none of its commits; three commits of others came onto it since the sync
    await pushedBefore(page, issueLinksRepo, ["JIRA-456"], { lacking: 3 })
    for (const [subject, author] of [
      ["Fix A", "Ann"],
      ["Fix B", "Bob"],
      ["Fix C", "Ann"],
    ] as const) {
      await issueLinksRepo.commitOnRemote(virtualBranch("JIRA-456"), theirCommit(subject, author, `${subject}.txt`))
    }
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()

    const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
    await expect(toast).toContainText("3 commits you don't have (by Ann and Bob). Nothing was pushed.")
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    expect(await remotePushes(issueLinksRepo)).toEqual([])
    // the row shows the fetched remote: a force push that drops the 3 commits
    await expect(row.getByRole("button", { name: "Force Push Branch (3)" })).toBeEnabled()
    await expect(row).toContainText("3↓")

    // the push drops the 3 commits: the toast opens the confirm of the row's push button
    await toast.getByRole("button", { name: "Force Push Branch" }).click()
    await expect(toast).toHaveCount(0)
    expect(await remotePushes(issueLinksRepo)).toEqual([])
    await confirmForcePush(page, "Force Push Branch (3)")

    await expectPushedToast(page, "JIRA-456")
    // leased on what the fetch brought: the remote took it over the 3 commits
    await expectPushedToRemote(issueLinksRepo, ["JIRA-456"])
    expect(await remotePushes(issueLinksRepo)).toEqual([["JIRA-456"]])
  })

  test("should push a rebased branch at once: its force push drops nothing", async ({ page, issueLinksRepo }) => {
    // the remote has the commits of ABC-123 but its newest as an earlier build: the sync rebuilt them since
    await pushedBefore(page, issueLinksRepo, ["ABC-123"], { lacking: 1, earlierBuild: true })
    const row = findBranchRow(page, "ABC-123")
    const forcePush = row.getByRole("button", { name: "Force Push Branch (1)" })
    await expect(forcePush).toBeEnabled()

    await forcePush.click()

    await expectPushedToRemote(issueLinksRepo, ["ABC-123"])
    expect(await remotePushes(issueLinksRepo)).toEqual([["ABC-123"]])
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)
  })
})

test.describe("Push toast", () => {
  test("should tell a later push of the branch in the toast of the first, in place: one toast per branch", async ({ page, issueLinksRepo }) => {
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()
    const toast = await holdToast(pushedToast(page, "JIRA-456"))
    await expect(toast.locator("[data-slot=toast-description]")).toContainText("created the remote branch")
    // the pointer rests on the toast while the user reads it: its timer waits
    await toast.hover()

    // a commit of JIRA-456 made since, which the sync of the menu brings into the row: P on the focused row pushes it
    await issueLinksRepo.commit("(JIRA-456) Raise the pool size", { "pool.cfg": "size=50\n" }, 1704182460)
    await syncFromMenu(page)
    await expect(getPushButton(row)).toHaveAccessibleName("Push Branch (1)")
    await row.focus()
    await page.keyboard.press("p")
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("1 commit")
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
  })
})

test.describe("Push button", () => {
  test("should keep the colour of the push while its description is open", async ({ page }) => {
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await expect(button).toHaveAccessibleName("Push Branch (3)")
    await button.hover()
    await expect(page.getByTestId("push-description")).toBeVisible()
    await expect(button).toHaveAttribute("aria-expanded", "true")

    // the ghost variant turns an open trigger to the foreground; a routine push keeps its hue in the icon and the tint
    const colours = () =>
      button.evaluate(element => {
        const probe = document.createElement("span")
        document.body.append(probe)
        const styleOf = (className: string) => {
          probe.className = className
          const style = getComputedStyle(probe)
          return { color: style.color, background: style.backgroundColor }
        }
        const result = {
          label: getComputedStyle(element).color,
          icon: getComputedStyle(element.querySelector("svg")!).color,
          background: getComputedStyle(element).backgroundColor,
          primary: styleOf("text-primary").color,
          foreground: styleOf("text-foreground").color,
          tint: styleOf("bg-primary/10").background,
        }
        probe.remove()
        return result
      })
    await expect
      .poll(async () => {
        const { label, icon, background, primary, foreground, tint } = await colours()
        return icon === primary && label === foreground && background === tint
      })
      .toBe(true)
  })

  test("should push a branch pressed during a sync once, over the remote tip the sync brought, the row as it was meanwhile", async ({ page, issueLinksRepo }) => {
    const pushes = await recordCommand(page, "execute")
    // the sync of the session runs until the test releases it
    await issueLinksRepo.holdRemoteAnswers({ hold: ["sync"] })
    // another machine pushed an ancestor of JIRA-456, which the next sync finds on the remote
    const movedTip = await issueLinksRepo.pushInTerminal(virtualBranch("JIRA-456"), { revision: `${virtualBranch("JIRA-456")}~3` })
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    const row = findBranchRow(page, "JIRA-456")
    // no "syncing…" on the row, no disabled push: a push waits for the sync instead
    await expect(row.getByTestId("branch-status")).toHaveText("not pushed")
    const button = getPushButton(row)
    await expect(button).toBeEnabled()
    await button.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    // the push waits in the session for its sync: the remote has nothing of it yet
    await expect.poll(() => pushes.length).toBe(1)
    expect(await remotePushes(issueLinksRepo)).toEqual([])

    await issueLinksRepo.holdRemoteAnswers({ release: ["sync"] })
    await expectPushedToast(page, "JIRA-456")
    // one push, over the tip the sync brought: the remote took the branch once
    expect(pushes).toEqual([expect.objectContaining({ intent: expect.objectContaining({ kind: "push", subject: { kind: "branch", name: "JIRA-456" } }) })])
    expect(await remotePushes(issueLinksRepo)).toEqual([["JIRA-456"]])
    expect(await issueLinksRepo.revision(`origin/${virtualBranch("JIRA-456")}`)).not.toBe(movedTip)
    // the row has the status the push left from the session, with no sync after it
    await expectNothingToPush(row)
  })

  test("should refuse a push pressed during a sync that could not build the branch, its virtual branch an older build", async ({ page, issueLinksRepo }) => {
    // behind the app's back: a commit of JIRA-456 and one of ABC-123 that need a commit of another branch, so the next sync
    // builds neither, and their virtual branches stay builds of the commits before
    await issueLinksRepo.commit("(pool) Add the pool config", { "pool.cfg": "size=10\n", "auth.cfg": "ttl=1\n" }, 1704182400)
    await issueLinksRepo.commit("(JIRA-456) Raise the pool size", { "pool.cfg": "size=50\n" }, 1704182460)
    await issueLinksRepo.commit("(ABC-123) Raise the session ttl", { "auth.cfg": "ttl=2\n" }, 1704182520)
    await issueLinksRepo.holdRemoteAnswers({ hold: ["sync"] })
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    // the rows as they were: both pushes wait for the sync of the session, the button and P
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await button.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    const abc = findBranchRow(page, "ABC-123")
    await abc.focus()
    await page.keyboard.press("p")
    await expect(getPushButton(abc)).toHaveAttribute("aria-busy", "true")

    await issueLinksRepo.holdRemoteAnswers({ release: ["sync"] })
    // the backend refuses both: what they would push is not what the rows show
    await expect(page.getByText("Nothing was pushed: JIRA-456 is not synced (a merge conflict)")).toBeVisible()
    await expect(page.getByText("Nothing was pushed: ABC-123 is not synced (a merge conflict)")).toBeVisible()
  })

  test("should push a branch pressed again during a sync once after it, in one toast", async ({ page, issueLinksRepo }) => {
    const pushes = await recordCommand(page, "execute")
    const row = await pushDuringHeldSync(page, issueLinksRepo)
    expect(pushes).toHaveLength(1)
    // P on the row asks for the same push while the first waits for the sync: the push of the row runs, so the press
    // starts no second one
    await row.focus()
    await page.keyboard.press("p")
    await expect(getPushButton(row)).toHaveAttribute("aria-busy", "true")

    await issueLinksRepo.holdRemoteAnswers({ release: ["sync"] })
    await expectPushedToast(page, "JIRA-456")
    expect(await remotePushes(issueLinksRepo)).toEqual([["JIRA-456"]])
    expect(pushes).toHaveLength(1)
    await expectNothingToPush(row)
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
  })

  test("should push a branch in its repository when another repository opens while its push sends", async ({ page, issueLinksRepo, syncAndWaitForBranches }) => {
    await addRecentProject(page)
    await syncAndWaitForBranches()
    const held = await holdRemotePushes(issueLinksRepo)
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()
    await held.arrived()
    // the window opens another repository while the push sends: the push goes on in its own
    await switchRepository(page, OTHER_REPOSITORY)
    await held.release()
    await expect.poll(() => remotePushes(issueLinksRepo)).toEqual([["JIRA-456"]])
    await expect(pushedToast(page, "JIRA-456")).toBeVisible()
    await expect(page.getByText("Cannot push branch JIRA-456")).toHaveCount(0)
    // back in it, the row has the status the push left in its session
    await switchRepository(page, issueLinksRepo.path)
    await expectNothingToPush(row)
  })

  test("should push a branch a review window asks for during a sync once that sync ended, in the main window", async ({ page, issueLinksRepo }) => {
    const previousCount = await pressSyncHeld(page, issueLinksRepo)
    // the review window of JIRA-456 asks the main window for the push of its menu: the main window runs it on the
    // table of the sync that runs, once that sync ended
    const request = { id: "window", repositoryPath: issueLinksRepo.path, branchName: "JIRA-456", menuItemId: "push" } satisfies BranchReviewRequest
    await emitTauriEvent(page, "branch-review/request", request)
    await releaseHeldSync(page, issueLinksRepo, previousCount)
    await expect.poll(() => getEmittedEvents(page, "branch-review/reply")).toEqual([{ id: "window", ok: true }])
    await expectPushedToast(page, "JIRA-456")
    expect(await remotePushes(issueLinksRepo)).toEqual([["JIRA-456"]])
    await expectNothingToPush(findBranchRow(page, "JIRA-456"))
  })

  test("should describe a disabled push on hover", async ({ page }) => {
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()
    await expectPushedToast(page, "JIRA-456")
    await expectNothingToPush(row)
    // read: closed, it would cover the button of the preview
    await closeToast(pushedToast(page, "JIRA-456"))

    // the preview keeps the labelled button: disabled, it says why on hover
    const preview = await openBranchPreview(page, "JIRA-456")
    const button = preview.getByTestId("push-button")
    await expect(button).toBeDisabled()
    await button.hover()
    await expect(page.getByTestId("push-description")).toContainText("Everything is pushed.")
  })
})

test.describe("Push size", () => {
  test("should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "landing" })
    const setting = window.getByTestId("setting-pushSizeLimit")
    const field = setting.getByRole("textbox", { name: "Push size limit" })
    const source = setting.getByTestId("setting-pushSizeLimit-source")
    await expect(source).toHaveText("50M by default. A change goes to your global Git config, which bd reads too")
    await expect(field).toHaveAttribute("placeholder", "50M")
    // typing the backend would refuse is refused in place
    await field.fill("lots")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await expect(setting.getByTestId("setting-pushSizeLimit-typing")).toHaveText("lots is not a size such as 50M or 1G, nor 0 or off")
    await field.fill("1")
    await field.press("Enter")
    await expect(source).toHaveText("Set in your global Git config, which bd reads too")

    // the push of the main window reads the limit as it starts: refused before it talks to the remote
    const row = findBranchRow(page, "feature-auth")
    const push = getPushButton(row)
    await push.click()
    const toast = page.locator("[data-slot=toast]", { hasText: "Cannot push branch feature-auth" })
    await expect(toast).toContainText(/^Cannot push branch feature-authThe push would add .+, over the push size limit of 1 B/)
    await expect(toast).toContainText(
      "Nothing was pushed. Remove the files from the commits (build output?), or raise the push size limit (branchdeck.pushSizeLimit, Settings → Landing)",
    )
    await expect(row.getByTestId("branch-status")).toHaveText("not pushed")
    await closeToast(toast)

    await field.fill("off")
    await field.press("Enter")
    await expect(field).toHaveValue("off")
    await push.click()
    await expectPushedToast(page, "feature-auth")
  })
})
