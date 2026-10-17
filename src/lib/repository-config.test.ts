// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ConfigSetting } from "@/lib/bindings"
import { describeFallback, describeSource, isValidGitHubRepository, overriddenReason, repositoryName, resetLabel, stacksOffReason, writeErrorTitle } from "@/lib/repository-config"

const setting = (overrides: Partial<ConfigSetting<boolean>>): ConfigSetting<boolean> => ({
  value: false,
  scope: null,
  localSet: false,
  invalid: null,
  fallbackValue: false,
  fallbackScope: null,
  ...overrides,
})

describe("isValidGitHubRepository", () => {
  // the cases of `invalid_values_are_refused_before_git_runs` in crates/sync-core/src/repository_config_test.rs
  it("takes owner/name without whitespace, as the backend does", () => {
    expect(isValidGitHubRepository("acme/app")).toBe(true)
    expect(isValidGitHubRepository("acme")).toBe(false)
    expect(isValidGitHubRepository("acme/app/x")).toBe(false)
    expect(isValidGitHubRepository("acme/my app")).toBe(false)
    expect(isValidGitHubRepository("/app")).toBe(false)
    expect(isValidGitHubRepository("acme/")).toBe(false)
  })
})

describe("the rows of Settings → Repository", () => {
  it("names the source only when it is not the default", () => {
    expect(describeSource(setting({}), "false")).toBeNull()
    expect(describeSource(setting({ scope: "global", value: true }), "true")).toBe("From your global Git config: true")
    expect(describeSource(setting({ scope: "local", localSet: true }), "report")).toBe("This repository")
  })

  it("offers the global value, else the default, once the local one is unset", () => {
    expect(resetLabel(setting({ localSet: true }))).toBe("Use Default")
    expect(describeFallback(setting({ localSet: true }), "Off")).toBe("Off, the default")
    const global = setting({ localSet: true, fallbackValue: true, fallbackScope: "global" })
    expect(resetLabel(global)).toBe("Use Global")
    expect(describeFallback(global, "On")).toBe("On, from your global Git config")
  })

  it("can't change a key the worktree config or the command line sets", () => {
    expect(overriddenReason(setting({ scope: "local" }))).toBeNull()
    expect(overriddenReason(setting({ scope: "worktree" }))).toBe("From the config of this worktree (config.worktree), which wins over .git/config: change it there")
    expect(overriddenReason(setting({ scope: "command" }))).toBe("From the command line (git -c or GIT_CONFIG_*), which wins over .git/config")
  })

  it("says why stacked branches can't be turned off, and what to do", () => {
    expect(stacksOffReason([])).toBeNull()
    expect(stacksOffReason([{ branch: "auth/login", beside: "auth" }])).toBe(
      "login is stacked by its prefix: turned off, auth/login is a flat branch git cannot keep beside auth. Unstack login first",
    )
    expect(
      stacksOffReason([
        { branch: "auth/login", beside: "auth" },
        { branch: "auth/session", beside: "auth" },
      ]),
    ).toBe("login and 1 more are stacked by their prefixes: turned off, auth/login is a flat branch git cannot keep beside auth. Unstack them first")
  })

  it("titles a failed write by what it changes", () => {
    expect(repositoryName("/home/jane/app/")).toBe("app")
    expect(writeErrorTitle({ key: "stacks", value: true }, false, "/repo/app")).toBe("Cannot turn on stacked branches in app")
    expect(writeErrorTitle({ key: "stacks", value: false }, false, "/repo/app")).toBe("Cannot turn off stacked branches in app")
    // an unset turns them as the global value or the default says
    expect(writeErrorTitle({ key: "stacks", value: null }, true, "/repo/app")).toBe("Cannot turn on stacked branches in app")
    expect(writeErrorTitle({ key: "githubRepository", value: "acme/app" }, false, "C:\\repo\\app")).toBe("Cannot change the GitHub repository of app")
  })
})
