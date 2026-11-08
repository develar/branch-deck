import type { ConfigScope, ConfigSetting, DetectedGitHubRepository, LandingDefault, LandingSetting, RepositoryConfig, RepositoryConfigChange, SymbolMode } from "@/lib/bindings"

/*
 * The pages of the git config, Settings → Branches, Landing and Agents (spec/repository/settings.spec.md#scope): the
 * `branchdeck.*` git config of every repository and of the selected one, read and written by the backend (`crates/sync-core/src/repository_config.rs`), which `brd` reads too.
 */

/** A key of the page, as `RepositoryConfigChange` and the `repository_config_changed` event name it */
export type RepositoryConfigKey = RepositoryConfigChange["key"]

/** The payload of `repository_config_changed`, sent to the main window after a write (`RepositoryConfigChanged`) */
export interface RepositoryConfigChanged {
  readonly repositoryPath: string
  readonly key: RepositoryConfigKey | "companions"
}

/** The git config key of each setting, in the case Branch Deck writes it */
export const CONFIG_KEYS: Record<RepositoryConfigKey, string> = {
  symbols: "branchdeck.symbols",
  githubRepository: "branchdeck.githubRepository",
  branchPrefix: "branchdeck.branchPrefix",
  pushSizeLimit: "branchdeck.pushSizeLimit",
  resolveByWords: "branchdeck.resolveByWords",
  landing: "branchdeck.landing",
  agentLease: "branchdeck.agentLease",
  agentSubjectBranch: "branchdeck.agentSubjectBranch",
  agentPullRequest: "branchdeck.agentPullRequest",
  reviewExperimental: "branchdeck.reviewExperimental",
}

/** The last part of the path: the name the user knows the repository by */
export function repositoryName(repositoryPath: string): string {
  return repositoryPath.split(/[\\/]/).findLast(part => part !== "") ?? repositoryPath
}

/** `owner/name` without whitespace, as the backend validates it (`validate_github_repository`) */
export function isValidGitHubRepository(value: string): boolean {
  const parts = value.split("/")
  return !/\s/.test(value) && parts.length === 2 && parts.every(part => part !== "")
}

/** Why the backend refuses the text as a GitHub repository, in its words */
export function invalidGitHubRepositoryReason(value: string): string {
  return `${value} is not a GitHub repository: use owner/name`
}

/** The push size limit while no config sets it (`DEFAULT_PUSH_SIZE_LIMIT` of `crates/sync-core/src/push_size.rs`) */
export const DEFAULT_PUSH_SIZE_LIMIT = "50M"

/**
 * A size with a binary unit (`50M`, `1GiB`, `512`), `0` or off, as the backend takes it (`parse_push_size_limit`):
 * the field refuses other typing in place
 */
export function isValidPushSizeLimit(value: string): boolean {
  return /^(off|false|no)$/i.test(value) || /^\d+\s*(B|[KkMGTP](iB|B)?)?$/.test(value)
}

/** Why the backend refuses the text as a push size limit, as the field says it */
export function invalidPushSizeLimitReason(value: string): string {
  return `${value} is not a size such as ${DEFAULT_PUSH_SIZE_LIMIT} or 1G, nor 0 or off`
}

/** The modes of `branchdeck.symbols`, each with what it does (spec/commit/symbol-dependencies.spec.md) */
export const SYMBOL_MODES: ReadonlyArray<{ mode: SymbolMode; label: string; description: string }> = [
  { mode: "off", label: "Off", description: "Commits depend only on the lines they change" },
  { mode: "report", label: "Report Only", description: "Shown by brd commits, never used for the branches" },
  { mode: "on", label: "On", description: "Commits also depend on the commits that add the symbols they use" },
]

export function symbolModeLabel(mode: SymbolMode): string {
  return SYMBOL_MODES.find(it => it.mode === mode)?.label ?? mode
}

/**
 * The routes into master of `branchdeck.landing`, each with what it does (Settings → Landing,
 * spec/branch/landing-routes.spec.md): its label and the sentence of the route chosen
 */
export const LANDING_ROUTES: ReadonlyArray<{ setting: LandingSetting; label: string; description: string }> = [
  { setting: "patronus", label: "Safe Push", description: "A robot of Patronus checks each piece and pushes it into master; every pull request goes into master" },
  {
    setting: "push",
    label: "Push",
    description: "A push of its copy lands each piece, without a robot or the CI of a pull request; a straight stack is a stack of GitHub",
  },
  { setting: "pullRequest", label: "Pull Request", description: "Its pull request lands each piece, merged on GitHub; a straight stack is a stack of GitHub" },
]

