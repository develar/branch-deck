import type { ConfigScope, ConfigSetting, DetectedGitHubRepository, FlatNameClash, RepositoryConfig, RepositoryConfigChange, SymbolMode } from "@/lib/bindings"
import { joinNames } from "@/lib/text"

/*
 * Settings → Repository (spec/repository/settings.spec.md#repository): the `branchdeck.*` git config of the selected
 * repository, read and written by the backend (`crates/sync-core/src/repository_config.rs`), which `bd` reads too.
 */

/** A key of the page, as `RepositoryConfigChange` and the `repository_config_changed` event name it */
export type RepositoryConfigKey = RepositoryConfigChange["key"]

/** The payload of `repository_config_changed`, sent to the main window after a write (`RepositoryConfigChanged`) */
export interface RepositoryConfigChanged {
  readonly repositoryPath: string
  readonly key: RepositoryConfigKey
}

/** The git config key of each setting, in the case Branch Deck writes it */
export const CONFIG_KEYS: Record<RepositoryConfigKey, string> = {
  stacks: "branchdeck.stacks",
  symbols: "branchdeck.symbols",
  githubRepository: "branchdeck.githubRepository",
  branchPrefix: "branchdeck.branchPrefix",
  pushSizeLimit: "branchdeck.pushSizeLimit",
  resolveByWords: "branchdeck.resolveByWords",
  landing: "branchdeck.landing",
  agentLease: "branchdeck.agentLease",
  agentSubjectBranch: "branchdeck.agentSubjectBranch",
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
  { mode: "report", label: "Report Only", description: "Shown by bd commits, never used for the branches" },
  { mode: "on", label: "On", description: "Commits also depend on the commits that add the symbols they use" },
]

