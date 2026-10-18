// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ConfigSetting } from "@/lib/bindings"
import {
  describeAppPrefixSource,
  describeDetectedGitHubRepository,
  describeFallback,
  describeSource,
  detectedGitHubRepositoryText,
  isValidGitHubRepository,
  overriddenReason,
  repositoryName,
  resetLabel,
  stacksOffReason,
  withAppPrefix,
  writeErrorTitle,
} from "@/lib/repository-config"

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
    expect(describeSource(setting({}))).toBeNull()
    expect(describeSource(setting({ scope: "global", value: true }))).toBe("Set in your global Git config")
    expect(describeSource(setting({ scope: "system", value: true }))).toBe("Set in the system Git config")
    expect(describeSource(setting({ scope: "local", localSet: true }))).toBe("Set in this repository")
  })

  it("says where the GitHub repository comes from while no config sets it", () => {
    expect(describeDetectedGitHubRepository({ fullName: "acme/app", source: "origin" })).toBe("Detected from origin")
    expect(describeDetectedGitHubRepository({ fullName: "acme/app", source: "spaceMirror" })).toBe("Detected from the Space mirror")
    expect(describeDetectedGitHubRepository(null)).toBe("None detected: neither origin nor its Space mirror is on GitHub")
    expect(detectedGitHubRepositoryText({ fullName: "acme/app", source: "spaceMirror" })).toBe("acme/app, detected from the Space mirror")
    expect(detectedGitHubRepositoryText(null)).toBe("no GitHub repository")
  })

  it("offers the global value, else the default, once the local one is unset", () => {
    expect(resetLabel(setting({ localSet: true }))).toBe("Use Default")
    expect(describeFallback(setting({ localSet: true }), "Off")).toBe("Off, the default")
    const global = setting({ localSet: true, fallbackValue: true, fallbackScope: "global" })
    expect(resetLabel(global)).toBe("Use Global")
    expect(describeFallback(global, "On")).toBe("On, set in your global Git config")
    expect(describeFallback(setting({ localSet: true }), "", "no GitHub repository")).toBe("no GitHub repository")
  })

  it("takes the prefix of General over a global git one, as the branches are named", () => {
    const prefix = (overrides: Partial<ConfigSetting<string | null>>): ConfigSetting<string | null> => ({
      value: null,
      scope: null,
      localSet: false,
      invalid: null,
      fallbackValue: null,
      fallbackScope: null,
      ...overrides,
    })
    const global = prefix({ value: "g", scope: "global", invalid: "g is not valid", fallbackValue: "g", fallbackScope: "global" })
    expect(withAppPrefix(global, "")).toBe(global)
    const shown = withAppPrefix(global, "jane")
    expect(shown).toEqual(prefix({ fallbackValue: "jane" }))
    expect(describeSource(shown)).toBeNull()
    expect(describeAppPrefixSource(global, "jane")).toBe("Set in General, which overrides your global Git config")
    expect(describeAppPrefixSource(prefix({}), "jane")).toBe("Set in General")
    expect(describeAppPrefixSource(prefix({}), "")).toBeNull()

    // the repository's own prefix stays; unsetting it gives General's
    const local = withAppPrefix(prefix({ value: "team", scope: "local", localSet: true, fallbackValue: "g", fallbackScope: "global" }), "jane")
    expect(local).toMatchObject({ value: "team", scope: "local", fallbackValue: "jane", fallbackScope: null })
    expect(resetLabel(local)).toBe("Use Default")
    expect(describeFallback(local, "jane", "jane, the prefix of General")).toBe("jane, the prefix of General")
    expect(describeAppPrefixSource(local, "jane")).toBeNull()
  })

  it("can't change a key the worktree config or the command line sets", () => {
    expect(overriddenReason(setting({ scope: "local" }))).toBeNull()
    expect(overriddenReason(setting({ scope: "worktree" }))).toBe("Set in config.worktree, which overrides .git/config: change it there")
    expect(overriddenReason(setting({ scope: "command" }))).toBe("Set by git -c or GIT_CONFIG_*, which overrides .git/config")
  })

  it("says why stacked branches can't be turned off, and what to do", () => {
    expect(stacksOffReason([])).toBeNull()
    expect(stacksOffReason([{ branch: "auth/login", beside: "auth" }])).toBe(
      "Turning this off makes login the branch auth/login, which git cannot keep next to auth. Unstack login first",
    )
    expect(
      stacksOffReason([
        { branch: "auth/login", beside: "auth" },
        { branch: "auth/session", beside: "auth" },
      ]),
    ).toBe("Turning this off renames login and 1 more to names like auth/login, which git cannot keep next to auth. Unstack them first")
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
