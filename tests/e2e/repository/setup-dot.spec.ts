// @spec spec/repository/setup-attention.spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { teamcityBehindOnHomebrew, teamcitySignedIn } from "../helpers/cli-tools-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

/** The main window reads the items again, as once the CLIs were read (`setup-changed`), and opens the dot */
async function openSetup(page: Page) {
  await emitTauriEvent(page, "setup-changed")
  const button = page.getByTestId("settings-button")
  await expect(button.getByTestId("setup-dot")).toBeVisible()
  await button.click()
  return page.getByTestId("setup-popover")
}

test.describe("Setup dot", () => {
  test("should open Settings at once while nothing waits", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const button = page.getByTestId("settings-button")
    await expect(button).toHaveAccessibleName("Settings")
    await expect(button.getByTestId("setup-dot")).toHaveCount(0)
    await button.click()
    await expectSettingsOpened(page, {})
  })

  test("should keep a downloaded update of the app in the dot, not in a toast, and install it from there", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setAvailableUpdate("1.2.0")
    await page.reload()
    // the silent check after startup finds it
    const button = page.getByTestId("settings-button")
    await expect(button.getByTestId("setup-dot")).toBeVisible({ timeout: 10_000 })
    await expect(button).toHaveAccessibleName("Settings · 1 to do")
    await expect(page.locator("[data-slot=toast]", { hasText: "Update available" })).toHaveCount(0)

    await button.click()
    const item = page.getByTestId("setup-popover").getByTestId("setup-item")
    await expect(item).toHaveText(/Branch Deck 1\.2\.0 is ready to install/)
    await item.getByRole("button", { name: "Install and Restart" }).click()
    // the test server installs nothing: the install reached it
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot install the update" })).toContainText("no updater")
  })

  test("should find an update after the app has stayed open for six hours", async ({ page, setupRepo }) => {
    const silentCheck = page.waitForResponse(response => response.url().endsWith("/invoke/check_for_updates"))
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const button = page.getByTestId("settings-button")
    await expect(button).toHaveAccessibleName("Settings")
    await page.clock.fastForward(3_000)
    await silentCheck
    await expect(button.getByTestId("setup-dot")).toHaveCount(0)

    await repo.setAvailableUpdate("1.2.0")
    const periodicCheck = page.waitForResponse(response => response.url().endsWith("/invoke/check_for_updates"))
    await page.clock.fastForward(6 * 60 * 60 * 1000)
    await periodicCheck

    await expect(button.getByTestId("setup-dot")).toBeVisible()
    await button.click()
    await expect(page.getByTestId("setup-popover").getByTestId("setup-item")).toHaveText(/Branch Deck 1\.2\.0 is ready to install/)
  })

  test("should show a failed update check and clear it when Check Again succeeds", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setUpdateCheckFailure("Network unavailable")
    const silentCheck = page.waitForResponse(response => response.url().endsWith("/invoke/check_for_updates"))
    await page.reload()
    const button = page.getByTestId("settings-button")
    await expect(button).toHaveAccessibleName("Settings")
    await page.clock.fastForward(3_000)
    await silentCheck

    const item = (await openSetup(page)).getByTestId("setup-item")
    await expect(item).toContainText("Cannot check for updates")
    await expect(item).toContainText("Network unavailable")

    await repo.setUpdateCheckFailure(null)
    await item.getByRole("button", { name: "Check Again" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "is up to date" })).toBeVisible()
    await expect(button.getByTestId("setup-dot")).toHaveCount(0)
  })

  test("should update a CLI from the dot, and skip a version until a newer one", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools(teamcityBehindOnHomebrew())
    const popover = await openSetup(page)
    const item = popover.getByTestId("setup-item")
    await expect(item).toHaveText(/teamcity 1\.7\.0 is out/)
    await item.getByRole("button", { name: "Update" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Updated teamcity to 1.7.0" })).toBeVisible()
    expect(await repo.getCliToolsLog()).toContain("brew upgrade jetbrains/utils/teamcity")
    await expect(page.getByTestId("settings-button").getByTestId("setup-dot")).toHaveCount(0)

    await repo.setCliTools({ latest: { teamcity: "1.8.0" } })
    const newer = (await openSetup(page)).getByTestId("setup-item")
    await expect(newer).toHaveText(/teamcity 1\.8\.0 is out/)
    await newer.getByRole("button", { name: "Skip This Version" }).click()
    await expect(page.getByTestId("settings-button").getByTestId("setup-dot")).toHaveCount(0)
    // still skipped when the items are read again
    await emitTauriEvent(page, "setup-changed")
    await expect(page.getByTestId("settings-button")).toHaveAccessibleName("Settings")
  })

  test("should remind of an expiring sign-in tomorrow, and open Tools to sign in", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ installed: [{ tool: "teamcity", version: "1.6.0" }], files: { "teamcity.auth": teamcitySignedIn("2024-01-05T10:00:00Z") } })
    const item = (await openSetup(page)).getByTestId("setup-item")
    await expect(item).toHaveText(/The teamcity sign-in to ci\.example expires Jan 5/)
    await item.getByRole("button", { name: "Sign In" }).click()
    await expectSettingsOpened(page, { page: "tools", focus: "teamcity" })

    const again = (await openSetup(page)).getByTestId("setup-item")
    await again.getByRole("button", { name: "Remind Tomorrow" }).click()
    await expect(page.getByTestId("settings-button").getByTestId("setup-dot")).toHaveCount(0)
  })

  test("should keep an expired sign-in without a skip", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ installed: [{ tool: "teamcity", version: "1.6.0" }], files: { "teamcity.auth": teamcitySignedIn("2024-01-01T10:00:00Z") } })
    const item = (await openSetup(page)).getByTestId("setup-item")
    await expect(item).toHaveText(/The teamcity sign-in to ci\.example expired/)
    await expect(item.getByRole("button", { name: "Sign In" })).toBeVisible()
    await expect(item.getByRole("button")).toHaveCount(1)
  })
})
