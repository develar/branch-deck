// @spec spec/repository/settings.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ConfigSetting, RepositoryConfig } from "@/lib/bindings"
import {
  describeAppPrefixSource,
  describeDetectedGitHubRepository,
  describeFallback,
  describeSource,
  detectedGitHubRepositoryText,
  isValidGitHubRepository,
  isValidPushSizeLimit,
  newer,
  overriddenReason,
  repositoryName,
  resetLabel,
  stacksOffReason,
  withAppPrefix,
  writeErrorTitle,
  describeGlobalSource,
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

describe("isValidPushSizeLimit", () => {
  // the cases of `the_limit_is_a_size_or_off` in crates/sync-core/src/push_size_test.rs and `a_size_is_a_number_and_a_binary_unit`
  // in crates/sync-utils/src/size_test.rs
  it("takes a size with a binary unit, 0 or off, as the backend does", () => {
    for (const value of ["50M", "1G", "200GiB", "10KB", "512", "0", "off", "OFF", "false", "1 G"]) {
      expect(isValidPushSizeLimit(value), value).toBe(true)
    }
    for (const value of ["", "lots", "1.5G", "200X", "-1G", "G"]) {
      expect(isValidPushSizeLimit(value), value).toBe(false)
    }
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
    ).toBe("Turning this off renames login and session to names like auth/login, which git cannot keep next to auth. Unstack them first")
  })

  it("titles a failed write by what it changes", () => {
    expect(repositoryName("/home/jane/app/")).toBe("app")
    expect(writeErrorTitle({ key: "stacks", value: true }, false, "/repo/app")).toBe("Cannot turn on stacked branches in app")
    expect(writeErrorTitle({ key: "stacks", value: false }, false, "/repo/app")).toBe("Cannot turn off stacked branches in app")
    // an unset turns them as the global value or the default says
    expect(writeErrorTitle({ key: "stacks", value: null }, true, "/repo/app")).toBe("Cannot turn on stacked branches in app")
    expect(writeErrorTitle({ key: "githubRepository", value: "acme/app" }, false, "C:\\repo\\app")).toBe("Cannot change the GitHub repository of app")
    expect(writeErrorTitle({ key: "pushSizeLimit", value: "200M" }, false, "/repo/app")).toBe("Cannot change the push size limit of app")
    expect(writeErrorTitle({ key: "agentLease", value: null }, false, "/repo/app")).toBe("Cannot change where agents work in app")
    expect(writeErrorTitle({ key: "agentSubjectBranch", value: true }, false, "/repo/app")).toBe("Cannot change how agents write commit subjects in app")
    expect(writeErrorTitle({ key: "landing", value: "push" }, false, "/repo/app")).toBe("Cannot change how app lands into master")
  })
})

describe("describeGlobalSource", () => {
  it("says where the value of every repository comes from, and that a change goes to the global config", () => {
    expect(describeGlobalSource(null, "On")).toBe("On by default. A change goes to your global Git config, which bd reads too")
    expect(describeGlobalSource(null, "50M")).toBe("50M by default. A change goes to your global Git config, which bd reads too")
    expect(describeGlobalSource("global", "Off")).toBe("Set in your global Git config, which bd reads too")
    expect(describeGlobalSource("system", "On")).toBe("Set in the system Git config. A change goes to your global Git config, which wins over it")
  })
})

describe("newer", () => {
  const text = (value: string | null): ConfigSetting<string | null> => ({
    value,
    scope: value === null ? null : "local",
    localSet: value !== null,
    invalid: null,
    fallbackValue: null,
    fallbackScope: null,
  })
  /** The settings an answer read after `seq` writes of the backend */
  const config = (seq: number, stacks: boolean, prefix: string | null): RepositoryConfig => ({
    stacks: setting({ value: stacks, scope: "local", localSet: true }),
    symbols: { value: "on", scope: null, localSet: false, invalid: null, fallbackValue: "on", fallbackScope: null },
    githubRepository: text(null),
    branchPrefix: text(prefix),
    pushSizeLimit: text(null),
    resolveByWords: setting({}),
    landing: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
    agentLease: setting({}),
    agentSubjectBranch: setting({}),
    reviewExperimental: setting({}),
    detectedGithubRepository: null,
    landingDefault: { route: "pullRequest", onSpace: false },
    stacksOffClashes: [],
    seq,
  })
  /** What the page shows once the answers arrived in this order */
  const shownAfter = (shown: RepositoryConfig, ...answers: RepositoryConfig[]) => answers.reduce((page, answer) => (newer(page, answer) ? answer : page), shown)

  it("keeps the newest of answers that arrive in reverse order", () => {
    const read = config(0, false, "jane")
    // the switch is flipped, then a prefix saved while its write runs: the switch's answer was read before the prefix
    const switchAnswer = config(1, true, "jane")
    const prefixAnswer = config(2, true, "team")
    expect(shownAfter(read, prefixAnswer, switchAnswer)).toBe(prefixAnswer)
    expect(shownAfter(read, switchAnswer, prefixAnswer)).toBe(prefixAnswer)
    // a read that a focus started before the writes answers last
    expect(shownAfter(read, prefixAnswer, switchAnswer, config(0, false, "jane"))).toBe(prefixAnswer)
  })

  it("takes an answer of as many writes: a read again finds a git config of meanwhile", () => {
    const written = config(2, true, "team")
    const readAgain = config(2, true, "bd")
    expect(shownAfter(written, readAgain)).toBe(readAgain)
  })
})
