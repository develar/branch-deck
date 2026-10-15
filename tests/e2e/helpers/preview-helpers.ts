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
