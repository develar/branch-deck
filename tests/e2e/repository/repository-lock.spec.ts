// @spec openspec/specs/branch/move-commits/spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import {
  expandBranch,
  expectNothingToPush,
  findBranchRow,
  getBranchCommitRow,
  getBranchDetailsRow,
  getMoreButton,
  getPushButton,
  verifyCommitCount,
} from "../helpers/branch-helpers"
import { boxOf } from "../helpers/header-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { editCommitMessage } from "../helpers/preview-helpers"
import { expectPushedToast, remotePushes } from "../helpers/push-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, openSubmenu, selectCommit } from "../helpers/selection-helpers"
import { runAndWaitForSync, syncFromMenu, runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { animationsEnded, recordCommand, recordRewrites, expectClosed, openTooltip, waitForStepFacts } from "../helpers/wait-helpers"

const ISSUE = "ABC-123"

/** Whom a rewrite or a push waits for while another process holds the repository lock: here the test server */
const WAITING = /^Waiting for `[^`]+` \(pid \d+, for \d+ s\)/

/**
 * The same, as a screen reader is told it once per holder: without the time, which grows every second, and without the
 * backticks around the command line, which a screen reader reads aloud
 */
const WAITING_FOR = /^Waiting for [^`]+ \(pid \d+\)$/

/** The `issue_split` repository synced, its issue branch expanded */
async function setup(page: Page, setupRepo: (template: string) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_split")
  await syncAndWaitForBranches()
  await verifyCommitCount(findBranchRow(page, ISSUE), 3)
  // the menus a test opens show what the facts of the first sync decided, and keep their items under the pointer
  await waitForStepFacts(page.getByTestId("next-step-slot"))
  await expandBranch(page, findBranchRow(page, ISSUE))
  return repo
}

/** Opens the squash dialog of "Add tokens" and "Add parser"; the dialog, its message focused */
async function openSquashOfTwo(page: Page): Promise<Locator> {
  await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
  await multiSelectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))
  await getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions").getByRole("button", { name: "Squash" }).click()
  const dialog = page.getByTestId("squash-dialog")
  await expect(dialog.getByTestId("squash-message-editor")).toBeFocused()
  return dialog
}

/**
 * Opens the squash dialog of "Add tokens" and "Add parser" and presses Squash by key while another process holds the
 * lock (`brd pick --follow` of an agent: here the test server, whose record names it); the dialog, and its notice of whom
 * the squash waits for, there once the squash reached the backend, where it waits without a bound while the holder lives
 */
async function squashTwoWhileHeld(page: Page, repo: TestRepositoryBuilder): Promise<{ dialog: Locator; notice: Locator }> {
  await repo.holdRepositoryLock(true)
  const dialog = await openSquashOfTwo(page)
  await page.keyboard.press("ControlOrMeta+Enter")
  const notice = dialog.getByTestId("lock-wait-notice")
  await expect(notice).toContainText(WAITING)
  return { dialog, notice }
}

/** Opens Drop Commit… of "Add parser" of the issue branch, the lock held; its form and its Drop Commit */
async function openDropWhileHeld(page: Page, repo: TestRepositoryBuilder): Promise<{ form: Locator; drop: Locator }> {
  await repo.holdRepositoryLock(true)
  await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
  await clickContextMenuItem(page, "Drop Commit…")
  const form = page.getByTestId("drop-commits-form")
  return { form, drop: form.getByRole("button", { name: "Drop Commit" }) }
}

/** Opens Move to… of "Add parser" of the issue branch and types "docs", the lock held; its input, focused, and the form */
async function openMoveToDocsWhileHeld(page: Page, repo: TestRepositoryBuilder): Promise<{ input: Locator; form: Locator }> {
  await repo.holdRepositoryLock(true)
  await selectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))
  await getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions").getByRole("button", { name: "Move to…" }).click()
  const input = inlineBranchCreator.getInput(page)
  await expect(input).toBeFocused()
  await page.keyboard.type("docs")
  return { input, form: inlineBranchCreator.getPortal(page) }
}

/** Moves "Add parser" of the issue branch to docs through Move to Existing of its context menu */
async function moveParserToDocsFromMenu(page: Page): Promise<void> {
  await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
  await openSubmenu(page, "Move to Existing")
  await clickContextMenuItem(page, "docs")
}