export function landingRouteLabel(setting: LandingSetting): string {
  return LANDING_ROUTES.find(it => it.setting === setting)?.label ?? setting
}

/** The route of the repository while no config names it, and why: the host of `origin` (`LandingDefault`, Rust's) */
export function describeDefaultLanding(landingDefault: LandingDefault): string {
  const route = landingRouteLabel(landingDefault.route)
  return landingDefault.onSpace ? `${route} by default: the repository is on Space` : `${route} by default: the repository is not on Space`
}

/** The config a value comes from, as a sentence names it */
function scopeName(scope: ConfigScope): string {
  switch (scope) {
    case "system":
      return "the system Git config"
    case "global":
      return "your global Git config"
    case "local":
      return "this repository"
    case "worktree":
      return "config.worktree"
    case "command":
      return "git -c or GIT_CONFIG_*"
  }
}

/** "Set in your global Git config", "Set by git -c or GIT_CONFIG_*" */
function scopeText(scope: ConfigScope): string {
  return `${scope === "command" ? "Set by" : "Set in"} ${scopeName(scope)}`
}

/**
 * Why the page cannot change the key, which also says where its value comes from: the worktree config or the command
 * line sets it, which wins over `.git/config` (the backend refuses the write); null when it can
 */
export function overriddenReason(setting: ConfigSetting<unknown>): string | null {
  switch (setting.scope) {
    case "worktree":
      return `${scopeText("worktree")}, which overrides .git/config: change it there`
    case "command":
      return `${scopeText("command")}, which overrides .git/config`
    default:
      return null
  }
}

/** A scope of the repository itself, which wins over the value of every repository */
function isRepositoryScope(scope: ConfigScope | null): boolean {
  return scope === "local" || scope === "worktree" || scope === "command"
}

/**
 * The value of every repository (the tab "All repositories"): the global or system entry, else the default, whatever
 * the repository itself sets; with its scope, null for the default
 */
export function everyRepositoryValue<T>(setting: ConfigSetting<T>): { value: T; scope: ConfigScope | null } {
  return isRepositoryScope(setting.scope) ? { value: setting.fallbackValue, scope: setting.fallbackScope } : { value: setting.value, scope: setting.scope }
}

/**
 * The line under a row of a page of the git config (spec/repository/settings.spec.md#scope), at most one:
 * - `inherited`: the repository tab shows the default of a row only a repository has (the value of every repository has
 *   no line);
 * - `own`: the repository's `.git/config` sets it; `reset` is the tooltip of Reset, which removes it there;
 * - `elsewhere`: the tab of every repository, while the selected repository has a value of its own;
 * - `note`: the tab of every repository, while the system config sets the value;
 * - `locked`: the worktree config or the command line set it, which the page cannot change
 */
export type ScopedLine =
  | { kind: "inherited"; text: string }
  | { kind: "own"; text: string; reset: string }
  | { kind: "elsewhere"; text: string }
  | { kind: "note"; text: string }
  | { kind: "locked"; text: string }

/** How a row names a value and its default (`valueText`); `defaultText` is what an unset row only a repository has means */
export interface ScopedRowWords<T> {
  /** The value as the line names it: "On", "50M", "Pull Request, the default off Space" for an unset route */
  valueText: (value: T) => string
  /** A row only a repository has (the route, the GitHub repository): its line while no config sets it */
  repositoryOnly?: { defaultText: string | null }
}

/**
 * The line under a row of `setting` on the tab `scope`, for the repository `name` (empty without one): what holds there
 * and where it comes from, in the words of `words`; null when nothing needs saying
 */
export function describeScopedSource<T>(setting: ConfigSetting<T>, scope: "all" | "repository", name: string, words: ScopedRowWords<T>): ScopedLine | null {
  if (scope === "all") {
    if (name !== "" && isRepositoryScope(setting.scope)) {
      return { kind: "elsewhere", text: `${name} has its own value: ${words.valueText(setting.value)}` }
    }
    return everyRepositoryValue(setting).scope === "system"
      ? { kind: "note", text: "Set in the system Git config; a change goes to your global Git config, which wins over it" }
      : null
  }
  const overridden = overriddenReason(setting)
  if (overridden !== null) {
    return { kind: "locked", text: overridden }
  }
  const repositoryOnly = words.repositoryOnly
  if (setting.localSet) {
    const same = setting.value === setting.fallbackValue
    const text = same ? (repositoryOnly !== undefined && setting.fallbackScope === null ? "Same as the default" : "Same as all repositories") : `Only in ${name}`
    const origin = setting.fallbackScope === null ? (repositoryOnly === undefined ? ", the default" : "") : ", from all repositories"
    return { kind: "own", text, reset: `Removes it from .git/config: back to ${words.valueText(setting.fallbackValue)}${origin}` }
  }
  // the value of every repository, unmarked as in VS Code: only a value of its own has a line and a bar
  if (repositoryOnly === undefined) {
    return null
  }
  if (setting.scope === "global" || setting.scope === "system") {
    return { kind: "inherited", text: `${scopeText(setting.scope)}, which applies to every repository` }
  }
  return repositoryOnly.defaultText === null ? null : { kind: "inherited", text: repositoryOnly.defaultText }
}

