import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"

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