test.describe("Rewrites while another process holds the repository lock", () => {
  test("should wait for the holder in the squash dialog, say whom for, and squash once it released the lock", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const squashes = await recordRewrites(page, "squash")
    const { dialog, notice } = await squashTwoWhileHeld(page, repo)
    await expect(notice.getByRole("button", { name: "Stop Waiting" })).toBeVisible()
    expect(squashes).toHaveLength(1)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
    await expect(page.getByText("Cannot squash the commits")).toHaveCount(0)

    await runAndWaitForAnswer(page, "execute", () => repo.holdRepositoryLock(false))
    await expect(page.getByText("Squashed 2 commits")).toBeVisible()
    await expectClosed(dialog)
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)
  })

  test(
    "should stop the wait of a squash with Stop Waiting in its dialog, squash nothing, and squash on the next press",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      const { dialog, notice } = await squashTwoWhileHeld(page, repo)

      // the press by key left the focus nowhere (the message became disabled): Stop Waiting took it, Space stops the wait,
      // and the focus goes to Squash, which can be pressed again
      await expect(notice.getByRole("button", { name: "Stop Waiting" })).toBeFocused()
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Space"))
      await expect(page.getByText("Canceled squashing 2 commits before it started")).toBeVisible()
      await expect(notice).toHaveCount(0)
      await expect(page.getByText("Cannot squash the commits")).toHaveCount(0)
      const squash = dialog.getByRole("button", { name: /Squash/ })
      await expect(squash).toBeFocused()
      // nothing changed, and the dialog stays with its message: the next press squashes, once the holder is done
      await verifyCommitCount(findBranchRow(page, ISSUE), 3)
      await repo.holdRepositoryLock(false)
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await expect(page.getByText("Squashed 2 commits")).toBeVisible()
      await verifyCommitCount(findBranchRow(page, ISSUE), 2)
    },
  )

  test(
    "should keep the focus on the row's ⋯ through a rewrite started from its menu by key that waits for the lock, and stop it by key",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // oauth stacked on login: Unstack from login of its menu, a rewrite, waits for the lock another process holds
      const repo = await setupRepo("stacked_branches")
      await syncAndWaitForBranches()
      const unstacks = await recordRewrites(page, "unstack")
      const cancels = await recordCommand(page, "cancel_operation")
      await repo.holdRepositoryLock(true)
      const more = getMoreButton(findBranchRow(page, "oauth"))
      await more.focus()
      await expect(more).toBeFocused()

      // Enter opens its menu, the arrows go to the item, Enter runs it: the menu gives the focus back to the "⋯"
      await page.keyboard.press("Enter")
      const unstack = page.getByRole("menuitem", { name: "Unstack from login" })
      await expect(unstack).toBeVisible()
      for (let step = 0; step < 20 && !(await unstack.evaluate(item => item === document.activeElement)); step++) {
        await page.keyboard.press("ArrowDown")
      }
      await expect(unstack).toBeFocused()
      await page.keyboard.press("Enter")
      await expectClosed(page.getByRole("menu"))
      // the spinner, then the wait with its Stop Waiting: one button through both, the focus on it
      await expect(more).toHaveAttribute("aria-busy", "true")
      await expect(more).not.toHaveAttribute("aria-disabled", "true")
      await expect(more).toBeFocused()
      expect(unstacks).toHaveLength(1)

      // Enter opens Stop Waiting, Escape closes it and gives the focus back
      await page.keyboard.press("Enter")
      const stop = page.getByRole("menuitem", { name: "Stop Waiting" })
      await expect(stop).toBeVisible()
      await page.keyboard.press("Escape")
      await expectClosed(stop)
      await expect(more).toBeFocused()
      // Enter and Enter: Stop Waiting ends the wait with nothing changed, the focus on the "⋯" again
      await page.keyboard.press("Enter")
      await expect(stop).toBeFocused()
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await expect(page.getByText(/^Canceled unstacking oauth/)).toBeVisible()
      expect(cancels).toHaveLength(1)
      await expect(more).not.toHaveAttribute("aria-busy", "true")
      // the menu gives the focus back as it unmounts, after its exit
      await expectClosed(page.getByRole("menu"))
      await expect(more).toBeFocused()
    },
  )

  test(
    "should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      const moves = await recordRewrites(page, "move")
      await repo.holdRepositoryLock(true)
      await moveParserToDocsFromMenu(page)

      // a start from a menu shows on the "⋯" of its row: the spinner, whom it waits for on hover, Stop Waiting on a click
      const more = getMoreButton(findBranchRow(page, ISSUE))
      await expect(more).toHaveAttribute("aria-busy", "true")
      await more.hover()
      await expect(openTooltip(page)).toHaveText(WAITING)
      expect(moves).toHaveLength(1)
      const cancels = await recordCommand(page, "cancel_operation")
      await more.click()
      await runAndWaitForAnswer(page, "execute", () => page.getByRole("menuitem", { name: "Stop Waiting" }).click())
      await expect(page.getByText("Canceled moving 1 commit to docs before it started")).toBeVisible()
      // the stop is the cancel of the operation of the session
      expect(cancels).toHaveLength(1)
      await expect(more).not.toHaveAttribute("aria-busy", "true")
      await verifyCommitCount(findBranchRow(page, "docs"), 1)
      await verifyCommitCount(findBranchRow(page, ISSUE), 3)

      // the same move once the holder is done: it runs at once
      await repo.holdRepositoryLock(false)
      await runAndWaitForAnswer(page, "execute", () => moveParserToDocsFromMenu(page))
      await verifyCommitCount(findBranchRow(page, "docs"), 2)
      await verifyCommitCount(findBranchRow(page, ISSUE), 2)
      expect(moves).toHaveLength(2)
    },
  )

  test("should say whom a push waits for in the hover of its button, whose Stop Waiting pushes nothing", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const pushes = await recordCommand(page, "execute")
    await repo.holdRepositoryLock(true)
    const push = getPushButton(findBranchRow(page, "feature-auth"))
    await push.click()
    await expect(push).toHaveAttribute("aria-busy", "true")
    // the hover of the push says whom it waits for and offers Stop Waiting
    await push.hover()
    const description = page.getByTestId("push-description")
    await expect(description.getByTestId("push-progress")).toContainText(WAITING)
    await runAndWaitForAnswer(page, "execute", () => description.getByRole("button", { name: "Stop Waiting" }).click())
    await expect(page.getByText("Canceled the push of branch feature-auth before it started")).toBeVisible()
    await expect(push).not.toHaveAttribute("aria-busy", "true")
    expect(pushes).toHaveLength(1)
    await expect(page.getByText("Cannot push branch feature-auth")).toHaveCount(0)
    // nothing was pushed: the button offers the same push
    await expect(push).toBeEnabled()
    await repo.holdRepositoryLock(false)
  })

  // the push gives the ghost button a hover of its colour (`hover:bg-primary/10` of `ghostButtonColorClass`): busy, it
  // takes none, and keeps its background at rest, which only the caller knows
  test("should give a busy push no hover of its colour, its background at rest kept", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    await repo.holdRepositoryLock(true)
    const push = getPushButton(findBranchRow(page, "feature-auth"))
    await push.click()
    await expect(push).toHaveAttribute("aria-busy", "true")
    await expect(push).toHaveAttribute("aria-disabled", "true")
    // away from the button, its description closed: a hover then is the hover alone, the timers of the page stopped so
    // that it opens no description (an open one tints the button by `aria-expanded`)
    await push.evaluate(button => (button as HTMLElement).blur())
    await page.mouse.move(0, 0)
    await expect(push).toHaveAttribute("aria-expanded", "false")
    await page.addStyleTag({ content: "*, *::before, *::after { transition: none !important; }" })
    const background = () => push.evaluate(button => getComputedStyle(button).backgroundColor)
    const rest = await background()
    await page.clock.pauseAt(new Date((await page.evaluate(() => Date.now())) + 1000))
    await push.hover()
    expect(await background()).toEqual(rest)
    await expect(push).toHaveAttribute("aria-expanded", "false")
    await page.clock.resume()
    await runAndWaitForAnswer(page, "execute", () => repo.holdRepositoryLock(false))
  })

  test("should sync while another process holds the repository lock, and push once it released it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    // an agent's `brd push` holds the lock (its sync and its freeze) after a commit behind the app's back
    await repo.commit("(JIRA-456) Raise the pool size", { "pool.cfg": "size=50\n" }, 1704182460)
    await repo.holdRepositoryLock(true)
    // the sync of the app takes no lock: it ends while the other process holds it, and brings the commit into the row
    await runAndWaitForSync(page, () => syncFromMenu(page))
    const row = findBranchRow(page, "JIRA-456")
    const push = getPushButton(row)
    await expect(push).toHaveAccessibleName("Push Branch (4)")
    // the push meets the lock where it writes: it waits for the holder, nothing sent
    await push.click()
    await expect(push).toHaveAttribute("aria-busy", "true")
    await push.hover()
    await expect(page.getByTestId("push-description").getByTestId("push-progress")).toContainText(WAITING)
    expect(await remotePushes(repo)).toEqual([])

    await runAndWaitForAnswer(page, "execute", () => repo.holdRepositoryLock(false))
    await expectPushedToast(page, "JIRA-456")
    expect(await remotePushes(repo)).toEqual([["JIRA-456"]])
    await expectNothingToPush(row)
  })
})

