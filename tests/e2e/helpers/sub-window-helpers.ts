import type { Page } from "@playwright/test"

/** A sub-window the page opened (`open_sub_window`, recorded by the Tauri mock) */
export interface OpenedSubWindow<T = unknown> {
  windowId: string
  url: string
  title: string
  data: T
}

export async function getOpenedSubWindows<T = unknown>(page: Page): Promise<OpenedSubWindow<T>[]> {
  return page.evaluate(() => (window as unknown as { __e2eSubWindows?: OpenedSubWindow<T>[] }).__e2eSubWindows ?? [])
}

/**
 * Shows a sub-window in the page, as the app opens it: its route with the data as `window.__INIT_DATA__`. The mock
 * keeps the label `main` (another label would wait for a main window), so what only a sub-window does is not covered.
 */
export async function openSubWindowPage(page: Page, subWindow: OpenedSubWindow): Promise<void> {
  const repoId = new URL(page.url()).searchParams.get("repoId")
  await page.addInitScript(data => {
    ;(window as unknown as { __INIT_DATA__: unknown }).__INIT_DATA__ = data
  }, subWindow.data)
  await page.goto(`${subWindow.url}?repoId=${repoId}`)
}

/** The payloads of the events the page emitted for other windows (`emit`, recorded by the Tauri mock), in order */
export async function getEmittedEvents<T = unknown>(page: Page, event: string): Promise<T[]> {
  return page.evaluate(
    name => ((window as unknown as { __e2eEmittedEvents?: { event: string; payload: T }[] }).__e2eEmittedEvents ?? []).filter(it => it.event === name).map(it => it.payload),
    event,
  )
}
