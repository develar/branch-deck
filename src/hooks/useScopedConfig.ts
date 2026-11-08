import { useConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigRead } from "@/hooks/useRepositoryConfig"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { RepositoryConfigChange } from "@/lib/bindings"
import { globalWriteErrorTitle, writeErrorTitle } from "@/lib/repository-config"

export interface ScopedConfig {
  readonly read: RepositoryConfigRead
  /** A write runs: the controls of the page are disabled */
  readonly writing: boolean
  /** Writes `change` where the tab shown keeps it: the global config for every repository, else `.git/config`; true once answered */
  readonly write: (change: RepositoryConfigChange) => Promise<boolean>
}

/**
 * The `branchdeck.*` settings a page of the git config shows (`useRepositoryConfig` of the selected repository, the
 * global config alone without one) and their write to the scope of the tab shown (`useSettingsScope`), the failure a
 * toast in the words of the change
 */
export function useScopedConfig(): ScopedConfig {
  const { scope, path } = useSettingsScope()
  const read = useRepositoryConfig(path)
  const { writing, write } = useConfigWrite(path, read.showWritten)
  return {
    read,
    writing,
    write: change => (scope === "all" ? write(change, "global", globalWriteErrorTitle(change.key)) : write(change, "local", writeErrorTitle(change, path))),
  }
}
