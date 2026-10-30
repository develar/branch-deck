import type { Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"

/** What the Tauri mock counts of the page's commands (`window.__ipcStats`, tests/e2e/setup/tauri-mocks.ts) */
export interface IpcStats {
  invokes: Record<string, number>
  requestBytes: number
  responseBytes: number
  channelMessages: Record<string, number>
  channelBytes: number
  pending: number
}

type IpcWindow = { __ipcStats: IpcStats & { reset: () => void } }

/** Past the rest of the pointer that counts as the intent to look at a row (`INTENT_DWELL_MS` of usePreviewHotkeys) */
export const INTENT_DWELL_PAST_MS = 300

/** What the app invokes on its own timers, wherever they fire: the debounced writes of the settings, the check for updates after the start */
const APP_TIMERS: ReadonlySet<string> = new Set(["set_setting", "check_for_updates"])

/** The commands of the page but those of the app's own timers */
export function withoutAppTimers(invokes: Record<string, number>): Record<string, number> {
  return Object.fromEntries(Object.entries(invokes).filter(([name]) => !APP_TIMERS.has(name)))
}

/** Zeroes the counts of the page's IPC */
export function resetIpc(page: Page): Promise<void> {
  return page.evaluate(() => (window as unknown as IpcWindow).__ipcStats.reset())
}

/** The IPC of the page since its last reset, once no command is pending: a command's answer that starts another counts */
export async function settledIpc(page: Page): Promise<IpcStats> {
  await expect.poll(() => page.evaluate(() => (window as unknown as IpcWindow).__ipcStats.pending)).toBe(0)
  return page.evaluate(() => {
    const { invokes, requestBytes, responseBytes, channelMessages, channelBytes, pending } = (window as unknown as IpcWindow).__ipcStats
    return { invokes, requestBytes, responseBytes, channelMessages, channelBytes, pending }
  })
}

/** The commands the page invoked since its last reset, by name: none pending */
export async function settledInvokes(page: Page): Promise<Record<string, number>> {
  return (await settledIpc(page)).invokes
}

/** What the page asked of the backend when its first branch row came into the document (`__ipcStats` of the mock) */
export interface FirstRowsIpc {
  readonly invokes: Record<string, number>
  readonly channelMessages: Record<string, number>
}

/** Records the IPC of the page at its first branch row, for every load from now on (`firstRowsAsks`) */
export async function recordIpcAtFirstRows(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const observer = new MutationObserver(() => {
      if (document.querySelector('[data-testid="branch-row"]') === null) {
        return
      }
      observer.disconnect()
      const stats = (window as unknown as { __ipcStats: FirstRowsIpc }).__ipcStats
      Object.assign(window, { __e2eFirstRowsIpc: { invokes: { ...stats.invokes }, channelMessages: { ...stats.channelMessages } } })
    })
    observer.observe(document, { childList: true, subtree: true })
  })
}

/**
 * What every page tells the backend as it loads, which reads nothing of the repository: that it loaded (the show of
 * the window), the changes of the settings after the snapshot it was built with, and the demand of its subscription
 * once the subscription answered
 */
const PAGE_PROTOCOL: ReadonlySet<string> = new Set(["sub_window_loaded", "get_settings_changes", "set_demand"])

/** The commands the page asked before its first branch row, beside `PAGE_PROTOCOL`, and the channels of their answers */
export async function firstRowsAsks(page: Page): Promise<FirstRowsIpc | null> {
  const ipc = await page.evaluate(() => (window as unknown as { __e2eFirstRowsIpc?: FirstRowsIpc }).__e2eFirstRowsIpc ?? null)
  return ipc === null ? null : { ...ipc, invokes: Object.fromEntries(Object.entries(ipc.invokes).filter(([command]) => !PAGE_PROTOCOL.has(command))) }
}
