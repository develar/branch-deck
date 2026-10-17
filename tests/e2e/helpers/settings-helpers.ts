import type { Locator, Page } from "@playwright/test"
import { expect } from "@playwright/test"
import type { SettingsTarget } from "../../../src/lib/settings-window"
import { getOpenedSubWindows, openLinkedSubWindow } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"
import { emitTauriEvent, waitForTauriListener } from "./sync-helpers"

/**
 * The Settings window the main window opened as the `count`-th sub-window (so exactly one more than before), at
 * `target`: the page and the field the entry point asked for
 */
export async function expectSettingsOpened(page: Page, target: SettingsTarget, count = 1): Promise<OpenedSubWindow<SettingsTarget>> {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  const opened = (await getOpenedSubWindows<SettingsTarget>(page)).at(-1)!
  expect(opened).toMatchObject({ windowId: "settings", url: "/settings", title: "Settings" })
  expect(opened.data).toEqual(target)
  return opened
}

/**
 * Opens Settings at `target` as the native menu does (`open_settings` to the main window), then shows the window as a
 * linked page of its own (`openLinkedSubWindow`): its writes of the settings reach the main window. `beforeLoad` routes
 * what the window asks the backend itself.
 */
export async function openSettingsWindow(page: Page, target: SettingsTarget = {}, options: { beforeLoad?: (window: Page) => Promise<unknown> } = {}): Promise<Page> {
  const count = (await getOpenedSubWindows(page)).length
  await waitForTauriListener(page, "open_settings")
  await emitTauriEvent(page, "open_settings", target)
  const window = await openLinkedSubWindow(page, await expectSettingsOpened(page, target, count + 1), options)
  await expect(window.getByTestId("settings-window")).toBeVisible()
  return window
}

/** The tab of a page in the sidebar */
export function settingsTab(window: Page, name: string): Locator {
  return window.getByRole("tablist", { name: "Settings pages" }).getByRole("tab", { name, exact: true })
}