export function symbolModeLabel(mode: SymbolMode): string {
  return SYMBOL_MODES.find(it => it.mode === mode)?.label ?? mode
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
 * Where the value comes from, only when it is not the default: "Set in this repository", "Set in your global Git
 * config"; not the value, which the control shows. Null for the default
 */
export function describeSource(setting: ConfigSetting<unknown>): string | null {
  return setting.scope === null ? null : scopeText(setting.scope)
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

/**
 * Where the value of a setting with one value for every repository comes from (`scope` of the global or system entry,
 * null for the default, `defaultText` its value: "On", "50M"), and where a change goes: the global git config, which bd
 * reads too
 */
export function describeGlobalSource(scope: ConfigScope | null, defaultText: string): string {
  switch (scope) {
    case "global":
      return "Set in your global Git config, which bd reads too"
    case "system":
      return "Set in the system Git config. A change goes to your global Git config, which wins over it"
    default:
      return `${defaultText} by default. A change goes to your global Git config, which bd reads too`
  }
}

/**
 * Where the value of a setting of every repository comes from, and where a change goes: the global git config, unless
 * `.git/config` of the repository `name` sets the key (then the control changes that value, and the reset removes it);
 * empty for the worktree config or the command line, whose reason says it (`overriddenReason`)
 */
export function describeEveryRepositorySource(setting: ConfigSetting<unknown>, name: string, defaultText: string): string {
  switch (setting.scope) {
    case null:
    case "global":
    case "system":
      return describeGlobalSource(setting.scope, defaultText)
    case "local":
      return setting.fallbackScope === null ? `Set in .git/config of ${name}, for this repository only` : `Set in .git/config of ${name}, which overrides your global Git config`
    default:
      return ""
  }
}

/** "Use Global" when a global or system value applies once the local one is unset, else "Use Default" */
export function resetLabel(setting: ConfigSetting<unknown>): string {
  return setting.fallbackScope === null ? "Use Default" : "Use Global"
}

/**
 * What unsetting the local value gives, for the tooltip of its button after "back to": "On, set in your global Git
 * config", else the default ("Off, the default", or the words of the row: "acme/app, detected from origin")
 */
export function describeFallback(setting: ConfigSetting<unknown>, valueText: string, defaultText = `${valueText}, the default`): string {
  if (setting.fallbackScope === null) {
    return defaultText
  }
  const source = scopeText(setting.fallbackScope)
  return `${valueText}, ${source.charAt(0).toLowerCase()}${source.slice(1)}`
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

/**
 * Where the branch prefix comes from while the repository has none of its own and General has one: "Set in General",
 * "Set in General, which overrides your global Git config" when a global or system prefix gives way. Null otherwise
 */
export function describeAppPrefixSource(setting: ConfigSetting<string | null>, appPrefix: string): string | null {
  if (appPrefix === "") {
    return null
  }
  switch (setting.scope) {
    case null:
      return "Set in General"
    case "global":
    case "system":
      return `Set in General, which overrides ${scopeName(setting.scope)}`
    default:
      return null
  }
}

/**
 * Why stacked branches can't be turned off: the flat names of the stacked branches would clash with their parents
 * (`stacks_off_clashes`), and the sync would fail. Null without a clash
 */
export function stacksOffReason(clashes: readonly FlatNameClash[]): string | null {
  const first = clashes[0]
  if (first === undefined) {
    return null
  }
  const leafOf = (clash: FlatNameClash): string => clash.branch.split("/").at(-1) ?? clash.branch
  const leaf = leafOf(first)
  if (clashes.length === 1) {
    return `Turning this off makes ${leaf} the branch ${first.branch}, which git cannot keep next to ${first.beside}. Unstack ${leaf} first`
  }
  return `Turning this off renames ${joinNames(clashes.map(leafOf))} to names like ${first.branch}, which git cannot keep next to ${first.beside}. Unstack them first`
}

/** The title of a failed write, in the words of the change (ux-writing: "Cannot <verb> <object>") */
export function writeErrorTitle(change: RepositoryConfigChange, fallbackStacks: boolean, repositoryPath: string): string {
  const name = repositoryName(repositoryPath)
  switch (change.key) {
    case "stacks":
      return `Cannot turn ${(change.value ?? fallbackStacks) ? "on" : "off"} stacked branches in ${name}`
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
  }
}

/** What a page shows after a write answered: the settings, and whether to read them again */
export interface ConfigWriteOutcome {
  config: RepositoryConfig | null
  readAgain: boolean
}

/**
 * The writes of one page of Settings (`set_repository_config`), each answering the settings read again. Two writes at
 * once (a prefix saved on blur, then a switch) answer in any order, and the older answer may lack the newer write: while
 * writes overlap, an answer gives the page only the setting it wrote, and once the last of them answered the page reads
 * the settings again. A write alone gives the page its whole answer
 */
export class ConfigWrites {
  private running = 0
  private overlapped = false

  /** A write starts */
  start(): void {
    this.running++
    if (this.running > 1) {
      this.overlapped = true
    }
  }

  /** The write of `key` answered `answer` (null: it failed), while the page shows `shown` */
  finish(key: RepositoryConfigKey, answer: RepositoryConfig | null, shown: RepositoryConfig | null): ConfigWriteOutcome {
    this.running--
    if (!this.overlapped) {
      return { config: answer ?? shown, readAgain: false }
    }
    const config = answer === null || shown === null ? shown : withWrittenSetting(shown, answer, key)
    if (this.running > 0) {
      return { config, readAgain: false }
    }
    this.overlapped = false
    return { config, readAgain: true }
  }
}

/** `shown` with the setting of `key` that a write answered (and the clashes, which only the stacks change) */
function withWrittenSetting(shown: RepositoryConfig, answer: RepositoryConfig, key: RepositoryConfigKey): RepositoryConfig {
  const config: RepositoryConfig = { ...shown, [key]: answer[key] }
  if (key === "stacks") {
    config.stacksOffClashes = answer.stacksOffClashes
  }
  return config
}
