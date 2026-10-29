// @spec spec/repository/settings.spec.md
import type { RepositoryConfig } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { openSettingsWindow, recordWrites } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { routeCommand } from "../helpers/wait-helpers"

/** The repository's name, as the page says it */
function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

test.describe("Settings → Agents", () => {
  test("should change a rule for every repository in the global config, and give the repository its own value", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "agents" })
    const writes = recordWrites(window)
    const lease = window.getByTestId("setting-agentLease")
    const subjects = window.getByTestId("setting-agentSubjectBranch")
    await expect(lease).toContainText("branchdeck.agentLease")
    await expect(lease.getByRole("switch")).toBeChecked()
    await expect(window.getByTestId("setting-agentLease-source")).toHaveText("On by default. A change goes to your global Git config, which bd reads too")
    await expect(subjects.getByRole("switch")).toBeChecked()
    await expect(window.getByTestId("setting-agentSubjectBranch-source")).toHaveText("On by default. A change goes to your global Git config, which bd reads too")

    // off for every repository: the global config of the test repository (its own file)
    await lease.click()
    await expect(lease.getByRole("switch")).not.toBeChecked()
    await expect(window.getByTestId("setting-agentLease-source")).toHaveText("Set in your global Git config, which bd reads too")
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "agentLease", value: false }, scope: "global" }])

    // the subject branch off in this repository alone: .git/config
    const own = window.getByTestId("setting-agentSubjectBranch-repository")
    await expect(own).toContainText(`In ${nameOf(repo.path)}`)
    await expect(own.getByRole("button", { name: "Like Global" })).toHaveAttribute("aria-pressed", "true")
    await own.getByRole("button", { name: "Off", exact: true }).click()
    await expect(own.getByRole("button", { name: "Off", exact: true })).toHaveAttribute("aria-pressed", "true")
    await expect(subjects.getByRole("switch")).toBeChecked()
    // back to the value of every repository
    await own.getByRole("button", { name: "Like Global" }).click()
    await expect(own.getByRole("button", { name: "Like Global" })).toHaveAttribute("aria-pressed", "true")
    expect(writes.slice(1)).toEqual([
      { repositoryPath: repo.path, change: { key: "agentSubjectBranch", value: false }, scope: "local" },
      { repositoryPath: repo.path, change: { key: "agentSubjectBranch", value: null }, scope: "local" },
    ])

    // `git config branchdeck.agentSubjectBranch false` meanwhile: read again when the window gets the focus
    const local = await page.request.post(`${TEST_SERVER_URL}/invoke/set_repository_config`, {
      headers: { "x-repository-id": repo.id },
      data: { repositoryPath: repo.path, change: { key: "agentSubjectBranch", value: false } },
    })
    expect(local.ok()).toBe(true)
    await emitTauriEvent(window, "tauri://focus")
    await expect(own.getByRole("button", { name: "Off", exact: true })).toHaveAttribute("aria-pressed", "true")
  })

  test("should disable the buttons of the repository while its worktree config sets the rule, and say so", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(
      page,
      { page: "agents" },
      {
        beforeLoad: async window => {
          await routeCommand(window, "get_repository_config", () => ({
            status: "ok",
            data: {
              ...EMPTY_CONFIG,
              agentLease: { value: false, scope: "worktree", localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
            },
          }))
        },
      },
    )
    const own = window.getByTestId("setting-agentLease-repository")
    await expect(own.getByRole("button", { name: "On", exact: true })).toBeDisabled()
    await expect(own.getByRole("button", { name: "Like Global" })).toHaveAttribute("aria-pressed", "false")
    await expect(window.getByTestId("setting-agentLease-reason")).toHaveText("Set in config.worktree, which overrides .git/config: change it there")
    // the value of every repository can still change
    await expect(window.getByTestId("setting-agentLease").getByRole("switch")).toBeEnabled()
  })

  test("should install the agent skill from the page, and then name it", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "agents" })
    const skill = window.getByTestId("agent-skill")
    await expect(skill).toContainText("The agent skill is not installed")
    await skill.getByRole("button", { name: "Install Agent CLI" }).click()
    const toast = window.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })
    await expect(toast).toContainText("Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.")
    // the page is the place the toast of the main window opens: no action to open it
    await expect(toast.getByRole("button", { name: "Open Agent Settings" })).toHaveCount(0)
    await expect(skill).toContainText("Installed: agents find bd (/Users/test/.local/bin/bd) through /Users/test/.claude/skills/branch-deck/SKILL.md")
  })
})

/** A repository without any `branchdeck.*` key */
const EMPTY_CONFIG: RepositoryConfig = {
  stacks: { value: false, scope: null, localSet: false, invalid: null, fallbackValue: false, fallbackScope: null },
  symbols: { value: "on", scope: null, localSet: false, invalid: null, fallbackValue: "on", fallbackScope: null },
  githubRepository: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  branchPrefix: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  pushSizeLimit: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  resolveByWords: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  landing: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  agentLease: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  agentSubjectBranch: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  detectedGithubRepository: null,
  landingDefault: { route: "pullRequest", onSpace: false },
  stacksOffClashes: [],
  seq: 0,
}
