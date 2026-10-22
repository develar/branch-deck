// @spec spec/branch/parked-branches.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton } from "../helpers/branch-helpers"
import { dropAt, dropOn, startDrag } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

// the `branch_filter` template: the stack core → api → web with cli forked off core, and seven branches on their own;
// neither Patronus nor GitHub, so every branch the remote lacks is the user's move, its push
const BRANCH_COUNT = 11

function parkedCard(page: Page): Locator {
  return page.getByTestId("parked-branches-card")
}

/** The bar of the targets of a drag, over the bottom of the window while a drag goes on */
function dragBar(page: Page): Locator {
  return page.getByTestId("drag-target-bar")
}

/** The header of the Parked card, its toggle */
function parkedHeader(page: Page): Locator {
  return parkedCard(page)
    .getByRole("button", { name: /Parked/ })
    .first()
}

function toast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]").filter({ hasText: title })
}

/** The branch rows of the Parked card, in its order */
async function parkedRows(page: Page): Promise<string[]> {
  return parkedCard(page)
    .getByTestId("branch-row")
    .evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

/** Parks `name` with `note` through the form of Park Branch…, opened by `open`; resolves once the sync showed it */
async function parkWithForm(page: Page, open: () => Promise<void>, note: string): Promise<void> {
  await open()
  await page.getByRole("menuitem", { name: "Park Branch…" }).click()
  const input = page.getByTestId("inline-park-input").getByRole("textbox")
  await expect(input).toBeFocused()
  if (note !== "") {
    await page.keyboard.type(note)
  }
  await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
}

test.describe("Parked branches", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("branch_filter")
    await syncAndWaitForBranches()
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
  })

  test("should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo", async ({ page }) => {
    const yourMove = page.getByRole("group", { name: "Branch state" }).locator("[data-chip=yourMove]")
    const moves = page.getByTestId("landing-indicator").getByTestId("landing-moves")
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT}`)
    const movesBefore = Number(await moves.textContent())
    await expect(parkedCard(page)).toHaveCount(0)

    await parkWithForm(page, () => findBranchRow(page, "docs").click({ button: "right" }), "waits for the fix of IJPL-1")

    // the row moves into the Parked card, says why, and is no move of the user
    expect(await parkedRows(page)).toEqual(["docs"])
    await expect(parkedCard(page)).toContainText("1 branch")
    const row = findBranchRow(page, "docs")
    await expect(row.getByTestId("parked-status")).toHaveText("Parked")
    await expect(row.getByTestId("branch-summary")).toHaveText("waits for the fix of IJPL-1")
    await expect(row.getByTestId("lane-node")).toHaveAttribute("data-kind", "parked")
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT - 1}`)
    await expect(moves).toHaveText(String(movesBefore - 1))

    // no push of it: the menu says why
    await row.click({ button: "right" })
    const push = page.getByRole("menuitem", { name: /^Push/ }).first()
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(page.getByRole("tooltip")).toHaveText("Parked: Unpark it to push")
    await page.keyboard.press("Escape")

    // F2 steps through the moves and never stops at it
    await page.mouse.click(5, 300)
    const focused: string[] = []
    for (let i = 0; i < BRANCH_COUNT - 1; i++) {
      await page.keyboard.press("F2")
      await expect(page.locator("tr[data-testid=branch-row]:focus")).toHaveCount(1)
      focused.push((await page.locator("tr[data-testid=branch-row]:focus").getAttribute("data-branch-name")) ?? "")
    }
    expect(new Set(focused).size).toBe(BRANCH_COUNT - 1)
    expect(focused).not.toContain("docs")

    // Undo of the toast unparks it
    await runAndWaitForSync(page, () => toast(page, "Parked docs").getByRole("button", { name: "Undo" }).click())
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT}`)
    await expect(findBranchRow(page, "docs").getByTestId("lane-node")).not.toHaveAttribute("data-kind", "parked")
  })

  test("should park a stack root with the branches built on it, edit its note and unpark it from the menus", async ({ page }) => {
    await parkWithForm(page, () => getMoreButton(findBranchRow(page, "core")).click(), "")

    // the branches built on core carry its commits: they are parked with it, as a stack of their own
    expect((await parkedRows(page)).toSorted()).toEqual(["api", "cli", "core", "web"])
    await expect(parkedCard(page)).toContainText("4 branches")
    await expect(findBranchRow(page, "core").getByTestId("parked-status")).toHaveText("Parked")
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveText("Parked with core")
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)

    // a branch parked with its parent unparks the parent
    await findBranchRow(page, "web").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: /^Unpark core/ })).toBeVisible()
    await page.keyboard.press("Escape")

    // the note of the parked root
    await findBranchRow(page, "core").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Edit Park Note…" }).click()
    const input = page.getByTestId("inline-park-input").getByRole("textbox")
    await expect(input).toBeFocused()
    await page.keyboard.type("later")
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await expect(findBranchRow(page, "core").getByTestId("branch-summary")).toHaveText("later")
    // a branch parked with it keeps its own summary; the hover of its status tells the note
    await expect(findBranchRow(page, "api").getByTestId("parked-status")).toHaveText("Parked with core")
    await findBranchRow(page, "api").getByTestId("parked-status").hover()
    await expect(page.getByRole("tooltip")).toHaveText("Parked with core, kept local: later")

    // Unpark Branch puts the stack back into the table
    await findBranchRow(page, "core").click({ button: "right" })
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: /^Unpark Branch/ }).click())
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(toast(page, "Unparked core")).toBeVisible()
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveCount(0)
  })

  test("should collapse the Parked card and keep its rows out of the table", async ({ page }) => {
    await parkWithForm(page, () => findBranchRow(page, "lint").click({ button: "right" }), "")
    await expect(findBranchRow(page, "lint")).toBeVisible()
    await parkedCard(page)
      .getByRole("button", { name: /Parked/ })
      .first()
      .click()
    await expect(parkedCard(page)).toHaveAttribute("data-state", "closed")
    await expect(findBranchRow(page, "lint")).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT - 1)
  })

  test("should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch", async ({
    page,
  }) => {
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(dragBar(page)).toHaveCount(0)
    await startDrag(page, findBranchRow(page, "docs"))
    const parkZone = dragBar(page).getByTestId("park-drop-zone")
    await expect(parkZone).toHaveAttribute("data-drop-target", "ready")
    await expect(parkZone).toHaveText("Park docs")
    // a branch that lands is no unpark
    await expect(dragBar(page).getByTestId("new-branch-drop-zone")).toHaveText("Move to New Branch")
    await expect(dragBar(page).getByTestId("unpark-drop-zone")).toHaveCount(0)
    await runAndWaitForSync(page, () => dropAt(page, parkZone))

    expect(await parkedRows(page)).toEqual(["docs"])
    await expect(findBranchRow(page, "docs").getByTestId("parked-status")).toHaveText("Parked")
    await expect(toast(page, "Parked docs")).toBeVisible()
    await expect(dragBar(page)).toHaveCount(0)

    // out of the Parked card: no park zone for it, the unpark zone instead
    await startDrag(page, findBranchRow(page, "docs"))
    const unparkZone = dragBar(page).getByTestId("unpark-drop-zone")
    await expect(unparkZone).toHaveAttribute("data-drop-target", "ready")
    await expect(unparkZone).toHaveText("Unpark docs")
    await expect(dragBar(page).getByTestId("park-drop-zone")).toHaveCount(0)
    await runAndWaitForSync(page, () => dropAt(page, unparkZone))
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(findBranchRow(page, "docs").getByTestId("parked-status")).toHaveCount(0)

    // Undo of the unpark parks it again
    await runAndWaitForSync(page, () => toast(page, "Unparked docs").getByRole("button", { name: "Undo" }).click())
    expect(await parkedRows(page)).toEqual(["docs"])

    // a row of the Parked card drags its commits onto a branch of the table as any row
    await startDrag(page, findBranchRow(page, "docs"))
    await runAndWaitForSync(page, () => dropOn(page, findBranchRow(page, "lint")))
    await expect(toast(page, "Moved 1 commit to lint")).toBeVisible()
    await expect(parkedCard(page)).toHaveCount(0)
  })

  test("should park the selected branches dropped on a row or the header of the Parked card at once with one Undo, and unpark a stack by its root, never by a branch parked with it", async ({
    page,
  }) => {
    await parkWithForm(page, () => findBranchRow(page, "lint").click({ button: "right" }), "")
    expect(await parkedRows(page)).toEqual(["lint"])

    // two branches dropped on a row of the card: the card takes them, not the row; one park, one toast, one Undo
    await findBranchRow(page, "core").click({ modifiers: ["ControlOrMeta"] })
    await findBranchRow(page, "docs").click({ modifiers: ["ControlOrMeta"] })
    await startDrag(page, findBranchRow(page, "docs"))
    await expect(parkedCard(page)).toHaveAttribute("data-drop-target", "ready")
    await expect(findBranchRow(page, "lint")).not.toHaveAttribute("data-drop-target")
    await expect(findBranchRow(page, "core")).toHaveAttribute("data-dragging", "true")
    await runAndWaitForSync(page, () => dropOn(page, findBranchRow(page, "lint")))
    expect((await parkedRows(page)).toSorted()).toEqual(["api", "cli", "core", "docs", "lint", "web"])
    const parkedBoth = toast(page, "Parked ").filter({ hasText: "core" }).filter({ hasText: "docs" })
    await runAndWaitForSync(page, () => parkedBoth.getByRole("button", { name: "Undo" }).click())
    expect(await parkedRows(page)).toEqual(["lint"])

    // a plain click selects no branch: the drag carries core alone, onto the header of the card
    await findBranchRow(page, "core").click()
    await startDrag(page, findBranchRow(page, "core"))
    await runAndWaitForSync(page, () => dropOn(page, parkedHeader(page)))
    expect((await parkedRows(page)).toSorted()).toEqual(["api", "cli", "core", "lint", "web"])

    // web carries the commits of core: alone it is refused, and says why
    await startDrag(page, findBranchRow(page, "web"))
    const unparkZone = dragBar(page).getByTestId("unpark-drop-zone")
    await expect(unparkZone).toHaveAttribute("data-drop-target", "refused")
    await expect(unparkZone).toHaveText("Cannot unpark web: It carries the commits of parked core, drag core here")
    await dropAt(page, unparkZone)
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveText("Parked with core")

    // the root unparks its stack
    await startDrag(page, findBranchRow(page, "core"))
    await expect(unparkZone).toHaveAttribute("data-drop-target", "ready")
    await runAndWaitForSync(page, () => dropAt(page, unparkZone))
    expect(await parkedRows(page)).toEqual(["lint"])
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveCount(0)
    await expect(toast(page, "Unparked core")).toBeVisible()
  })

  test("should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes", async ({
    page,
  }) => {
    await parkWithForm(page, () => findBranchRow(page, "lint").click({ button: "right" }), "")
    await parkedHeader(page).click()
    await expect(parkedCard(page)).toHaveAttribute("data-state", "closed")

    // an unselected row goes alone: docs stays selected
    await findBranchRow(page, "docs").click({ modifiers: ["ControlOrMeta"] })
    await startDrag(page, findBranchRow(page, "cli"))
    await expect(parkedCard(page)).toHaveAttribute("data-drop-target", "ready")
    await runAndWaitForSync(page, () => dropOn(page, parkedHeader(page)))
    await expect(toast(page, "Parked cli")).toBeVisible()
    await expect(parkedCard(page)).toContainText("2 branches")
    await expect(findBranchRow(page, "docs")).toHaveAttribute("data-selected", "true")

    // Park of the selection bar
    const actions = page.getByTestId("branch-selection-actions")
    await expect(actions).toContainText("1 branch selected")
    await expect(actions.getByRole("button", { name: "Unpark" })).toHaveCount(0)
    await runAndWaitForSync(page, () => actions.getByRole("button", { name: "Park", exact: true }).click())
    await expect(toast(page, "Parked docs")).toBeVisible()
    await expect(actions).toHaveCount(0)

    // a selection of a parked and a landing branch: Park takes core, Unpark takes docs
    await parkedHeader(page).click()
    await findBranchRow(page, "docs").click({ modifiers: ["ControlOrMeta"] })
    await findBranchRow(page, "core").click({ modifiers: ["ControlOrMeta"] })
    await expect(actions).toContainText("2 branches selected")
    await runAndWaitForSync(page, () => actions.getByRole("button", { name: "Park", exact: true }).click())
    await expect(toast(page, "Parked core")).toBeVisible()
    await findBranchRow(page, "docs").click({ modifiers: ["ControlOrMeta"] })
    await runAndWaitForSync(page, () => actions.getByRole("button", { name: "Unpark" }).click())
    await expect(toast(page, "Unparked docs")).toBeVisible()
    await expect(findBranchRow(page, "docs").getByTestId("parked-status")).toHaveCount(0)
    await expect(findBranchRow(page, "core").getByTestId("parked-status")).toHaveText("Parked")
  })

  test("should keep the rows in place when a drag starts, show the drag bar in view at the bottom of a long list, and take no drop on the bar from a drag that started under it", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1100, height: 420 })
    const rows = page.getByTestId("branch-row")
    const last = rows.last()
    await last.scrollIntoViewIfNeeded()
    const lastName = (await last.getAttribute("data-branch-name"))!
    const before = (await last.boundingBox())!

    // nothing shows in the flow: the row stays under the pointer, the bar is over the bottom of the window
    await startDrag(page, last)
    const parkZone = dragBar(page).getByTestId("park-drop-zone")
    await expect(parkZone).toBeInViewport()
    await expect(dragBar(page).getByTestId("new-branch-drop-zone")).toBeInViewport()
    expect((await last.boundingBox())!.y).toBeCloseTo(before.y, 0)
    const bar = (await dragBar(page).boundingBox())!
    await runAndWaitForSync(page, () => dropAt(page, parkZone))
    await expect(toast(page, `Parked ${lastName}`)).toBeVisible()
    await runAndWaitForSync(page, () => toast(page, `Parked ${lastName}`).getByRole("button", { name: "Undo" }).click())
    await expect(parkedCard(page)).toHaveCount(0)

    // a row where the bar shows: the window ends so that its middle is the middle of the bar
    await page.getByTestId("branches-view").evaluate(main => main.scrollTo(0, 0))
    const row = rows.nth(2)
    const rowName = (await row.getAttribute("data-branch-name"))!
    const rowBox = (await row.boundingBox())!
    const barBottomGap = 420 - (bar.y + bar.height)
    await page.setViewportSize({ width: 1100, height: Math.round(rowBox.y + rowBox.height / 2 + bar.height / 2 + barBottomGap) })
    await expect(row).toBeInViewport()
    // pressed where the middle of the bar will be
    const pressAt = { x: bar.x + bar.width / 2, y: rowBox.y + rowBox.height / 2 }
    const startDragUnderBar = async () => {
      await page.mouse.move(pressAt.x, pressAt.y)
      await page.mouse.down()
      await page.mouse.move(pressAt.x + 10, pressAt.y + 4, { steps: 4 })
    }
    await startDragUnderBar()
    await expect(dragBar(page)).toHaveAttribute("data-armed", "false")
    // released at once: the bar takes nothing, the drag ends
    await page.mouse.up()
    await expect(dragBar(page)).toHaveCount(0)
    await expect(findBranchRow(page, rowName).getByTestId("parked-status")).toHaveCount(0)
    await expect(parkedCard(page)).toHaveCount(0)

    // once the pointer left the bar, it takes the drop
    await startDragUnderBar()
    await expect(dragBar(page)).toHaveAttribute("data-armed", "false")
    // onto the first row, in the window, not the title bar above it
    const first = (await rows.first().boundingBox())!
    await page.mouse.move(pressAt.x, first.y + first.height / 2, { steps: 4 })
    await expect(dragBar(page)).toHaveAttribute("data-armed", "true")
    await runAndWaitForSync(page, () => dropAt(page, dragBar(page).getByTestId("park-drop-zone")))
    await expect(toast(page, `Parked ${rowName}`)).toBeVisible()
  })
})
