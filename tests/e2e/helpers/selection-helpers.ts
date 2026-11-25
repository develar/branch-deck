import type { Page, Locator } from "@playwright/test"
import { expect, hoverSubmenuTrigger } from "../fixtures/base-test"
import { animationsEnded, expectClosed } from "./wait-helpers"

/**
 * Select a single commit
 */
export async function selectCommit(commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click()

  // the selection is there once the row says it: what follows waits for the element it acts on
  await expect(commitLocator).toHaveAttribute("data-selected", "true")
}

/**
 * Multi-select a commit (adds to existing selection)
 */
export async function multiSelectCommit(commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click({ modifiers: ["ControlOrMeta"] })

  await expect(commitLocator).toHaveAttribute("data-selected", "true")
}

/**
 * Shift-click a commit to select the range from the last clicked row to it
 */
export async function shiftSelectCommit(commitLocator: Locator): Promise<void> {
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click({ modifiers: ["Shift"] })
  await expect(commitLocator).toHaveAttribute("data-selected", "true")
}

/**
 * Select all rows of a selectable commit table with Cmd/Ctrl+A
 */
export async function selectAllCommits(page: Page, table: Locator): Promise<void> {
  await expect(table).toHaveAttribute("tabindex", "0")
  await table.focus()
  await page.keyboard.press("ControlOrMeta+a")
}

/**
 * Open context menu on an element
 */
export async function openContextMenu(page: Page, element: Locator): Promise<void> {
  await element.click({ button: "right" })
}

/**
 * Click a context menu item by name
 */
export async function clickContextMenuItem(page: Page, itemName: string, options: { readonly exact?: boolean } = {}): Promise<void> {
  const menuItem = page.getByRole("menuitem", { name: itemName, exact: options.exact === true })
  await menuItem.waitFor({ state: "visible" })
  await menuItem.click()
}

/**
 * Open the submenu `name` of the open menu ("Patronus", "Copy" of the branch menu) as the user does, by hovering its
 * item; resolves once it is open. The one way a test opens a submenu: `hover()` of its item is refused
 * (`refuseSubmenuHovers` of fixtures/base-test.ts). An open menu doesn't move (`frozenMenuGroups`), but a scroll of it
 * moves the item from under the pointer before the delay ran out, and nothing opens: the error says so
 */
export async function openSubmenu(page: Page, name: string): Promise<void> {
  const trigger = page.getByRole("menuitem", { name, exact: true })
  // a menu zooms in as it opens: the item would move away under a pointer put on it meanwhile, its leave canceling the
  // open; the hover that follows into the submenu waits for it likewise. Only the open menus: the exit of a closing one
  // ends only as the window paints, which a loaded machine does not for seconds
  const open = page.getByRole("menu").and(page.locator(":not([data-closed])"))
  await expect(trigger).toBeVisible()
  await animationsEnded(open)
  await hoverSubmenuTrigger(trigger)
  try {
    await expect(trigger).toHaveAttribute("aria-expanded", "true")
  } catch (error) {
    if (error instanceof Error && !(await trigger.evaluate(element => element.matches(":hover")))) {
      error.message = `${error.message}\n\nThe menu moved "${name}" from under the pointer before its submenu opened (a scroll of the menu). An open menu keeps its items in their places: an item a fact brings shows only at the next open (waitForStepFacts of tests/e2e/helpers/wait-helpers.ts before the menu)`
    }
    throw error
  }
  await animationsEnded(open)
}

/** Closes the open menu with its open submenu by Escape, as the user does: each Escape closes one level */
export async function closeMenus(page: Page): Promise<void> {
  await page.keyboard.press("Escape")
  await page.keyboard.press("Escape")
  await expectClosed(page.getByRole("menu"))
}

/**
 * Wait for and click the "Move to…" button of the selection
 */
export async function clickGroupIntoBranchButton(page: Page): Promise<void> {
  // the button is on the lead row of the selection (the row clicked last), wherever the list is
  const createBranchButton = page.getByTestId("selection-actions").getByRole("button", { name: "Move to…" })
  await expect(createBranchButton).toBeVisible()
  await createBranchButton.click()
}

