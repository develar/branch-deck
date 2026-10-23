// @spec spec/repository/cli-tools.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { teamcityBehindOnHomebrew, teamcitySignedIn } from "../helpers/cli-tools-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

test.describe("Settings → Tools", () => {
  test("should install a missing CLI through Homebrew with its agent skill, then show its version and its sign-in", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ brew: true, latest: { teamcity: "1.6.0" }, files: { "teamcity.skill": "1.4.0" } })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-teamcity")
    await expect(row).toContainText("Not installed")
    // the skill comes along by default
    await expect(row.getByRole("checkbox", { name: "Also install the agent skill teamcity-cli" })).toBeChecked()

    await row.getByRole("button", { name: "Install" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Installed teamcity 1.6.0 and its agent skill" })).toBeVisible()
    const log = await repo.getCliToolsLog()
    expect(log).toContain("brew install jetbrains/utils/teamcity")
    expect(log).toContain("teamcity skill install teamcity-cli --no-input --no-color")
    await expect(row).toContainText("1.6.0")
    await expect(row.getByTestId("cli-tool-skill")).toContainText("Agent skill teamcity-cli 1.4.0")
    // signed in to no server: the server for its own sign-in, no command to copy
    await expect(row.getByRole("textbox", { name: "TeamCity server" })).toHaveValue("")
    await expect(row.getByRole("button", { name: "Sign In" })).toBeDisabled()
    await expect(row.locator("pre")).toHaveCount(0)
    await expect(row.getByRole("button", { name: "Install" })).toHaveCount(0)
  })

  test("should install a CLI without its skill once its checkbox is off", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ brew: true })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-youtrack")
    const checkbox = row.getByRole("checkbox", { name: "Also install the agent skill youtrack-cli" })
    await checkbox.click()
    await expect(checkbox).not.toBeChecked()
    await row.getByRole("button", { name: "Install" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Installed youtrack 1.0.0" })).toBeVisible()
    expect(await repo.getCliToolsLog()).not.toContain("youtrack skill install youtrack-cli --quiet")
    // the line of the skill offers it
    await expect(row.getByTestId("cli-tool-skill")).toContainText("Agent skill youtrack-cli · not installed")
    await expect(row.getByTestId("cli-tool-skill").getByRole("button", { name: "Install Skill" })).toBeVisible()
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

  test("should update a CLI to its newest release, and install, update and remove its skill from the line of the skill", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools(teamcityBehindOnHomebrew({ "teamcity.skill": "1.4.0" }))
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-teamcity")
    await expect(row.getByTestId("cli-tool-server")).toContainText("ci.example · jane · expires Nov 21")
    await expect(row).toContainText("1.6.0 · 1.7.0 is out")

    await row.getByRole("button", { name: "Update to 1.7.0" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Updated teamcity to 1.7.0" })).toBeVisible()
    await expect(row).toContainText("1.7.0")
    expect(await repo.getCliToolsLog()).toContain("brew upgrade jetbrains/utils/teamcity")

    // the agents have no skill: the line installs it
    const skill = row.getByTestId("cli-tool-skill")
    await expect(skill).toContainText("Agent skill teamcity-cli · not installed")
    await skill.getByRole("button", { name: "Install Skill" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Installed the teamcity agent skill 1.4.0" })).toBeVisible()
    await expect(skill).toContainText("Agent skill teamcity-cli 1.4.0")

    // a newer one in the CLI: Update Skill
    await repo.setCliTools({ files: { "teamcity.skill": "1.5.0" } })
    await emitTauriEvent(window, "tauri://focus")
    await expect(skill).toContainText("Agent skill teamcity-cli 1.4.0 · teamcity has 1.5.0")
    await skill.getByRole("button", { name: "Update Skill" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Updated the teamcity agent skill to 1.5.0" })).toBeVisible()

    // Remove, and its Undo installs it again
    await skill.getByRole("button", { name: "Remove the teamcity agent skill" }).click()
    const removed = window.locator("[data-slot=toast]", { hasText: "Removed the teamcity agent skill" })
    await expect(removed).toBeVisible()
    await expect(skill).toContainText("· not installed")
    await removed.getByRole("button", { name: "Undo" }).click()
    await expect(skill).toContainText("Agent skill teamcity-cli 1.5.0")
    const log = await repo.getCliToolsLog()
    for (const line of ["install", "update", "remove"]) {
      expect(log).toContain(`teamcity skill ${line} teamcity-cli --no-input --no-color`)
    }
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
    // the status read after the update, which the app follows with \`setup-changed\` itself
    await expect(window.getByTestId("cli-tool-youtrack").getByRole("button", { name: "Update to 0.7.0" })).toHaveCount(0)
    await emitTauriEvent(window, "setup-changed")
    await expect(tab.getByTestId("setup-dot")).toHaveCount(0)
  })

  test("should sign teamcity in through its own browser sign-in, to the server an open names", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ installed: [{ tool: "teamcity", version: "1.6.0" }] })
    // the server of a robot's build, as the toast of a copied prompt opens it
    const window = await openSettingsWindow(page, { page: "tools", focus: "teamcity", server: "https://ci.example" })
    const row = window.getByTestId("cli-tool-teamcity")
    const field = row.getByRole("textbox", { name: "TeamCity server" })
    await expect(field).toHaveValue("https://ci.example")
    await row.getByRole("button", { name: "Sign In" }).click()
    await expect(window.locator("[data-slot=toast]", { hasText: "Signed in to ci.example as jane" })).toBeVisible()
    await expect(row.getByTestId("cli-tool-server")).toContainText("ci.example · jane · expires Dec 1")
    // the CLI's own sign-in, in a terminal of script: no token on its command line
    expect(await repo.getCliToolsLog()).toContain("teamcity auth login --server https://ci.example --no-color")
    await expect(field).toHaveCount(0)
  })

  test("should refuse a TeamCity without the browser sign-in at once, and sign in again from a server's line", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await repo.setCliTools({ installed: [{ tool: "teamcity", version: "1.6.0" }], files: { "teamcity.paste": "" } })
    const window = await openSettingsWindow(page, { page: "tools" })
    const row = window.getByTestId("cli-tool-teamcity")
    const field = row.getByRole("textbox", { name: "TeamCity server" })
    await expect(row.getByRole("button", { name: "Sign In" })).toBeDisabled()
    await field.click()
    await expect(field).toBeFocused()
    await field.fill("https://old.example")
    await field.press("Enter")
    await expect(window.locator("[data-slot=toast]", { hasText: "Cannot sign teamcity in" })).toContainText("https://old.example has no browser sign-in (TeamCity 2026.1+)")

    await repo.setCliTools({ files: { "teamcity.auth": teamcitySignedIn("2024-01-03T10:00:00Z") } })
    await emitTauriEvent(window, "tauri://focus")
    await expect(row.getByTestId("cli-tool-server")).toContainText("expires Jan 3")
    await row.getByTestId("cli-tool-server").getByRole("button", { name: "Sign In Again" }).click()
    await expect.poll(() => repo.getCliToolsLog()).toContain("teamcity auth login --server https://ci.example --no-color")
  })
})
