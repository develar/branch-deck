import type { Locator, Page } from "@playwright/test"
import type { ActivityWindowData } from "../../../src/lib/activity-window"
import { expect } from "../fixtures/base-test"
import { getOpenedSubWindows, openLinkedSubWindow, pressAndExpectOpened } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"

/** The Activity button of the header */
export function activityButton(page: Page): Locator {
  return page.getByTestId("activity-button")
}

/**
 * Runs `press`, which makes the main window open Activity, and returns that open, after `count` sub-windows were opened
 * before it (`pressAndExpectOpened`: read once the open answered, never by a poll after the press)
 */
export async function pressAndExpectActivityOpened(page: Page, press: () => Promise<unknown>, count: number): Promise<OpenedSubWindow<ActivityWindowData>> {
  const opened = await pressAndExpectOpened<ActivityWindowData>(page, press, "activity", count + 1)
  expect(opened.url).toBe("/activity")
  return opened
}

/** The Activity side panel of the main window */
export function activityPanel(page: Page): Locator {
  return page.getByTestId("activity-panel")
}

/** Opens the side panel by the header button, then its Open in Window: a window wide enough for the panel */
export async function openWindowFromPanel(page: Page): Promise<void> {
  await activityButton(page).click()
  await activityPanel(page).getByRole("button", { name: "Open in Window" }).click()
  await expect(activityPanel(page)).toHaveCount(0)
}

/**
 * Opens the Activity window by `open` (Open in Window of the side panel by default; where the panel has no room, an
 * entry point opens the window itself), then shows the window as a linked page of its own (`openLinkedSubWindow`): the
 * window reads the log itself, the main window tells it what only it knows and runs what it asks for
 */
export async function openActivityWindow(page: Page, open: () => Promise<unknown> = () => openWindowFromPanel(page)): Promise<Page> {
  const count = (await getOpenedSubWindows(page)).length
  const window = await openLinkedSubWindow(page, await pressAndExpectActivityOpened(page, open, count))
  await expect(window.getByTestId("activity-window")).toBeVisible()
  return window
}

/** The entry with this title; a branch name in it is a button, so the title is matched as the whole text */
export function activityEntry(scope: Page | Locator, title: string): Locator {
  const page = "page" in scope ? scope.page() : scope
  const escaped = title.replace(/[.*+?^$()[\]{}|\\]/g, "\\$&")
  return scope.getByTestId("activity-entry").filter({ has: page.getByTestId("activity-title").filter({ hasText: new RegExp(`^${escaped}$`) }) })
}
