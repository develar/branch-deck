import type { Locator, Page } from "@playwright/test"
import { expect } from "@playwright/test"

export function getCommitPreview(page: Page): Locator {
  return page.getByTestId("commit-preview")
}

/** Opens the preview of the commit row under the pointer with Space and waits until it has the focus */
export async function openCommitPreview(page: Page, row: Locator): Promise<Locator> {
  await row.hover()
  await page.keyboard.press("Space")
  const preview = getCommitPreview(page)
  await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
  return preview
}

/** Opens the preview of the commit row with Space and starts editing its message with E; returns the focused editor */
export async function editCommitMessage(page: Page, row: Locator): Promise<Locator> {
  const preview = await openCommitPreview(page, row)
  await page.keyboard.press("e")
  const editor = preview.getByTestId("commit-message-editor")
  await expect(editor).toBeFocused()
  return editor
}

/** Opens the Files section of the commit preview, closed at first, by its header; returns the tree of the files */
export async function openCommitFilesSection(preview: Locator): Promise<Locator> {
  const header = preview.getByTestId("commit-files-header")
  await expect(header).toHaveAttribute("aria-expanded", "false")
  await header.click()
  await expect(header).toHaveAttribute("aria-expanded", "true")
  return preview.getByRole("tree", { name: "Changed files" })
}

/** Chooses a whitespace policy in the Whitespace menu of `scope` (a preview, a review window), and closes the menu */
export async function chooseWhitespace(page: Page, scope: Locator, label: string): Promise<void> {
  await scope.getByRole("button", { name: "Whitespace" }).click()
  await page.getByRole("menuitemradio", { name: label, exact: true }).click()
  // a radio item keeps its menu open: Escape closes it, not the preview or the window
  await page.keyboard.press("Escape")
  await expect(page.getByRole("menu")).toHaveCount(0)
}
