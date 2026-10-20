import type { Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * The left edges of the matches of `items`, rounded to the pixel, distinct ones in the order of the page: one value when
 * they line up in one column. The widths of the columns of the branch view follow its content a frame after it changed
 * (`ColumnRegistry`), so a check polls.
 */
export function columnLefts(items: Locator): Promise<number[]> {
  return items.evaluateAll(elements => [...new Set(elements.map(element => Math.round(element.getBoundingClientRect().left)))])
}

/** The matches of `items` (at least two) stand in one column: their left edges are one */
export async function expectColumnAligned(items: Locator): Promise<void> {
  await expect.poll(() => items.count()).toBeGreaterThan(1)
  await expect.poll(() => columnLefts(items)).toHaveLength(1)
}
