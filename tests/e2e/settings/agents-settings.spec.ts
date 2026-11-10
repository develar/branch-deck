// @spec openspec/specs/repository/settings/spec.md
import type { RepositoryConfig } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { expectSettingsOpened, openSettingsWindow, recordWrites } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { routeCommand, openTooltip } from "../helpers/wait-helpers"

/** The repository's name, as the page says it */
function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

test.describe("Settings → Agents", () => {
  test("should change a rule for every repository in the global config, and give the repository its own value", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const name = nameOf(repo.path)
    const window = await openSettingsWindow(page, { page: "agents" })
    const writes = recordWrites(window)
    const lease = window.getByTestId("setting-agentLease")
    const subjects = window.getByTestId("setting-agentSubjectBranch")
    const pullRequests = window.getByTestId("setting-agentPullRequest")
    // All repositories: the defaults, the key behind its copy button
    await expect(window.getByTestId("settings-scope-all")).toHaveAttribute("aria-selected", "true")
    await expect(lease.getByRole("button", { name: "Copy branchdeck.agentLease" })).toBeAttached()
    await expect(lease.getByRole("switch")).toBeChecked()
    await expect(subjects.getByRole("switch")).toBeChecked()
    // the pull request after a commit waits for the user unless a config turns it on
    await expect(pullRequests.getByRole("switch")).not.toBeChecked()
    await expect(window.getByTestId("setting-agentLease-source")).toHaveCount(0)

    // off for every repository: the global config of the test repository (its own file)
    await lease.getByRole("switch").click()
    await expect(lease.getByRole("switch")).not.toBeChecked()
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "agentLease", value: false }, scope: "global" }])

    // the tab of the repository: what holds there, from all repositories
    await window.getByTestId("settings-scope-repository").click()
    await expect(lease.getByRole("switch")).not.toBeChecked()
    await expect(window.getByTestId("setting-agentLease-source")).toHaveCount(0)
    // the subject branch off in this repository alone: .git/config, marked as its own
    await subjects.getByRole("switch").click()
    await expect(subjects.getByRole("switch")).not.toBeChecked()
    await expect(subjects).toHaveAttribute("data-own", "true")
    const source = window.getByTestId("setting-agentSubjectBranch-source")
    await expect(source).toHaveText(`Only in ${name}Reset`)

    // All repositories says the repository keeps its own value, and opens it
    await window.getByTestId("settings-scope-all").click()
    await expect(subjects.getByRole("switch")).toBeChecked()
    await expect(source).toHaveText(`${name} has its own value: OffOpen ${name}`)
    await source.getByRole("button", { name: `Open ${name}` }).click()
    await expect(window.getByTestId("settings-scope-repository")).toHaveAttribute("aria-selected", "true")

    // Reset: back to the value of every repository
    const reset = window.getByTestId("setting-agentSubjectBranch-source").getByRole("button", { name: "Reset" })
    await reset.hover()
    await expect(openTooltip(window)).toHaveText("Removes it from .git/config: back to On, the default")
    await reset.click()
    await expect(window.getByTestId("setting-agentSubjectBranch-source")).toHaveCount(0)
    await expect(subjects.getByRole("switch")).toBeChecked()
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
    await expect(subjects.getByRole("switch")).not.toBeChecked()
    await expect(window.getByTestId("setting-agentSubjectBranch-source")).toHaveText(`Only in ${name}Reset`)
  })

  test("should disable the rule of the repository while its worktree config sets it, and say so", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(
      page,
      { page: "agents", scope: "repository" },
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
    const lease = window.getByTestId("setting-agentLease")
    await expect(lease.getByRole("switch")).toBeDisabled()
    await expect(window.getByTestId("setting-agentLease-reason")).toHaveText("Set in config.worktree, which overrides .git/config: change it there")
    // the value of every repository can still change, and says the repository has its own
    await window.getByTestId("settings-scope-all").click()
    await expect(lease.getByRole("switch")).toBeEnabled()
    await expect(lease.getByRole("switch")).toBeChecked()
    await expect(window.getByTestId("setting-agentLease-source")).toContainText("has its own value: Off")
  })

  test("should turn the pull request of agents off where the repository lands by a push, with the way to its route", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    for (const change of [
      { key: "agentPullRequest", value: true },
      { key: "landing", value: "push" },
    ]) {
      const written = await page.request.post(`${TEST_SERVER_URL}/invoke/set_repository_config`, {
        headers: { "x-repository-id": repo.id },
        data: { repositoryPath: repo.path, change },
      })
      expect(written.ok()).toBe(true)
    }
    const window = await openSettingsWindow(page, { page: "agents", scope: "repository" })
    const pullRequests = window.getByTestId("setting-agentPullRequest")
    // Rust says the rule does not apply: no pull request on the route of a push
    const off = window.getByTestId("setting-agentPullRequest-not-applied")
    await expect(off).toContainText(AGENT_PULL_REQUEST_OFF)
    await expect(pullRequests.getByRole("switch")).toBeDisabled()
    await expect(pullRequests.getByRole("switch")).not.toBeChecked()
    // the way to the route: the main window opens Landing at it
    await off.getByRole("button", { name: "Change Route" }).click()
    await expectSettingsOpened(page, { page: "landing", scope: "repository", focus: "landing" }, 2)
  })

  test("should install the agent skill from the page, which Tools then names", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "agents" })
    const missing = window.getByTestId("agent-skill")
    await expect(missing).toHaveText("Agents read these rules once the skill is installedInstall Agent CLI")
    await missing.getByRole("button", { name: "Install Agent CLI" }).click()
    const toast = window.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })
    await expect(toast).toContainText("Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.")
    // the page is the place the toast of the main window opens: no action to open it
    await expect(toast.getByRole("button", { name: "Open Agent Settings" })).toHaveCount(0)
    await expect(missing).toHaveCount(0)
    await window.getByTestId("settings-tab-tools").click()
    const brd = window.getByTestId("agent-skill")
    await expect(brd).toContainText("/Users/test/.local/bin/brd")
    await expect(window.getByTestId("agent-skill-files")).toHaveText(
      "Agent skill: /Users/test/.claude/skills/branch-deck/SKILL.md, /Users/test/.agents/skills/branch-deck/SKILL.md",
    )
  })
})

/** Why agents open no pull request where the repository lands by a push, as Rust says it */
const AGENT_PULL_REQUEST_OFF = "The route into master is Push"

/** A repository without any `branchdeck.*` key */
const EMPTY_CONFIG: RepositoryConfig = {
  symbols: { value: "on", scope: null, localSet: false, invalid: null, fallbackValue: "on", fallbackScope: null },
  githubRepository: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  branchPrefix: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  pushSizeLimit: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  resolveByWords: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  landing: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  agentLease: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  agentSubjectBranch: { value: true, scope: null, localSet: false, invalid: null, fallbackValue: true, fallbackScope: null },
  agentPullRequest: { value: false, scope: null, localSet: false, invalid: null, fallbackValue: false, fallbackScope: null },
  reviewExperimental: { value: false, scope: null, localSet: false, invalid: null, fallbackValue: false, fallbackScope: null },
  agentPullRequestOff: null,
  companions: [],
  primary: null,
  detectedGithubRepository: null,
  landingDefault: { route: "pullRequest", onSpace: false },
  seq: 0,
}
