import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import type { TitleBar } from "../../../src/lib/title-bar"

/** What the backend decides on each platform (`title_bar_script` in src-tauri/src/title_bar.rs) */
export const TITLE_BARS = {
  macos: { kind: "overlay", doubleClick: "toggle-maximize", checkForUpdates: false },
  windows: {
    kind: "custom",
    controls: { start: [], end: ["minimize", "maximize", "close"], style: "windows" },
    doubleClick: "toggle-maximize",
    checkForUpdates: false,
    cornerRadius: 0,
  },
  /** Ubuntu's default `button-layout`, `appmenu:minimize,maximize,close` */
  gnome: {
    kind: "custom",
    controls: { start: [], end: ["minimize", "maximize", "close"], style: "gnome" },
    doubleClick: "toggle-maximize",
    checkForUpdates: false,
    cornerRadius: 12,
  },
  /** GNOME's default `appmenu:close` */
  gnomeClose: { kind: "custom", controls: { start: [], end: ["close"], style: "gnome" }, doubleClick: "toggle-maximize", checkForUpdates: false, cornerRadius: 0 },
  /** A split layout, `close:minimize,maximize` */
  gnomeSplit: {
    kind: "custom",
    controls: { start: ["close"], end: ["minimize", "maximize"], style: "gnome" },
    doubleClick: "toggle-maximize",
    checkForUpdates: false,
    cornerRadius: 0,
  },
  kde: { kind: "custom", controls: { start: [], end: ["minimize", "maximize", "close"], style: "kde" }, doubleClick: "toggle-maximize", checkForUpdates: false, cornerRadius: 0 },
} satisfies Record<string, TitleBar>

/** Injects the title bar before the page loads, as the window builder does */
export async function useTitleBar(page: Page, titleBar: TitleBar): Promise<void> {
  await page.addInitScript(value => {
    ;(window as unknown as { __BD_TITLE_BAR__: unknown }).__BD_TITLE_BAR__ = value
  }, titleBar)
}

/** The window actions the page asked of its window plugin (minimize, toggle_maximize, close…), in order */
export function windowCalls(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])
}

/** Opens View of ☰ (a custom title bar): File opens first, the open menu has the focus, → goes to View */
export async function openMainMenuView(page: Page): Promise<void> {
  await page.getByTestId("main-menu-button").click()
  await expect(page.getByRole("menuitem", { name: /Sync Branches/ })).toBeVisible()
  await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[role=menu]") !== null)).toBe(true)
  await page.keyboard.press("ArrowRight")
}
