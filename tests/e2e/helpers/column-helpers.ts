import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"

/**
 * The left (or right) edges of the matches of `items`, rounded to the pixel, distinct ones in the order of the page: one value when
 * they line up in one column. The widths of the columns of the branch view follow its content a frame after it changed
 * (`ColumnRegistry`), so a check polls.
 */
export function columnLefts(items: Locator, edge: "left" | "right" = "left"): Promise<number[]> {
  return items.evaluateAll((elements, side) => [...new Set(elements.map(element => Math.round(element.getBoundingClientRect()[side])))], edge)
}

/**
 * `right` stands `gap` px after the end of `left`, once the columns are laid out: the observer writes the widths of the
 * tracks (and their gutters) in the rendering of the frame after the content changed, so a read in between finds the
 * old ones; what is painted has them
 */
export async function expectGap(left: Locator, right: Locator, gap: number): Promise<void> {
  await expect
    .poll(async () => {
      const [start, end] = await Promise.all([left.boundingBox(), right.boundingBox()])
      return start === null || end === null ? null : Math.round(end.x - (start.x + start.width))
    })
    .toBe(gap)
}

/** The matches of `items` (at least two) stand in one column: their left edges (or right ones) are one */
export async function expectColumnAligned(items: Locator, edge: "left" | "right" = "left"): Promise<void> {
  await expect.poll(() => items.count()).toBeGreaterThan(1)
  await expect.poll(() => columnLefts(items, edge)).toHaveLength(1)
}

/**
 * What the tracks of each match of `rows` paint lies inside its track, and no two tracks of a row overlap: the boxes
 * of the items (`[data-track]`) stand apart, and every box inside an item that nothing clips (a `truncate` span clips
 * its text) ends within it. A content wider than the cap of its track painted over the next track ("1h 57m · usually
 * ~1h 39m" of a running robot over the pull request chip) while the boxes stood apart. Returns the overlaps found, as
 * "row: track", so a check polls until the columns followed their content
 */
export function trackOverlaps(rows: Locator): Promise<string[]> {
  return rows.evaluateAll(elements => {
    const found: string[] = []
    const clipped = (element: Element, item: Element): boolean => {
      for (let parent = element.parentElement; parent !== null && parent !== item; parent = parent.parentElement) {
        const style = getComputedStyle(parent)
        if (style.overflowX !== "visible") {
          return true
        }
      }
      return false
    }
    for (const row of elements) {
      const name = (row as HTMLElement).dataset.branchName ?? "?"
      const items = [...row.querySelectorAll("[data-track]")].map(item => ({ item, box: item.getBoundingClientRect(), track: (item as HTMLElement).dataset.track }))
      const shown = items.filter(it => it.box.width > 0).sort((a, b) => a.box.left - b.box.left)
      for (let index = 1; index < shown.length; index++) {
        if (shown[index - 1]!.box.right > shown[index]!.box.left + 0.5) {
          found.push(`${name}: ${shown[index - 1]!.track} over ${shown[index]!.track}`)
        }
      }
      for (const { item, box, track } of items) {
        for (const inner of item.querySelectorAll("*")) {
          const rect = inner.getBoundingClientRect()
          if (rect.width === 0 || clipped(inner, item) || getComputedStyle(inner).position === "absolute") {
            continue
          }
          if (rect.left < box.left - 0.5 || rect.right > box.right + 0.5) {
            found.push(`${name}: ${track}`)
            break
          }
        }
      }
    }
    return found
  })
}

/** The steps and "⋯" of the branch rows stand in their columns, and no track of a row paints over another (`trackOverlaps`) */
export async function expectRowTracksInPlace(page: Page): Promise<void> {
  const rows = page.getByTestId("branch-row")
  await expectColumnAligned(page.getByTestId("next-step-slot"))
  await expectColumnAligned(rows.getByRole("button", { name: /^More actions for / }))
  await expect.poll(() => trackOverlaps(rows)).toEqual([])
}
