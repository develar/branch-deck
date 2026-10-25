import { useState } from "react"
import type { RepositoryConfig, RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { RepositoryConfigKey } from "@/lib/repository-config"

export interface ConfigWrite {
  /** A write runs: its controls are disabled */
  writing: boolean
  /** Writes `change` to `scope`; a failure is a toast titled `errorTitle`. True once the backend answered the settings */
  write: (change: RepositoryConfigChange, scope: WriteScope, errorTitle: string) => Promise<boolean>
}

/**
 * A write of one `branchdeck.*` key of the repository at `path` (`set_repository_config`), for a setting of Settings that
 * one value of the global config gives every repository (Diffs & Merge, Agents): the answer is the settings read again,
 * handed to `startWrite` of `useRepositoryConfig`
 */
export function useConfigWrite(path: string, startWrite: (key: RepositoryConfigKey) => (answer: RepositoryConfig | null) => void): ConfigWrite {
  const [writing, setWriting] = useState(false)
  return {
    writing,
    write: (change, scope, errorTitle) => {
      setWriting(true)
      const answered = startWrite(change.key)
      return commands
        .setRepositoryConfig({ repositoryPath: path, change, scope })
        .then(
          result => {
            if (result.status === "error") {
              notifyError(errorTitle, result.error)
            }
            answered(result.status === "ok" ? result.data : null)
            return result.status === "ok"
          },
          (error: unknown) => {
            notifyError(errorTitle, error)
            answered(null)
            return false
          },
        )
        .finally(() => setWriting(false))
    },
  }
}
