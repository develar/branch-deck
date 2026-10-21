import { useState } from "react"
import type { RepositoryConfig, RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

export interface ConfigWrite {
  /** A write runs: its controls are disabled */
  writing: boolean
  /** Writes `change` to `scope`; a failure is a toast titled `errorTitle` */
  write: (change: RepositoryConfigChange, scope: WriteScope, errorTitle: string) => void
}

/**
 * A write of one `branchdeck.*` key of the repository at `path` (`set_repository_config`), for a setting of Settings that
 * one value of the global config gives every repository (Diffs & Merge, Agents): the answer is the settings read again,
 * handed to `setConfig` of `useRepositoryConfig`
 */
export function useConfigWrite(path: string, setConfig: (config: RepositoryConfig) => void): ConfigWrite {
  const [writing, setWriting] = useState(false)
  return {
    writing,
    write: (change, scope, errorTitle) => {
      setWriting(true)
      commands
        .setRepositoryConfig({ repositoryPath: path, change, scope })
        .then(
          result => (result.status === "error" ? notifyError(errorTitle, result.error) : setConfig(result.data)),
          (error: unknown) => notifyError(errorTitle, error),
        )
        .finally(() => setWriting(false))
    },
  }
}