/** Where the GitHub repository comes from while no config sets it: the remote it was detected from, or why there is none */
export function describeDetectedGitHubRepository(detected: DetectedGitHubRepository | null): string {
  if (detected === null) {
    return "None detected: neither origin nor its Space mirror is on GitHub"
  }
  return detected.source === "spaceMirror" ? "Detected from the Space mirror" : "Detected from origin"
}

/** The tooltip text of the GitHub repository's default: the detected one, or none */
export function detectedGitHubRepositoryText(detected: DetectedGitHubRepository | null): string {
  if (detected === null) {
    return "no GitHub repository"
  }
  return `${detected.fullName}, detected from ${detected.source === "spaceMirror" ? "the Space mirror" : "origin"}`
}

/**
 * The branch prefix row as the prefix order has it (spec/repository/settings.spec.md#branch-prefix-order): the prefix
 * of General wins over a global or system `branchdeck.branchPrefix`, so such a value is not shown as the effective one,
 * and General's prefix is what unsetting the repository's own gives. Unchanged without a prefix in General
 */
export function withAppPrefix(setting: ConfigSetting<string | null>, appPrefix: string): ConfigSetting<string | null> {
  if (appPrefix === "") {
    return setting
  }
  const hidden = setting.scope === "global" || setting.scope === "system"
  return {
    ...setting,
    ...(hidden && { value: null, scope: null, invalid: null }),
    fallbackValue: appPrefix,
    fallbackScope: null,
  }
}

/** The title of a failed write, in the words of the change (ux-writing: "Cannot <verb> <object>") */
export function writeErrorTitle(change: RepositoryConfigChange, repositoryPath: string): string {
  const name = repositoryName(repositoryPath)
  switch (change.key) {
    case "symbols":
      return `Cannot change the symbol dependencies of ${name}`
    case "githubRepository":
      return `Cannot change the GitHub repository of ${name}`
    case "branchPrefix":
      return `Cannot change the branch prefix of ${name}`
    case "pushSizeLimit":
      return `Cannot change the push size limit of ${name}`
    case "resolveByWords":
      return `Cannot change how ${name} resolves conflicts by words`
    case "landing":
      return `Cannot change how ${name} lands into master`
    case "agentLease":
      return `Cannot change where agents work in ${name}`
    case "agentSubjectBranch":
      return `Cannot change how agents write commit subjects in ${name}`
    case "agentPullRequest":
      return `Cannot change whether agents open pull requests in ${name}`
    case "reviewExperimental":
      return `Cannot change whether ${name} shows the review plan`
  }
}

/** The title of a failed write of the value of every repository (the global config), in the words of the change */
export function globalWriteErrorTitle(key: RepositoryConfigKey): string {
  switch (key) {
    case "symbols":
      return "Cannot change the symbol dependencies of every repository"
    case "githubRepository":
      return "Cannot change the GitHub repository of every repository"
    case "branchPrefix":
      return "Cannot change the branch prefix of every repository"
    case "pushSizeLimit":
      return "Cannot change the push size limit of every repository"
    case "resolveByWords":
      return "Cannot change how every repository resolves conflicts by words"
    case "landing":
      return "Cannot change how every repository lands into master"
    case "agentLease":
      return "Cannot change where agents work in every repository"
    case "agentSubjectBranch":
      return "Cannot change how agents write commit subjects in every repository"
    case "agentPullRequest":
      return "Cannot change whether agents open pull requests in every repository"
    case "reviewExperimental":
      return "Cannot change whether every repository shows the review plan"
  }
}

/**
 * Whether the page takes `answer`, the settings a read or a write of Settings answered, over the settings it shows. Two
 * writes at once (a prefix saved on blur, then a switch) answer in any order, and so does a read that a focus started
 * beside a write: each answer counts the config writes of the backend before its read (`seq`), so one that counts
 * fewer lacks a write the shown settings hold. One that counts as many holds the same writes and is taken: a read
 * again on focus finds a `git config` or a `brd` of meanwhile
 */
export function newer(shown: RepositoryConfig, answer: RepositoryConfig): boolean {
  return answer.seq >= shown.seq
}
