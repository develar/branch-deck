// @spec openspec/specs/repository/unattended-mode/spec.md
import type { Page } from "@playwright/test"
import { expect, expectErrorLog, test } from "../fixtures/test-fixtures"
import { openPalette, runByKeyboard } from "../helpers/command-palette-helpers"
import { header } from "../helpers/header-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { emitTauriEvent, runAndWaitForAnswer } from "../helpers/sync-helpers"
import { TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"
import { closeToast, openToast } from "../helpers/toast-helpers"
import { expectClosed } from "../helpers/wait-helpers"

/** The settings of Claude Code of a user, as Claude Code writes them, with a hook of their own */
const CLAUDE_SETTINGS = `{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "audit.sh"
          }
        ]
      }
    ]
  }
}
`

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
  test("should turn the mode on from the palette, show its mark, add the hook to the settings of Claude Code, and turn it off by the mark, which gives the file back", async ({
    page,
    setupRepo,
  }) => {
    const repo = await setupRepo("simple")
    await repo.setClaudeSettings(CLAUDE_SETTINGS)
    // off: nothing of the mode in the title bar
    await expect(header(page).getByRole("button", { name: "Settings" })).toBeVisible()
    await expect(mark(page)).toHaveCount(0)

    await turnOnFromPalette(page)
    await expect(mark(page)).toHaveText("Unattended")
    // the hook went into the settings of Claude Code before the mode, beside the user's own hook
    const settings = JSON.parse((await repo.claudeSettings()) ?? "{}")
    expect(settings.hooks.PermissionRequest).toEqual([
      { matcher: "*", hooks: [{ type: "command", command: "/Users/test/.local/bin/brd", args: ["hook", "permission-request"], onFailure: "continue" }] },
    ])
    expect(settings.hooks.PreToolUse).toEqual([{ matcher: "Bash", hooks: [{ type: "command", command: "audit.sh" }] }])

    // the mark turns the mode off: it leaves, the night denied nothing, and the file is the user's again, byte for byte
    await runAndWaitForAnswer(page, "set_unattended_mode", () => mark(page).click())
    await expect(mark(page)).toHaveCount(0)
    const off = openToast(page, "Turned off Unattended Mode")
    await expect(off).toContainText("No permission request was denied while it was on")
    await expect(off.getByRole("button", { name: "Show Denied Requests…" })).toHaveCount(0)
    await closeToast(off)
    expect(await repo.claudeSettings()).toBe(CLAUDE_SETTINGS)

    // on again: the palette names the other way, which turns it off
    await turnOnFromPalette(page)
    await expect(mark(page)).toBeVisible()
    await openPalette(page)
    await runAndWaitForAnswer(page, "set_unattended_mode", () => runByKeyboard(page, "unattended", "Turn Off Unattended Mode"))
    await expect(mark(page)).toHaveCount(0)
    expect(await repo.claudeSettings()).toBe(CLAUDE_SETTINGS)
  })

  test("should stay off and say why when the settings of Claude Code cannot take the hook", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    await repo.setClaudeSettings('{ "model": ')
    expectErrorLog(page, /^Cannot turn on Unattended Mode: Cannot add the hook of Unattended Mode to .*: it is no JSON object/)
    await turnOnFromPalette(page)
    const error = openToast(page, "Cannot turn on Unattended Mode")
    await expect(error).toContainText("Cannot add the hook of Unattended Mode to")
    await expect(error).toContainText("it is no JSON object")
    await expect(mark(page)).toHaveCount(0)
    expect(await repo.claudeSettings()).toBe('{ "model": ')
  })

  test("should switch the mode from Unattended Mode of the app menu, with the toasts of the mode, and check the item as the mode is, whichever surface switched it", async ({
    page,
    setupRepo,
  }) => {
    // what the main window checks in the native menu, in the order it asked
    const checks: boolean[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/set_unattended_menu_checked")) {
        checks.push((request.postDataJSON() as { checked: boolean }).checked)
      }
    })
    const repo = await setupRepo("simple")
    await repo.setClaudeSettings(CLAUDE_SETTINGS)
    await expect.poll(() => checks.at(-1)).toBe(false)

    // the item of the app menu sends `unattended_mode` to the main window, which turns the mode on as the palette does
    await runAndWaitForAnswer(page, "set_unattended_mode", () => emitTauriEvent(page, "unattended_mode"))
    await expect(mark(page)).toBeVisible()
    await expect.poll(() => checks.at(-1)).toBe(true)
    expect(await repo.claudeSettings()).toContain('"permission-request"')

    // again: off, with the toast of a switch off
    await runAndWaitForAnswer(page, "set_unattended_mode", () => emitTauriEvent(page, "unattended_mode"))
    await expect(mark(page)).toHaveCount(0)
    await expect(openToast(page, "Turned off Unattended Mode")).toContainText("No permission request was denied while it was on")
    await expect.poll(() => checks.at(-1)).toBe(false)

    // the palette and the switch of Settings check it too
    await turnOnFromPalette(page)
    await expect.poll(() => checks.at(-1)).toBe(true)
    const settings = await openSettingsWindow(page, { page: "general" })
    await runAndWaitForAnswer(settings, "set_unattended_mode", () => settings.getByTestId("setting-unattended").getByRole("switch").click())
    await expect.poll(() => checks.at(-1)).toBe(false)
    await settings.close()

    // a switch on that fails leaves the item unchecked
    await repo.setClaudeSettings('{ "model": ')
    expectErrorLog(page, /^Cannot turn on Unattended Mode: Cannot add the hook of Unattended Mode to .*: it is no JSON object/)
    const count = checks.length
    await runAndWaitForAnswer(page, "set_unattended_mode", () => emitTauriEvent(page, "unattended_mode"))
    await expect(openToast(page, "Cannot turn on Unattended Mode")).toBeVisible()
    await expect(mark(page)).toHaveCount(0)
    expect(checks.slice(count)).not.toContain(true)
  })

  test("should switch the mode from Unattended Mode of the main menu of a custom title bar, checked as the mode is", async ({ page, setupRepo }) => {
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("simple")
    await page.getByTestId("main-menu-button").click()
    const item = page.getByRole("menuitemcheckbox", { name: "Unattended Mode" })
    await expect(item).toHaveAttribute("aria-checked", "false")
    await runAndWaitForAnswer(page, "set_unattended_mode", () => item.click())
    await expect(mark(page)).toBeVisible()
    await page.getByTestId("main-menu-button").click()
    await expect(item).toHaveAttribute("aria-checked", "true")
    await runAndWaitForAnswer(page, "set_unattended_mode", () => item.click())
    await expect(mark(page)).toHaveCount(0)
    await expect(openToast(page, "Turned off Unattended Mode")).toBeVisible()
  })

  test("should turn the mode on and off from Settings → General, the mark of the main window following it", async ({ page, setupRepo }) => {
    await setupRepo("simple")
    const settings = await openSettingsWindow(page, { page: "general" })
    const unattended = settings.getByTestId("setting-unattended").getByRole("switch")
    await expect(unattended).not.toBeChecked()
    await runAndWaitForAnswer(settings, "set_unattended_mode", () => unattended.click())
    await expect(unattended).toBeChecked()
    await expect(mark(page)).toBeVisible()

    await runAndWaitForAnswer(settings, "set_unattended_mode", () => unattended.click())
    await expect(unattended).not.toBeChecked()
    await expect(mark(page)).toHaveCount(0)
    await expect(openToast(settings, "Turned off Unattended Mode")).toContainText("No permission request was denied while it was on")
  })

  test("should tell on a switch off how many requests the night denied and list them", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    await turnOnFromPalette(page)
    await expect(mark(page)).toBeVisible()
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
