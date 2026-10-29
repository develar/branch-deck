import type { Page } from "@playwright/test"

/**
 * The renders of the page by key (`countRender` of src/lib/render-counts.ts): `row:<name>` a branch row, `table` the
 * branch table card, `island:<root>` the card of a stack island. A key that did not render since the last reset is absent
 */
export type RenderCounts = Record<string, number>

type RenderWindow = { __bdRenderCounts: Map<string, number> }

/**
 * Counts the renders of the page from its next load on: the app takes the map as its module loads, so this goes
 * before the setup of the repository loads the page
 */
export async function recordRenders(page: Page): Promise<void> {
  await page.addInitScript(() => Object.assign(window, { __bdRenderCounts: new Map() }))
}

/** Zeroes the render counts of the page, in the map the app holds */
export function resetRenders(page: Page): Promise<void> {
  return page.evaluate(() => (window as unknown as RenderWindow).__bdRenderCounts.clear())
}

/** The renders of the page since its last reset */
export function renderCounts(page: Page): Promise<RenderCounts> {
  return page.evaluate(() => Object.fromEntries((window as unknown as RenderWindow).__bdRenderCounts))
}

/** The keys of `counts` with the prefix `prefix` (`row:`, `island:`) that rendered */
export function renderedKeys(counts: RenderCounts, prefix: string): string[] {
  return Object.keys(counts).filter(key => key.startsWith(prefix))
}
