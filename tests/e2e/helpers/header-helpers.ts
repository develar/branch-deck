import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"

/** The minimum size of the main window (`min_inner_size` in src-tauri/src/lib.rs) */
export const MIN_WINDOW = { width: 960, height: 700 }
/** The toolbar of the main window, as high as the title bar it replaces on macOS and Windows */
export const HEADER_HEIGHT = 40
/** The controls of the header of a repository with an update and work on its way into master (`behind_upstream`) */
export const HEADER_CONTROLS = ["browse-repository-button", "update-project-button", "landing-indicator", "activity-button", "last-sync-time", "sync-button", "settings-button"]

export interface Box {
  x: number
  y: number
  width: number
  height: number
}

export async function boxOf(locator: Locator): Promise<Box> {
  await expect(locator).toBeVisible()
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

function overlaps(a: Box, b: Box): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

export function header(page: Page): Locator {
  return page.getByTestId("configuration-header")
}

/**
 * Zooms the page as the webview does (`useZoom`): a window of `size` shows `size / zoom` CSS px.
 * The page has to set the zoom itself (the `zoom` event), so that the toolbar knows it.
 */
export async function zoomPage(page: Page, zoom: number, size: { width: number; height: number }): Promise<void> {
  await page.setViewportSize({ width: Math.round(size.width / zoom), height: Math.round(size.height / zoom) })
}

/** The boxes of the controls `ids` of the header, in order */
export async function headerBoxes(page: Page, ids: readonly string[]): Promise<Box[]> {
  return Promise.all(ids.map(id => boxOf(header(page).getByTestId(id))))
}

/** The header is one line high and its controls are inside it, apart from each other; `zoom` is that of the page */
export async function expectHeaderControlsOnOneLine(page: Page, ids: string[], zoom = 1): Promise<Box> {
  const headerBox = await boxOf(header(page))
  // the toolbar keeps its size at any page zoom
  expect(headerBox.height * zoom).toBeCloseTo(HEADER_HEIGHT, 0)
  const boxes = await headerBoxes(page, ids)
  boxes.forEach((box, index) => {
    // inside the header: nothing wraps to a second line or is pushed out of it
    expect(box.x, `${ids[index]} starts in the header`).toBeGreaterThanOrEqual(headerBox.x)
    expect(box.x + box.width, `${ids[index]} ends in the header`).toBeLessThanOrEqual(headerBox.x + headerBox.width)
    expect(box.y, `${ids[index]} is on the header line`).toBeGreaterThanOrEqual(headerBox.y)
    expect(box.y + box.height, `${ids[index]} is on the header line`).toBeLessThanOrEqual(headerBox.y + headerBox.height)
    for (let other = index + 1; other < boxes.length; other++) {
      expect(overlaps(box, boxes[other]!), `${ids[index]} overlaps ${ids[other]}`).toBe(false)
    }
  })
  return headerBox
}

/** The controls of the header that a width shows or hides, with those of every width: the names of the views and the count */
export const HEADER_ROW = ["main-menu-button", ...HEADER_CONTROLS, "main-view-switcher", "branch-count", "window-controls"]

/**
 * What breaks the one line of the header at the width the page has: a control of `ids` outside the header or its line,
 * or two that overlap. One read of the page, so a sweep over the widths of a window stays quick; a control the page
 * doesn't render at this width (`display: none`, or none at all) is left out
 */
export async function headerLineProblems(page: Page, ids: readonly string[]): Promise<string[]> {
  const { headerBox, boxes } = await header(page).evaluate((root, ids) => {
    const boxOfElement = (element: Element) => {
      const rect = element.getBoundingClientRect()
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
    }
    return {
      headerBox: boxOfElement(root),
      boxes: ids.flatMap(id => {
        const element = root.querySelector(`[data-testid="${id}"]`)
        return element !== null && element.getClientRects().length > 0 ? [{ id, box: boxOfElement(element) }] : []
      }),
    }
  }, ids)
  return boxes.flatMap(({ id, box }, index) => [
    ...(box.x < headerBox.x || box.x + box.width > headerBox.x + headerBox.width ? [`${id} leaves the header`] : []),
    ...(box.y < headerBox.y || box.y + box.height > headerBox.y + headerBox.height ? [`${id} leaves the header line`] : []),
    ...boxes.slice(index + 1).flatMap(other => (overlaps(box, other.box) ? [`${id} overlaps ${other.id}`] : [])),
  ])
}

/** What the page set in the Dock, in order (`__e2eDockCalls` of the Tauri mock) */
export function getDockCalls(page: Page): Promise<Record<string, unknown>[]> {
  return page.evaluate(() => (window as unknown as { __e2eDockCalls?: Record<string, unknown>[] }).__e2eDockCalls ?? [])
}
