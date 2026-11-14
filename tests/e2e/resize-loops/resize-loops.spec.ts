// @spec openspec/specs/repository/title-bar/spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { expectOverviewLaidOut } from "../helpers/diff-helpers"
import { MARKS_SUBJECT, openMarksReview } from "../helpers/marks-helpers"
import { turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { availableReviewRequests, openRequestReview, reviewRequest } from "../helpers/review-request-helpers"
import { openSubmenu } from "../helpers/selection-helpers"
import { checkForUpdatesFromMenu, TITLE_BARS, toastWith, useTitleBar } from "../helpers/title-bar-helpers"
import { closeToast } from "../helpers/toast-helpers"
import { expectClosed } from "../helpers/wait-helpers"
import { resizeObservers } from "../setup/resize-observers"

/*
 * The ResizeObserver loops of the views that made them in WebKit under load (docs/contributing/frontend.md, "A flushSync
 * in the delivery of a ResizeObserver"), each attributed (tests/e2e/setup/resize-observers.ts): a loop of an observer of
 * the app (`observeSize`) fails the test, as it fails any, which the page logs as an error; one of a library's observer
 * (Base UI, the panels) is a warning of its report, with its elements, which no other test names
 */

/** The loops of `page` from now on, by owner, as the attribution tells them */
function recordLoops(page: Page): { app: string[]; library: string[] }[] {
  const loops: { app: string[]; library: string[] }[] = []
  page.on("console", message => {
    if (message.type() === "info" && message.text().startsWith("[e2e-resize-loop] ")) {
      loops.push(JSON.parse(message.text().slice("[e2e-resize-loop] ".length)) as { app: string[]; library: string[] })
    }
  })
  return loops
}

/** No loop of the app; the loops of the libraries are warnings of the report */
function expectNoLoopOfTheApp(loops: readonly { app: string[]; library: string[] }[]): void {
  for (const loop of loops.filter(it => it.app.length === 0)) {
    test.info().annotations.push({ type: "warning", description: `a ResizeObserver loop of a library: ${loop.library.join("; ") || "no element left pending"}` })
  }
  expect(loops.flatMap(loop => loop.app)).toEqual([])
}

test.describe("ResizeObserver loops", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(resizeObservers, { attribute: true })
  })

  // the check itself: an observer of the app (marked as `observeSize` marks it) that resizes the element it watches from
  // outside its subtree in its callback makes a loop the attribution names as the app's; an unmarked one, a library's.
  // Chromium alone: WebKit reports no loop of such a page, though it does of the views of the app under load
  test("should attribute a loop to the observer of the app or of a library whose element it left pending", async ({ page }) => {
    const loops = recordLoops(page)
    await page.setContent("<div id=outer style='width:100px'><div id=app></div></div><div id=other style='width:100px'><div id=library></div></div>")
    // content set in place runs no init script: the wrapper goes in by hand
    await page.evaluate(resizeObservers, { attribute: true })
    await page.evaluate(() => {
      const loop = (id: string, owner: string, marked: boolean) => {
        const inner = document.getElementById(id)!
        const outer = inner.parentElement!
        const observer = new ResizeObserver(() => {
          outer.style.width = `${outer.offsetWidth + 1}px`
        })
        if (marked) {
          Reflect.set(observer, Symbol.for("branch-deck.observeSize"), true)
        }
        inner.textContent = owner
        observer.observe(inner)
      }
      loop("app", "app", true)
      loop("library", "library", false)
    })
    await expect.poll(() => loops.length).toBeGreaterThan(0)
    // each element by its text: the one the app's observer watches, the one the library's does
    expect(loops.some(loop => loop.app.some(element => element.includes('"app"')))).toBe(true)
    expect(loops.some(loop => loop.library.some(element => element.includes('"library"')))).toBe(true)
    expect(loops.flatMap(loop => loop.app).some(element => element.includes('"library"'))).toBe(false)
  })

  test("should make no loop of the app in the review of a pull request as its overview, its commits and their cards come and go @webkit", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const loops = recordLoops(page)
    const repo = await setupRepo("review_tour", { reviewRequests: availableReviewRequests([reviewRequest("layout", 201, 1, { title: "Keep the stored layout" })]) })
    turnOnReviewPlan(repo)
    await syncAndWaitForBranches()
    const { window, review } = await openRequestReview(page, 201)
    await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
    await window.keyboard.press("d")
    await expect(review.getByTestId("commit-review")).toBeVisible()
    await window.keyboard.press("Escape")
    const card = window.getByTestId("commit-card")
    const subject = review.getByTestId("review-commits").getByTestId("review-commit").first().getByTestId("review-commit-subject")
    for (let round = 0; round < 3; round++) {
      await window.mouse.move(0, 0)
      await subject.hover()
      await expect(card).toBeVisible()
      await window.mouse.move(0, 0)
      await expectClosed(card)
    }
    expectNoLoopOfTheApp(loops)
  })

  test("should make no loop of the app as the menu of a row and its submenu open and close @webkit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const loops = recordLoops(page)
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    for (let round = 0; round < 3; round++) {
      await findBranchRow(page, "air").click({ button: "right" })
      await openSubmenu(page, "Whole Stack")
      await page.keyboard.press("Escape")
      await page.keyboard.press("Escape")
      await expectClosed(page.getByRole("menu"))
    }
    expectNoLoopOfTheApp(loops)
  })

  // the patch of @base-ui/react (patches/@base-ui__react@1.9.0.patch): Toast.Content sets the height of its toast in the
  // next frame, never in the delivery of its observers, where its flushSync rendered every update pending then (a loop).
  // The height is set by measuring the toast with its height auto and back: a change of the style of the toast in the
  // callback of the observer of its content is the height set in the delivery. A port of the patch that loses that hunk
  // fails here at every toast, not by the chance of a loop
  test("should set the height of a toast after the delivery of the observer of its content, never in it @webkit", async ({ page, setupRepo }) => {
    await page.addInitScript(() => {
      const probe = { deliveries: 0, styled: [] as string[] }
      ;(window as unknown as { toastProbe: typeof probe }).toastProbe = probe
      const styles = new MutationObserver(() => {})
      styles.observe(document, { subtree: true, attributes: true, attributeFilter: ["style"] })
      const Original = globalThis.ResizeObserver
      globalThis.ResizeObserver = class extends Original {
        constructor(callback: ResizeObserverCallback) {
          super((entries, observer) => {
            styles.takeRecords()
            callback(entries, observer)
            const ofToasts = styles.takeRecords().filter(record => (record.target as Element).getAttribute("data-slot") === "toast")
            if (entries.some(entry => entry.target.getAttribute("data-slot") === "toast-content")) {
              probe.deliveries++
              probe.styled.push(...ofToasts.map(record => (record.target as Element).getAttribute("style") ?? ""))
            }
          })
        }
      }
    })
    await useTitleBar(page, { ...TITLE_BARS.gnome, checkForUpdates: true })
    await setupRepo("behind_upstream")
    await checkForUpdatesFromMenu(page)
    await expect(toastWith(page, "BranchDeck is up to date")).toBeVisible()
    const probe = () => page.evaluate(() => (window as unknown as { toastProbe: { deliveries: number; styled: string[] } }).toastProbe)
    // the observer of the content reported the toast, and set nothing of it as it did
    await expect.poll(async () => (await probe()).deliveries).toBeGreaterThan(0)
    expect((await probe()).styled).toEqual([])
  })

  test("should make no loop of the app as the toasts of two checks for updates stack and close @webkit", async ({ page, setupRepo }) => {
    const loops = recordLoops(page)
    await useTitleBar(page, { ...TITLE_BARS.gnome, checkForUpdates: true })
    const repo = await setupRepo("behind_upstream")
    await checkForUpdatesFromMenu(page)
    const upToDate = toastWith(page, "BranchDeck is up to date")
    await expect(upToDate).toBeVisible()
    await repo.setAvailableUpdate("1.2.3")
    await checkForUpdatesFromMenu(page)
    const available = toastWith(page, "Update available")
    await expect(available).toBeVisible()
    await closeToast(available)
    await closeToast(upToDate)
    expectNoLoopOfTheApp(loops)
  })
})

