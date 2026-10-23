// @spec spec/repository/cli-tools.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { teamcityBehindOnHomebrew } from "../helpers/cli-tools-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

test.describe("Settings → Tools", () => {
  test("should install a missing CLI through Homebrew, then show its version and how to sign it in", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ brew: true, latest: { teamcity: "1.6.0" } })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-teamcity")
    await expect(row).toContainText("Not installed")

    await row.getByRole("button", { name: "Install" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Installed teamcity 1.6.0" })).toBeVisible()
    expect(await repo.getCliToolsLog()).toContain("brew install jetbrains/utils/teamcity")
    await expect(row).toContainText("1.6.0")
    // signed in to no server: the command of its own sign-in
    await expect(row.getByText("Sign in in a terminal:")).toBeVisible()
    await expect(row.locator("pre")).toHaveText("teamcity auth login")
    await expect(row.getByRole("button", { name: "Install" })).toHaveCount(0)
  })

  test("should give the command to install a CLI without Homebrew", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-youtrack")
    await expect(row).toContainText("Not installed")
    await expect(row.getByText("Install it in a terminal:")).toBeVisible()
    await expect(row.locator("pre")).toHaveText("curl -fsSL https://jb.gg/youtrack-cli/install | bash")
    await expect(row.getByRole("button", { name: "Install" })).toHaveCount(0)
  })

  test("should update a CLI to its newest release, then the skill of the agents", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools(teamcityBehindOnHomebrew({ "teamcity.skill": "1.4.0" }))
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-teamcity")
    await expect(row.getByTestId("cli-tool-server")).toHaveText("ci.example · jane · expires Nov 21")
    await expect(row).toContainText("1.6.0 · 1.7.0 is out")

    await row.getByRole("button", { name: "Update to 1.7.0" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Updated teamcity to 1.7.0" })).toBeVisible()
    // the skill comes next: the agents have none
    await row.getByRole("button", { name: "Install Skill" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Updated the TeamCity skill of the agents to 1.4.0" })).toBeVisible()
    await expect(row).toContainText("1.7.0")
    await expect(row.getByRole("button", { name: "Update Skill" })).toHaveCount(0)
    const log = await repo.getCliToolsLog()
    expect(log).toContain("brew upgrade jetbrains/utils/teamcity")
    expect(log).toContain("teamcity skill install teamcity-cli --no-input --no-color")
  })

  test("should name why brew failed", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ brew: true, files: { "brew.fail": "Error: A `brew install` process has already locked /opt/homebrew/Cellar" } })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-youtrack")
    await row.getByRole("button", { name: "Install" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Cannot install youtrack" })).toContainText("another brew holds its lock")
    await expect(row.getByRole("button", { name: "Install" })).toBeVisible()
  })

  test("should stop a running install on Cancel, without an error", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ brew: true, files: { "brew.hang": "" } })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-youtrack")
    await row.getByRole("button", { name: "Install" }).click()
    await expect(row.getByTestId("cli-tool-progress")).toBeVisible()
    // brew runs, and would for a minute
    await expect.poll(() => repo.getCliToolsLog()).toContain("brew install jetbrains/utils/youtrack")
    await row.getByRole("button", { name: "Cancel" }).click()
    await expect(row.getByRole("button", { name: "Install" })).toBeVisible()
    await expect(row).toContainText("Not installed")
    await expect(window.locator("[data-slot=toast]")).toHaveCount(0)
  })

  test("should open from the line of Settings → Agents", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "agents" })
    await window.getByTestId("agent-tools").getByRole("button", { name: "Open Tools" }).click()
    await expectSettingsOpened(page, { page: "tools" }, 2)
  })

  test("should mark Tools in the sidebar while a CLI waits for you, until it is done", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({
      brew: true,
      installed: [{ tool: "youtrack", version: "0.6.0" }],
      latest: { youtrack: "0.7.0" },
      files: { "youtrack.auth": JSON.stringify({ url: "https://yt.example", login: "jane", status: "authenticated" }) },
    })
    const window = await openSettingsWindow(page, { page: "tools" })
    const tab = window.getByTestId("settings-tab-tools")
    await expect(tab.getByTestId("setup-dot")).toBeVisible()
    await window.getByTestId("cli-tool-youtrack").getByRole("button", { name: "Update to 0.7.0" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Updated youtrack to 0.7.0" })).toBeVisible()
    await emitTauriEvent(window, "setup-changed")
    await expect(tab.getByTestId("setup-dot")).toHaveCount(0)
  })
})
