// @spec openspec/specs/repository/settings/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ConfigSetting, RepositoryConfig } from "@/lib/bindings"
import {
  describeDetectedGitHubRepository,
  describeScopedSource,
  detectedGitHubRepositoryText,
  everyRepositoryValue,
  globalWriteErrorTitle,
  isValidGitHubRepository,
  isValidPushSizeLimit,
  newer,
  overriddenReason,
  repositoryName,
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

const onOff = { valueText: (value: boolean) => (value ? "On" : "Off") }

describe("describeScopedSource", () => {
  it("says on the tab of the repository when the value is its own, with what Reset gives back", () => {
    // unset: the value of every repository, without a line
    expect(describeScopedSource(setting({}), "repository", "app", onOff)).toBeNull()
    expect(describeScopedSource(setting({ scope: "global", value: true, fallbackValue: true, fallbackScope: "global" }), "repository", "app", onOff)).toBeNull()
    // its own value, over the default and over a global one
    expect(describeScopedSource(setting({ value: true, scope: "local", localSet: true }), "repository", "app", onOff)).toEqual({
      kind: "own",
      text: "Only in app",
      reset: "Removes it from .git/config: back to Off, the default",
    })
    expect(describeScopedSource(setting({ value: false, scope: "local", localSet: true, fallbackValue: true, fallbackScope: "global" }), "repository", "app", onOff)).toEqual({
      kind: "own",
      text: "Only in app",
      reset: "Removes it from .git/config: back to On, from all repositories",
    })
    // an own value that changes nothing says so, so it can be dropped
    expect(describeScopedSource(setting({ value: true, scope: "local", localSet: true, fallbackValue: true, fallbackScope: "global" }), "repository", "app", onOff)).toMatchObject({
      kind: "own",
      text: "Same as all repositories",
    })
  })

  it("says on the tab of every repository where the selected one keeps a value of its own, and the system config", () => {
    expect(describeScopedSource(setting({ value: true, scope: "local", localSet: true }), "all", "app", onOff)).toEqual({ kind: "elsewhere", text: "app has its own value: On" })
    expect(describeScopedSource(setting({ value: true, scope: "worktree" }), "all", "app", onOff)).toEqual({ kind: "elsewhere", text: "app has its own value: On" })
    // without a repository, only the value of every repository
    expect(describeScopedSource(setting({ value: true, scope: "global" }), "all", "", onOff)).toBeNull()
    expect(describeScopedSource(setting({}), "all", "app", onOff)).toBeNull()
    expect(describeScopedSource(setting({ value: true, scope: "system" }), "all", "app", onOff)).toEqual({
      kind: "note",
      text: "Set in the system Git config; a change goes to your global Git config, which wins over it",
    })
    expect(describeScopedSource(setting({ value: true, scope: "local", localSet: true, fallbackScope: "system" }), "all", "", onOff)).toMatchObject({ kind: "note" })
  })

  it("shows on the tab of every repository its value, whatever the repository sets", () => {
    expect(everyRepositoryValue(setting({ value: true, scope: "local", localSet: true, fallbackValue: false, fallbackScope: "global" }))).toEqual({ value: false, scope: "global" })
    expect(everyRepositoryValue(setting({ value: true, scope: "command", fallbackValue: false }))).toEqual({ value: false, scope: null })
    expect(everyRepositoryValue(setting({ value: true, scope: "system" }))).toEqual({ value: true, scope: "system" })
  })

  it("names the default of a row only a repository has, and a global value of it as one of every repository", () => {
    const route = {
      valueText: (value: boolean) => (value ? "Push" : "Pull Request, the default off Space"),
      repositoryOnly: { defaultText: "Pull Request by default: the repository is not on Space" },
    }
    expect(describeScopedSource(setting({}), "repository", "app", route)).toEqual({ kind: "inherited", text: "Pull Request by default: the repository is not on Space" })
    expect(describeScopedSource(setting({ value: true, scope: "local", localSet: true }), "repository", "app", route)).toEqual({
      kind: "own",
      text: "Only in app",
      reset: "Removes it from .git/config: back to Pull Request, the default off Space",
    })
    expect(describeScopedSource(setting({ scope: "local", localSet: true }), "repository", "app", route)).toMatchObject({ text: "Same as the default" })
    expect(describeScopedSource(setting({ value: true, scope: "global" }), "repository", "app", route)).toEqual({
      kind: "inherited",
      text: "Set in your global Git config, which applies to every repository",
    })
    expect(describeScopedSource(setting({}), "repository", "app", { ...route, repositoryOnly: { defaultText: null } })).toBeNull()
  })

  it("can't change a key the worktree config or the command line sets", () => {
    expect(overriddenReason(setting({ scope: "local" }))).toBeNull()
    expect(overriddenReason(setting({ scope: "worktree" }))).toBe("Set in config.worktree, which overrides .git/config: change it there")
    expect(overriddenReason(setting({ scope: "command" }))).toBe("Set by git -c or GIT_CONFIG_*, which overrides .git/config")
    expect(describeScopedSource(setting({ scope: "worktree", localSet: true }), "repository", "app", onOff)).toEqual({
      kind: "locked",
      text: "Set in config.worktree, which overrides .git/config: change it there",
    })
  })
})

describe("the rows of a repository", () => {
  it("says where the GitHub repository comes from while no config sets it", () => {
    expect(describeDetectedGitHubRepository({ fullName: "acme/app", source: "origin" })).toBe("Detected from origin")
    expect(describeDetectedGitHubRepository({ fullName: "acme/app", source: "spaceMirror" })).toBe("Detected from the Space mirror")
    expect(describeDetectedGitHubRepository(null)).toBe("None detected: neither origin nor its Space mirror is on GitHub")
    expect(detectedGitHubRepositoryText({ fullName: "acme/app", source: "spaceMirror" })).toBe("acme/app, detected from the Space mirror")
    expect(detectedGitHubRepositoryText(null)).toBe("no GitHub repository")
  })

  it("takes the prefix of all repositories over a global git one, as the branches are named", () => {
    const prefix = (overrides: Partial<ConfigSetting<string | null>>): ConfigSetting<string | null> => ({
      value: null,
      scope: null,
      localSet: false,
      invalid: null,
      fallbackValue: null,
      fallbackScope: null,
      ...overrides,
    })
    const words = { valueText: (value: string | null) => value ?? "no prefix" }
    const global = prefix({ value: "g", scope: "global", invalid: "g is not valid", fallbackValue: "g", fallbackScope: "global" })
    expect(withAppPrefix(global, "")).toBe(global)
    const shown = withAppPrefix(global, "jane")
    expect(shown).toEqual(prefix({ fallbackValue: "jane" }))
    expect(describeScopedSource(shown, "repository", "app", words)).toBeNull()

    // the repository's own prefix stays; unsetting it gives the prefix of all repositories
    const local = withAppPrefix(prefix({ value: "team", scope: "local", localSet: true, fallbackValue: "g", fallbackScope: "global" }), "jane")
    expect(local).toMatchObject({ value: "team", scope: "local", fallbackValue: "jane", fallbackScope: null })
    expect(describeScopedSource(local, "repository", "app", words)).toEqual({ kind: "own", text: "Only in app", reset: "Removes it from .git/config: back to jane, the default" })
  })

  it("titles a failed write by what it changes", () => {
    expect(repositoryName("/home/jane/app/")).toBe("app")
    expect(writeErrorTitle({ key: "githubRepository", value: "acme/app" }, "C:\\repo\\app")).toBe("Cannot change the GitHub repository of app")
    expect(writeErrorTitle({ key: "pushSizeLimit", value: "200M" }, "/repo/app")).toBe("Cannot change the push size limit of app")
    expect(writeErrorTitle({ key: "agentLease", value: null }, "/repo/app")).toBe("Cannot change where agents work in app")
    expect(writeErrorTitle({ key: "agentSubjectBranch", value: true }, "/repo/app")).toBe("Cannot change how agents write commit subjects in app")
    expect(writeErrorTitle({ key: "agentPullRequest", value: true }, "/repo/app")).toBe("Cannot change whether agents open pull requests in app")
    expect(writeErrorTitle({ key: "landing", value: "push" }, "/repo/app")).toBe("Cannot change how app lands into master")
    expect(globalWriteErrorTitle("pushSizeLimit")).toBe("Cannot change the push size limit of every repository")
    expect(globalWriteErrorTitle("agentLease")).toBe("Cannot change where agents work in every repository")
    expect(globalWriteErrorTitle("resolveByWords")).toBe("Cannot change how every repository resolves conflicts by words")
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
  const config = (seq: number, resolveByWords: boolean, prefix: string | null): RepositoryConfig => ({
    symbols: { value: "on", scope: null, localSet: false, invalid: null, fallbackValue: "on", fallbackScope: null },
    githubRepository: text(null),
    branchPrefix: text(prefix),
    pushSizeLimit: text(null),
    resolveByWords: setting({ value: resolveByWords, scope: "local", localSet: true }),
    landing: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
    agentLease: setting({}),
    agentSubjectBranch: setting({}),
    agentPullRequest: setting({}),
    agentPullRequestOff: null,
    reviewExperimental: setting({}),
    companions: [],
    primary: null,
    detectedGithubRepository: null,
    landingDefault: { route: "pullRequest", onSpace: false },
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