/**
 * The frames of a page as painted, in `window.paintedOverlaps`: each frame an `IntersectionObserver` takes the boxes of
 * the overview of a review and of its first file anew, which it reads after the observers of that rendering update and
 * before its paint (a box read in a frame callback would be the layout of a task that ran since the last paint, which
 * that update may correct); the frames with both, and those where the overview lies on the file
 */
function samplePaintedOverlaps(): void {
  const samples = { frames: 0, overlaps: [] as string[] }
  ;(window as unknown as { paintedOverlaps: typeof samples }).paintedOverlaps = samples
  const ofTestId = (entries: readonly IntersectionObserverEntry[], id: string) => entries.find(entry => entry.target.getAttribute("data-testid") === id)?.boundingClientRect
  const take = (entries: readonly IntersectionObserverEntry[]) => {
    const frames = new Map<number, IntersectionObserverEntry[]>()
    for (const entry of entries) {
      frames.set(entry.time, [...(frames.get(entry.time) ?? []), entry])
    }
    for (const frame of frames.values()) {
      const overview = ofTestId(frame, "diff-view-prologue")
      const file = ofTestId(frame, "diff-view-file")
      if (overview !== undefined && file !== undefined && overview.height > 0 && file.height > 0) {
        samples.frames++
        if (overview.bottom > file.top + 1) {
          samples.overlaps.push(`the overview ends at ${Math.round(overview.bottom)}, the file starts at ${Math.round(file.top)}`)
        }
      }
    }
  }
  const boxes = new IntersectionObserver(take)
  const frame = () => {
    take(boxes.takeRecords())
    boxes.disconnect()
    // the overview of the view that shows one, and the first file of that view: a view kept hidden has boxes of 0
    const overview = [...document.querySelectorAll("[data-testid=diff-view-prologue]")].find(element => element.getClientRects().length > 0)
    const file = overview?.closest("[data-testid=diff-view]")?.querySelector("[data-testid=diff-view-file]")
    if (overview !== undefined && file !== null && file !== undefined) {
      boxes.observe(overview)
      boxes.observe(file)
    }
    requestAnimationFrame(frame)
  }
  requestAnimationFrame(frame)
}