/**
 * `button` of `form` pressed with the pointer while another process holds the repository lock: whom the rewrite waits
 * for comes in `form` where it moves the button from under the pointer by nothing (`LockWaitNotice`, on the line of the
 * buttons before them, or below them; a line of its own above them moved them), a screen reader is told it (the live
 * region of `form`), and its Stop Waiting, which took the focus the press left on the button, stops the rewrite by Enter
 * and gives the focus back. The button keeps its box, or with `grows` (a spinner that came where it had no icon) at least
 * the point the pointer pressed, its middle: which way it grows depends on the room of the line (a narrow window, an
 * input not at its widest)
 */
async function expectWaitBesideItsButton(page: Page, form: Locator, button: Locator, canceled: string, { grows = false }: { grows?: boolean } = {}): Promise<void> {
  await animationsEnded(form)
  const before = await boxOf(button)
  const cancels = await recordCommand(page, "cancel_operation")
  await button.click()
  const notice = form.getByTestId("lock-wait-notice")
  await expect(notice).toContainText(WAITING)
  const status = form.getByTestId("lock-wait-status")
  await expect(status).toHaveRole("status")
  await expect(status).toHaveText(WAITING_FOR)
  const after = await boxOf(button)
  if (grows) {
    const pressed = { x: before.x + before.width / 2, y: before.y + before.height / 2 }
    expect(
      pressed.x >= after.x && pressed.x <= after.x + after.width && pressed.y >= after.y && pressed.y <= after.y + after.height,
      `${JSON.stringify(pressed)} in ${JSON.stringify(after)}`,
    ).toBe(true)
  } else {
    expect(after).toEqual(before)
  }
  // Stop Waiting took the focus as the wait came: from the button, which a press of the pointer focuses in Chromium and
  // which stays focusable while busy, or from nowhere in WebKit, whose pointer focuses no button; back after the stop to
  // the button the press left busy, in both
  await expect(notice.getByRole("button", { name: "Stop Waiting" })).toBeFocused()
  const leaves = await focusAsStopWaitingLeaves(page)
  await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
  await expect(page.getByText(canceled)).toBeVisible()
  expect(cancels).toHaveLength(1)
  await expect(button).toBeFocused()
  await expect(status).toBeEmpty()
  // the focus went from Stop Waiting to the button at once, never through the page: a popover takes a focus on the page
  // for one that left it and closes (the squash of the fixups under load, before a later return of the focus)
  expect(await leaves()).toEqual(["kept"])
}

