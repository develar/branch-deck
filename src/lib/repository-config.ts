import type { ConfigScope, ConfigSetting, FlatNameClash, RepositoryConfigChange, SymbolMode } from "@/lib/bindings"

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

/** Why the backend refuses the text as a branch prefix, in its words (`validate_branch_prefix`) */
export function invalidBranchPrefixReason(value: string): string {
  return `${value} is not a valid branch prefix: no spaces, none of ~^:?*[\\, no .. and no @{`
}

/** The modes of `branchdeck.symbols`, each with what it does (spec/commit/symbol-dependencies.spec.md) */
export const SYMBOL_MODES: ReadonlyArray<{ mode: SymbolMode; label: string; description: string }> = [
  { mode: "off", label: "Off", description: "Not read: only the changed lines link commits" },
  { mode: "report", label: "Report Only", description: "Reported by bd commits and the checks; they decide nothing" },
  { mode: "on", label: "On", description: "The symbols also order and stack the branches" },
]

export function symbolModeLabel(mode: SymbolMode): string {
  return SYMBOL_MODES.find(it => it.mode === mode)?.label ?? mode
}

/** Where the effective value comes from, as the row says it before the value */
function scopeText(scope: ConfigScope): string {
  switch (scope) {
    case "system":
      return "From the system Git config"
    case "global":
      return "From your global Git config"
    case "local":
      return "This repository"
    case "worktree":
      return "From the config of this worktree"
    case "command":
      return "From the command line"
  }
}

/**
 * The source of the value, only when it is not the default: "From your global Git config: true"; the local config is
 * "This repository" (the control shows its value). Null for the default
 */
export function describeSource(setting: ConfigSetting<unknown>, valueText: string): string | null {
  if (setting.scope === null) {
    return null
  }
  return setting.scope === "local" ? scopeText("local") : `${scopeText(setting.scope)}: ${valueText}`
}

/**
 * Why the page cannot change the key, which also says where its value comes from: the worktree config or the command
 * line sets it, which wins over `.git/config` (the backend refuses the write); null when it can
 */
export function overriddenReason(setting: ConfigSetting<unknown>): string | null {
  switch (setting.scope) {
    case "worktree":
      return "From the config of this worktree (config.worktree), which wins over .git/config: change it there"
    case "command":
      return "From the command line (git -c or GIT_CONFIG_*), which wins over .git/config"
    default:
      return null
  }
}

/** "Use Global" when a global or system value applies once the local one is unset, else "Use Default" */
export function resetLabel(setting: ConfigSetting<unknown>): string {
  return setting.fallbackScope === null ? "Use Default" : "Use Global"
}

/** What unsetting the local value gives, for the tooltip of its button: "Off, the default" or "On, from your global Git config" */
export function describeFallback(setting: ConfigSetting<unknown>, valueText: string): string {
  if (setting.fallbackScope === null) {
    return `${valueText}, the default`
  }
  const source = scopeText(setting.fallbackScope)
  return `${valueText}, ${source.charAt(0).toLowerCase()}${source.slice(1)}`
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
  const leaf = first.branch.split("/").at(-1) ?? first.branch
  const subject = clashes.length === 1 ? `${leaf} is stacked by its prefix` : `${leaf} and ${clashes.length - 1} more are stacked by their prefixes`
  return `${subject}: turned off, ${first.branch} is a flat branch git cannot keep beside ${first.beside}. Unstack ${clashes.length === 1 ? leaf : "them"} first`
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
  }
}
