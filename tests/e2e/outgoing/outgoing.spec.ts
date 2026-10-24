// @spec spec/repository/outgoing.spec.md
import { testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { expectNothingToPush, findBranchRow, getPushButton } from "../helpers/branch-helpers"
import type { DroppedCommit } from "../helpers/push-helpers"
import { confirmForcePush, expectPushedToast, pushedToast, routePush } from "../helpers/push-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { closeToast } from "../helpers/toast-helpers"
import { holdCommand } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed; JIRA-456 has 3 commits, ABC-123 4, the others 1

test.describe("Push progress", () => {
  test("should show the percent on the push button and git's last line on hover while the push runs", async ({ page }) => {
    let release!: () => void
    await routePush(page, {
      "JIRA-456": {
        progress: [
          { percent: 15, transferred: null, speed: null, line: "Compressing objects: 100% (3/3), done." },
          { percent: 55, transferred: "1.23 MiB", speed: "2.40 MiB/s", line: "Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s" },
        ],
        release: new Promise(resolve => (release = resolve)),
      },
    })
    const row = findBranchRow(page, "JIRA-456")
    const button = getPushButton(row)
    // the row shows the count, the label is its name and leads its description
    await expect(button).toHaveText("3")
    await expect(button).toHaveAccessibleName("Push (3)")

    await button.click()
    await expect(button).toHaveText("55%")
    await expect(button).toHaveAccessibleName("Pushing 55%")
    await expect(button).toHaveAttribute("aria-busy", "true")
    // the pointer stays on the button after the click: its popover is open
    const progress = page.getByTestId("push-progress")
    await expect(progress).toContainText("Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s")
    // git's line says the amount already: no second line repeats it
    await expect(progress).not.toContainText("1.23 MiB · 2.40 MiB/s")

    release()
    await expectPushedToast(page, "JIRA-456")
    // done: the button gives way to its empty place, the status says the branch is up to date; the focus it had goes to
    // the row, which keeps its keys
    await expectNothingToPush(row)
    await expect(row).toBeFocused()
  })
})

/** A commit someone else pushed to the remote branch, three hours before the fixed clock of the page */
function theirCommit(id: string, subject: string, author: string): DroppedCommit {
  return { id, subject, author, authorTime: Date.parse("2024-01-02T07:00:00Z") / 1000 }
}

test.describe("Force push", () => {
  test("should force push a branch the remote has commits for, leased on the remote head the row shows", async ({ page }) => {
    const requests = await routePush(page, {
      "ABC-123": [
        // someone else pushed a commit the branch lacks
        { unpushedCommits: ["a1"], commitsBehind: 1, remoteTip: "remote-head-1", droppedCommits: [theirCommit("c0ffee0123456789", "Fix login", "Ann")] },
        {},
      ],
    })
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    const forcePush = row.getByRole("button", { name: "Force Push (1)" })
    await expect(forcePush).toBeEnabled()
    // the description says what the force push does, then lists what it drops
    await forcePush.hover()
    await expect(page.getByText("Force push replaces the remote branch. 1 commit to push. The remote has 1 commit your branch doesn't (by Ann).")).toBeVisible()
    const dropped = page.getByTestId("dropped-commits")
    await expect(dropped).toContainText("Fix login")
    await expect(dropped).toContainText("Ann · c0ffee01 · 3 hours ago")
    // only the hover: no confirm before the click
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)

    // it drops Ann's commit: the click asks first, the Force Push in the description pushes
    await forcePush.click()
    await expect(forcePush).toHaveAttribute("aria-expanded", "true")
    expect(requests).toHaveLength(1)
    await confirmForcePush(page, "Force Push (1)")

    await expect.poll(() => requests.map(request => request.expectedRemote)).toEqual([null, "remote-head-1"])
    await expectNothingToPush(row)
  })

  test("should show what the remote has when it moved since the status, and force push over it from the toast", async ({ page }) => {
    const requests = await routePush(page, {
      "JIRA-456": [
        {
          outcome: "remoteMoved",
          unpushedCommits: ["j1", "j2", "j3"],
          commitsBehind: 3,
          remoteTip: "moved-head",
          droppedCommits: [theirCommit("aaaa000011112222", "Fix A", "Ann"), theirCommit("bbbb000011112222", "Fix B", "Bob"), theirCommit("cccc000011112222", "Fix C", "Ann")],
        },
        {},
      ],
    })
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()

    const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
    await expect(toast).toContainText("3 commits you don't have (by Ann and Bob). Nothing was pushed.")
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    // the row shows the fetched remote: a force push that drops the 3 commits
    await expect(row.getByRole("button", { name: "Force Push (3)" })).toBeEnabled()
    await expect(row).toContainText("3↓")

    // the push drops the 3 commits: the toast opens the confirm of the row's push button
    await toast.getByRole("button", { name: "Force Push" }).click()
    await expect(toast).toHaveCount(0)
    expect(requests).toHaveLength(1)
    await confirmForcePush(page, "Force Push (3)")

    await expectPushedToast(page, "JIRA-456")
    expect(requests.map(request => request.expectedRemote)).toEqual([null, "moved-head"])
  })

  test("should push a rebased branch at once: its force push drops nothing", async ({ page }) => {
    const requests = await routePush(page, {
      // the remote has the commits as they were before the rebase onto a newer baseline
      "ABC-123": [{ unpushedCommits: ["a1"], fastForward: false }, {}],
    })
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    const forcePush = row.getByRole("button", { name: "Force Push (1)" })
    await expect(forcePush).toBeEnabled()

    await forcePush.click()

    await expect.poll(() => requests).toHaveLength(2)
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)
  })
})

