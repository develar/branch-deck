import type { Locator, Page } from "@playwright/test"
import { expect } from "@playwright/test"

/** The minimum size of the main window (`min_inner_size` in src-tauri/src/lib.rs) */
export const MIN_WINDOW = { width: 960, height: 700 }
/** The toolbar of the main window, as high as the title bar it replaces on macOS and Windows */
export const HEADER_HEIGHT = 40
/** The controls of the header of a repository with an update and something to push (`behind_upstream`) */
export const HEADER_CONTROLS = ["browse-repository-button", "update-project-button", "outgoing-indicator", "last-sync-time", "sync-button", "sync-settings-button"]

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
export async function zoomPage(page: Page, zoom: number, size: { width: number, height: number }): Promise<void> {
  await page.setViewportSize({ width: Math.round(size.width / zoom), height: Math.round(size.height / zoom) })
}

/** The header is one line high and its controls are inside it, apart from each other; `zoom` is that of the page */
export async function expectHeaderControlsOnOneLine(page: Page, ids: string[], zoom = 1): Promise<Box> {
  const headerBox = await boxOf(header(page))
  // the toolbar keeps its size at any page zoom
  expect(headerBox.height * zoom).toBeCloseTo(HEADER_HEIGHT, 0)
  const boxes = await Promise.all(ids.map(id => boxOf(header(page).getByTestId(id))))
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
