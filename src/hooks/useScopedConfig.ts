import { useConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigRead } from "@/hooks/useRepositoryConfig"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { RepositoryConfigChange } from "@/lib/bindings"
import type { WriteAnswer } from "@/lib/field-writes"
import { globalWriteErrorTitle, writeErrorTitle } from "@/lib/repository-config"
import type { RepositoryConfigKey } from "@/lib/repository-config"
import type { SettingNotSaved } from "@/lib/settings-close"
import type { SettingsPageId } from "@/lib/settings-pages"

export interface ScopedConfig {
  readonly read: RepositoryConfigRead
  /** A write runs: the controls of the page are disabled */
  readonly writing: boolean
  /** Writes `change` where the tab shown keeps it: the global config for every repository, else `.git/config`; what the backend answered */
  readonly write: (change: RepositoryConfigChange) => Promise<WriteAnswer>
  /** What the main window says of a write of `key` the close of the window could not finish: its title and its field on `page` */
  readonly notSaved: (key: RepositoryConfigKey, page: SettingsPageId) => SettingNotSaved
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
  const errorTitle = (key: RepositoryConfigKey) => (scope === "all" ? globalWriteErrorTitle(key) : writeErrorTitle(key, path))
  return {
    read,
    writing,
    write: change => write(change, scope === "all" ? "global" : "local", errorTitle(change.key)),
    notSaved: (key, page) => ({ title: errorTitle(key), target: { page, scope, focus: key } }),
  }
}