/**
 * How the focus left Stop Waiting: "kept" where the control it went to got it while Stop Waiting was still in the
 * document, so the focus never was on the page; "page" where it came only after Stop Waiting had gone
 */
async function focusAsStopWaitingLeaves(page: Page): Promise<() => Promise<string[]>> {
  await page.evaluate(() => {
    const record: string[] = []
    ;(window as unknown as { focusAsStopWaitingLeaves: string[] }).focusAsStopWaitingLeaves = record
    document.addEventListener("focusin", event => {
      if (event.target instanceof Element && !event.target.matches('[data-testid="stop-waiting"]')) {
        record.push(document.querySelector('[data-testid="stop-waiting"]') === null ? "page" : "kept")
      }
    })
  })
  return () => page.evaluate(() => (window as unknown as { focusAsStopWaitingLeaves: string[] }).focusAsStopWaitingLeaves)
}

test.describe("A wait for the lock beside the button that started it", () => {
  test(
    "should say whom a drop waits for on the line of its buttons, Drop under the pointer, and stop it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      const { form, drop } = await openDropWhileHeld(page, repo)
      await expectWaitBesideItsButton(page, form, drop, "Canceled dropping 1 commit before it started")
      await verifyCommitCount(findBranchRow(page, ISSUE), 3)
      await repo.holdRepositoryLock(false)
    },
  )

  // a busy button keeps the focus (`focusableWhenDisabled`), so it has no `disabled`: `Button` gives it no hover and no
  // press shift by `aria-disabled`, and no dimming while it is busy, its spinner says it
  test("should change nothing of a busy button under the pointer, neither by its hover nor by a press", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const { form, drop } = await openDropWhileHeld(page, repo)
    await drop.click()
    await expect(form.getByTestId("lock-wait-notice")).toContainText(WAITING)
    await expect(drop).toHaveAttribute("aria-busy", "true")
    await expect(drop).toHaveAttribute("aria-disabled", "true")
    // the final look of each state at once: a transition would show the look before it for a while
    await page.addStyleTag({ content: "*, *::before, *::after { transition: none !important; }" })
    const look = () =>
      drop.evaluate(button => {
        const style = getComputedStyle(button)
        return { background: style.backgroundColor, color: style.color, translate: style.translate, opacity: style.opacity }
      })
    await page.mouse.move(0, 0)
    const rest = await look()
    expect(rest.opacity).toBe("1")
    await drop.hover()
    expect(await look()).toEqual(rest)
    await page.mouse.down()
    expect(await look()).toEqual(rest)
    await page.mouse.up()
    await runAndWaitForAnswer(page, "execute", () => form.getByRole("button", { name: "Stop Waiting" }).click())
    await expect(page.getByText("Canceled dropping 1 commit before it started")).toBeVisible()
    await repo.holdRepositoryLock(false)
  })

  test(
    "should say whom a move to a branch waits for below its input, Move under the pointer, and stop it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      const { form } = await openMoveToDocsWhileHeld(page, repo)
      await expectWaitBesideItsButton(page, form, form.getByRole("button", { name: /^(Loading )?Move$/ }), "Canceled moving 1 commit to docs before it started", {
        grows: true,
      })
      await verifyCommitCount(findBranchRow(page, "docs"), 1)
      await repo.holdRepositoryLock(false)
    },
  )

  test(
    "should take the keys to Stop Waiting as a move pressed by Enter waits for the lock, and give them back to its input as Space stops it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      const { input, form } = await openMoveToDocsWhileHeld(page, repo)
      const cancels = await recordCommand(page, "cancel_operation")
      await page.keyboard.press("Enter")

      // the screen reader is told whom for, and the keys are on Stop Waiting: no Tab to reach it
      const notice = form.getByTestId("lock-wait-notice")
      await expect(notice).toContainText(WAITING)
      await expect(form.getByTestId("lock-wait-status")).toHaveText(WAITING_FOR)
      await expect(notice.getByRole("button", { name: "Stop Waiting" })).toBeFocused()
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Space"))
      await expect(page.getByText("Canceled moving 1 commit to docs before it started")).toBeVisible()
      expect(cancels).toHaveLength(1)
      // the focus is back where the press left it: Enter moves, once the holder is done
      await expect(input).toBeFocused()
      await verifyCommitCount(findBranchRow(page, "docs"), 1)
      await repo.holdRepositoryLock(false)
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await verifyCommitCount(findBranchRow(page, "docs"), 2)
    },
  )

  test(
    "should say whom a squash of fixups waits for on the line of its buttons, Squash under the pointer, and stop it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("fixups")
      await syncAndWaitForBranches()
      await repo.holdRepositoryLock(true)
      const auth = findBranchRow(page, "auth")
      await auth.getByRole("button", { name: "1 fixup" }).click()
      const popover = page.getByTestId("squash-fixups-popover")
      await expectWaitBesideItsButton(page, popover, popover.getByTestId("squash-fixups-confirm"), "Canceled squashing the fixups before it started")
      await verifyCommitCount(auth, 3)
      await repo.holdRepositoryLock(false)
    },
  )

  // a dialog: whom it waits for comes over the lines of its footer, which keep their room, and the spinner takes the
  // place of the icon of the button, so the footer keeps its height and the button its box (it moved 9 px down and
  // widened 14 px to the left)
  test(
    "should say whom a squash waits for in the footer of its dialog, Squash under the pointer, and stop it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      await repo.holdRepositoryLock(true)
      const dialog = await openSquashOfTwo(page)
      await expectWaitBesideItsButton(page, dialog, dialog.getByRole("button", { name: /^(Loading )?Squash/ }), "Canceled squashing 2 commits before it started")
      await verifyCommitCount(findBranchRow(page, ISSUE), 3)
      await repo.holdRepositoryLock(false)
    },
  )

  test(
    "should say whom a reword waits for in the footer of the preview, Save under the pointer, and stop it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setup(page, setupRepo, syncAndWaitForBranches)
      await repo.holdRepositoryLock(true)
      const editor = await editCommitMessage(page, getBranchCommitRow(page, ISSUE, "Add parser"))
      await editor.press("End")
      await page.keyboard.type(" again")
      const preview = page.getByTestId("commit-preview")
      await expectWaitBesideItsButton(page, preview, preview.getByRole("button", { name: /^(Loading )?Save/ }), "Canceled editing the commit message before it started")
      await expect(getBranchCommitRow(page, ISSUE, "Add parser")).toBeVisible()
      await repo.holdRepositoryLock(false)
    },
  )
})
