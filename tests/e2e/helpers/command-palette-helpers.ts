import type { Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { emitTauriEvent, waitForTauriListener } from "./sync-helpers"

/** The command palette of the main window (⌘K) */
export function palette(page: Page) {
  return page.getByTestId("command-palette")
}

export function search(page: Page) {
  return palette(page).getByRole("combobox", { name: "Command or branch" })
}

/** ⌘K of the native menu (View → Command Palette…), which sends its command to the main window */
export async function openPalette(page: Page): Promise<void> {
  await waitForTauriListener(page, "open_command_palette")
  await emitTauriEvent(page, "open_command_palette")
  await expect(search(page)).toBeFocused()
}

/** Types `text` and presses Enter on the first option it keeps, whose label must be `label` (its key hint after it) */
export async function runByKeyboard(page: Page, text: string, label: string): Promise<void> {
  await search(page).fill(text)
  await expect(palette(page).getByRole("option").first()).toHaveText(new RegExp(`^${label.replace(/[.*+?^$()[\]{}|\\]/g, "\\$&")}`))
  await page.keyboard.press("Enter")
  await expect(palette(page)).toHaveCount(0)
}
