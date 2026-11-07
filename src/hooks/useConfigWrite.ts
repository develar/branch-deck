import { useState } from "react"
import type { RepositoryConfig, RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

export interface ConfigWrite {
  /** A write runs: its controls are disabled */
  writing: boolean
  /** Writes `change` to `scope`; a failure is a toast titled `errorTitle`. True once the backend answered the settings */
  write: (change: RepositoryConfigChange, scope: WriteScope, errorTitle: string) => Promise<boolean>
}

/**
 * A write of one `branchdeck.*` key of the repository at `path` (`set_repository_config`), to `scope` (the local config
 * when none): the answer is the settings read again, handed to `showWritten`; a failure is a toast titled `errorTitle`.
 * True once the backend answered the settings. The one call of the command, for Settings → Repository and the settings
 * that one value of the global config gives every repository (`useConfigWrite`)
 */
export function writeRepositoryConfig(
  path: string,
  change: RepositoryConfigChange,
  scope: WriteScope | undefined,
  errorTitle: string,
  showWritten: (answer: RepositoryConfig) => void,
): Promise<boolean> {
  return commands.setRepositoryConfig({ repositoryPath: path, change, ...(scope !== undefined && { scope }) }).then(
    result => {
      if (result.status === "error") {
        notifyError(errorTitle, result.error)
        return false
      }
      showWritten(result.data)
      return true
    },
    (error: unknown) => {
      notifyError(errorTitle, error)
      return false
    },
  )
}

/**
 * A write of one `branchdeck.*` key of the repository at `path` (`writeRepositoryConfig`), for a setting of Settings that
 * one value of the global config gives every repository (Diffs & Merge, Agents): the answer is the settings read again,
 * handed to `showWritten` of `useRepositoryConfig`
 */
export function useConfigWrite(path: string, showWritten: (answer: RepositoryConfig) => void): ConfigWrite {
  const [writing, setWriting] = useState(false)
  return {
    writing,
    write: (change, scope, errorTitle) => {
      setWriting(true)
      return writeRepositoryConfig(path, change, scope, errorTitle, showWritten).finally(() => setWriting(false))
    },
  }
}

/**
 * Add Companion… of Settings → Landing: the folder dialog of the app picks the companion of the repository at `path`
 * (named by its directory), written to both sides (`add_companion`); the answer is the settings read again, handed to
 * `showWritten`. False when the user canceled or the write failed, a toast then
 */
export function addCompanion(path: string, showWritten: (answer: RepositoryConfig) => void): Promise<boolean> {
  return answered(commands.addCompanion({ repositoryPath: path }), "Cannot add the companion", showWritten)
}

/** Removes the companion `name` of the repository at `path` (`set_companion`), both sides; as `addCompanion` */
export function removeCompanion(path: string, name: string, showWritten: (answer: RepositoryConfig) => void): Promise<boolean> {
  return answered(commands.setCompanion({ repositoryPath: path, name, path: null }), `Cannot remove the companion ${name}`, showWritten)
}

function answered(
  call: Promise<{ status: "ok"; data: RepositoryConfig | null } | { status: "error"; error: unknown }>,
  errorTitle: string,
  showWritten: (answer: RepositoryConfig) => void,
): Promise<boolean> {
  return call.then(
    result => {
      if (result.status === "error") {
        notifyError(errorTitle, result.error)
        return false
      }
      if (result.data === null) {
        return false
      }
      showWritten(result.data)
      return true
    },
    (error: unknown) => {
      notifyError(errorTitle, error)
      return false
    },
  )
}