/**
 * Check if an element has the selected attribute
 */
export async function isSelected(element: Locator): Promise<boolean> {
  const selected = await element.getAttribute("data-selected")
  return selected === "true"
}

/**
 * Get all selected commits
 */
export function getSelectedCommits(page: Page): Locator {
  return page.locator('[data-row-id][data-selected="true"]')
}

/** The `drop` and `dragend` events the page got since its first count (a capture listener the first call installs) */
async function dragEnds(page: Page): Promise<{ drops: number; ends: number }> {
  return page.evaluate(() => {
    const target = window as unknown as { __e2eDragEnds?: { drops: number; ends: number } }
    if (target.__e2eDragEnds === undefined) {
      const counts = { drops: 0, ends: 0 }
      target.__e2eDragEnds = counts
      document.addEventListener("drop", () => counts.drops++, { capture: true })
      document.addEventListener("dragend", () => counts.ends++, { capture: true })
    }
    return { ...target.__e2eDragEnds }
  })
}

/**
 * The gestures of a drop at most: under a load of 200 the gesture again lost its drop too in 1 of 20 runs (none in 40
 * with a third)
 */
const DROP_ATTEMPTS = 3

/** The row of the drag `startDrag` started last, which `dropOn` drags again when its drop never reached the page */
let draggedRow: Locator | null = null

/** Starts a native drag of a commit or branch row and keeps the button pressed; finish it with `dropOn` */
export async function startDrag(page: Page, row: Locator): Promise<void> {
  draggedRow = row
  await dragEnds(page)
  await row.scrollIntoViewIfNeeded()
  await row.hover()
  await page.mouse.down()
  const box = (await row.boundingBox())!
  // the drag starts on the first move
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2 + 10, { steps: 4 })
}

/**
 * Drops the drag of `startDrag` on `target`, and drags onto it once more when the drop never reached the page
 * (docs/contributing/e2e-tests.md, "A drop that never reached the page")
 */
export async function dropOn(page: Page, target: Locator): Promise<void> {
  for (let attempt = 1; ; attempt++) {
    const before = await dragEnds(page)
    await target.hover()
    const took = (await target.getAttribute("data-drop-target")) === "over"
    // the row the target is in, read while it shows: the Stack on chip shows only while its row is under the drag
    const rowName = await target.evaluate(element => element.closest("[data-testid=branch-row]")?.getAttribute("data-branch-name") ?? null)
    await page.mouse.up()
    // the drag ended: with a drop, or with its end alone
    await expect
      .poll(async () => {
        const now = await dragEnds(page)
        return now.ends > before.ends || now.drops > before.drops
      })
      .toBe(true)
    if (!took || attempt === DROP_ATTEMPTS || draggedRow === null || (await dragEnds(page)).drops > before.drops) {
      return
    }
    // the gesture again: the drag of the row, the row of the target (a target in a row, the Stack on chip, shows while
    // the row is under the drag), the target
    await page.mouse.move(0, 0)
    await startDrag(page, draggedRow)
    if (rowName !== null) {
      await page.locator(`[data-testid="branch-row"][data-branch-name="${rowName}"]`).hover()
    }
  }
}

/**
 * Drops on the center of `target` where it is on screen, as the user aims at it: no scroll into view and no check that
 * nothing covers it (`dropOn` hovers), for the zones of the drag bar over the content
 */
export async function dropAt(page: Page, target: Locator): Promise<void> {
  const box = (await target.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 4 })
  await page.mouse.up()
}

/** The selected text of the page and the file of the diff view it is in: the name a reveal of the diffs selected */
export async function selectedInDiffs(page: Page): Promise<{ text: string; path: string | null }> {
  return page.evaluate(() => {
    const selection = document.getSelection()
    const node = selection?.anchorNode ?? null
    const element = node instanceof Element ? node : (node?.parentElement ?? null)
    return { text: selection?.toString() ?? "", path: element?.closest<HTMLElement>("[data-testid=diff-view-file]")?.dataset.path ?? null }
  })
}