test.describe("Push button", () => {
  test("should keep the colour of the push while its description is open", async ({ page }) => {
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await expect(button).toHaveAccessibleName("Push (3)")
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

  test("should push a branch pressed during a sync once the sync finished, the row as it was meanwhile", async ({ page }) => {
    const pushes = await routePush(page, {})
    const releaseSync = await holdCommand(page, "sync")
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    const row = findBranchRow(page, "JIRA-456")
    // no "syncing…" on the row, no disabled push: a push waits for the sync instead
    await expect(row.getByTestId("branch-status")).toHaveText("not pushed")
    const button = getPushButton(row)
    await expect(button).toBeEnabled()
    await button.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    expect(pushes).toHaveLength(0)

    releaseSync()
    await expectPushedToast(page, "JIRA-456")
    expect(pushes.map(it => it.branchName)).toEqual(["JIRA-456"])
  })

  test("should refuse a push pressed during a sync that could not build the branch, its virtual branch an older build", async ({ page, issueLinksRepo }) => {
    // behind the app's back: a commit of JIRA-456 and one of ABC-123 that need a commit of another branch, so the next sync
    // builds neither, and their virtual branches stay builds of the commits before
    await issueLinksRepo.commit("(pool) Add the pool config", { "pool.cfg": "size=10\n", "auth.cfg": "ttl=1\n" }, 1704182400)
    await issueLinksRepo.commit("(JIRA-456) Raise the pool size", { "pool.cfg": "size=50\n" }, 1704182460)
    await issueLinksRepo.commit("(ABC-123) Raise the session ttl", { "auth.cfg": "ttl=2\n" }, 1704182520)
    const releaseSync = await holdCommand(page, "sync")
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    // the rows as they were: both pushes wait for the sync, the button and P
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await button.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    const abc = findBranchRow(page, "ABC-123")
    await abc.focus()
    await page.keyboard.press("p")
    await expect(getPushButton(abc)).toHaveAttribute("aria-busy", "true")

    releaseSync()
    // the backend refuses both: what they would push is not what the rows show
    await expect(page.getByText("Nothing was pushed: JIRA-456 is not synced (a merge conflict)")).toBeVisible()
    await expect(page.getByText("Nothing was pushed: ABC-123 is not synced (a merge conflict)")).toBeVisible()
  })

  test("should describe a disabled push on hover", async ({ page }) => {
    await routePush(page, {})
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