test.describe("The frames of a review as it opens", () => {
  // the overview of a review (the heading of its commit) comes above its files by a read of its own and grows its block
  // after they show; the observer of the block reports it in the frame it grows in, however React rendered it (a task
  // before the end of the delivery, a commit in it), so no frame paints the overview over the first file
  // (src/lib/observe-size.ts). Chromium alone: a CPU four times as slow, as a loaded machine's, by its DevTools protocol
  test("should paint no frame of a review with its overview over its first file as it opens, on a slow CPU", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.addInitScript(samplePaintedOverlaps)
    await (await page.context().newCDPSession(page)).send("Emulation.setCPUThrottlingRate", { rate: 4 })
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    const { code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    const painted = () => page.evaluate(() => (window as unknown as { paintedOverlaps: { frames: number; overlaps: string[] } }).paintedOverlaps)
    const overlaps: string[] = []
    let frames = 0
    for (let opening = 0; opening < 4; opening++) {
      if (opening > 0) {
        await page.reload()
        await expectOverviewLaidOut(code)
      }
      // the frames that settle it
      await expect.poll(async () => (await painted()).frames).toBeGreaterThan(5)
      const samples = await painted()
      frames += samples.frames
      overlaps.push(...samples.overlaps.map(overlap => `opening ${opening + 1}: ${overlap}`))
    }
    expect(frames).toBeGreaterThan(0)
    expect(overlaps).toEqual([])
  })
})
