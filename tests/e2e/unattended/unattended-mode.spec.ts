// @spec openspec/specs/repository/unattended-mode/spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { openPalette, runByKeyboard } from "../helpers/command-palette-helpers"
import { header } from "../helpers/header-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import { closeToast, openToast } from "../helpers/toast-helpers"
import { expectClosed } from "../helpers/wait-helpers"

/** The mark of Unattended Mode in the title bar, there only while the mode is on; a click turns it off */
function mark(page: Page) {
  return header(page).getByRole("button", { name: "Unattended Mode" })
}

/** Turns the mode on by Turn On Unattended Mode of the command palette */
async function turnOnFromPalette(page: Page): Promise<void> {
  await openPalette(page)
  await runAndWaitForAnswer(page, "set_unattended_mode", () => runByKeyboard(page, "unattended", "Turn On Unattended Mode"))
}

test.describe("Unattended Mode", () => {
  test("should turn the mode on from the palette, show its mark, offer the install of a missing hook, and turn it off by the mark", async ({ page, setupRepo }) => {
    await setupRepo("simple")
    // off: nothing of the mode in the title bar
    await expect(header(page).getByRole("button", { name: "Settings" })).toBeVisible()
    await expect(mark(page)).toHaveCount(0)

    await turnOnFromPalette(page)
    await expect(mark(page)).toHaveText("Unattended")
    // the agent CLI of the test is not installed, so the toast warns and offers it
    const warning = openToast(page, "Turned on Unattended Mode")
    await expect(warning).toContainText("Claude Code has no hook of Unattended Mode, so its permission requests still wait for you")
    await runAndWaitForAnswer(page, "install_agent_cli", () => warning.getByRole("button", { name: "Install Agent CLI" }).click())
    await expect(openToast(page, "Agent CLI installed")).toBeVisible()

    // the mark turns the mode off: it leaves, and the night denied nothing
    await runAndWaitForAnswer(page, "set_unattended_mode", () => mark(page).click())
    await expect(mark(page)).toHaveCount(0)
    const off = openToast(page, "Turned off Unattended Mode")
    await expect(off).toContainText("No permission request was denied while it was on")
    await expect(off.getByRole("button", { name: "Show Denied Requests…" })).toHaveCount(0)
    await closeToast(off)

    // on again, the hook installed now: the palette names the other way, which turns it off
    await turnOnFromPalette(page)
    await expect(mark(page)).toBeVisible()
    await openPalette(page)
    await runAndWaitForAnswer(page, "set_unattended_mode", () => runByKeyboard(page, "unattended", "Turn Off Unattended Mode"))
    await expect(mark(page)).toHaveCount(0)
  })

  test("should turn the mode on and off from Settings → General, the mark of the main window following it", async ({ page, setupRepo }) => {
    await setupRepo("simple")
    const settings = await openSettingsWindow(page, { page: "general" })
    const unattended = settings.getByTestId("setting-unattended").getByRole("switch")
    await expect(unattended).not.toBeChecked()
    await runAndWaitForAnswer(settings, "set_unattended_mode", () => unattended.click())
    await expect(unattended).toBeChecked()
    await expect(mark(page)).toBeVisible()
    await expect(openToast(settings, "Turned on Unattended Mode")).toBeVisible()

    await runAndWaitForAnswer(settings, "set_unattended_mode", () => unattended.click())
    await expect(unattended).not.toBeChecked()
    await expect(mark(page)).toHaveCount(0)
    await expect(openToast(settings, "Turned off Unattended Mode")).toContainText("No permission request was denied while it was on")
  })

  test("should tell on a switch off how many requests the night denied and list them", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    await turnOnFromPalette(page)
    await closeToast(openToast(page, "Turned on Unattended Mode"))
    // the night: a subagent's command and a fetch of the main session, as the hook writes them
    await repo.addUnattendedDenials([
      {
        time: 1_795_500_000,
        session_id: "4f1c",
        agent_id: "a7",
        agent_type: "general-purpose",
        cwd: "/Users/me/.branch-deck/pool/app-1/3/app",
        tool_name: "Bash",
        summary: "cat /etc/hosts",
      },
      { time: 1_795_503_600, session_id: "4f1c", cwd: "/Users/me/projects/app", tool_name: "WebFetch", summary: "https://example.com/docs" },
    ])

    await runAndWaitForAnswer(page, "set_unattended_mode", () => mark(page).click())
    const off = openToast(page, "Turned off Unattended Mode")
    await expect(off).toContainText("2 permission requests were denied while it was on")
    await off.getByRole("button", { name: "Show Denied Requests…" }).click()

    const dialog = page.getByTestId("denied-requests-dialog")
    await expect(dialog.getByRole("heading", { name: "Denied Requests" })).toBeVisible()
    await expect(dialog).toContainText("2 permission requests were denied while Unattended Mode was on")
    const requests = dialog.getByTestId("denied-request")
    await expect(requests).toHaveCount(2)
    await expect(requests.nth(0)).toContainText("Bash")
    await expect(requests.nth(0)).toContainText("cat /etc/hosts")
    await expect(requests.nth(0)).toContainText("general-purpose (a7) · app")
    await expect(requests.nth(1)).toContainText("WebFetch")
    await expect(requests.nth(1)).toContainText("https://example.com/docs")
    await expect(requests.nth(1)).toContainText("Main session · app")

    await page.keyboard.press("Escape")
    await expectClosed(dialog)
  })
})
