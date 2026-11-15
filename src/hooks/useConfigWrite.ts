import { useState } from "react"
import type { RepositoryConfig, RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { nextWriteOrder } from "@/lib/field-writes"
import type { WriteAnswer } from "@/lib/field-writes"
import type { MenuItem } from "@/lib/menu"
import { repositoryName } from "@/lib/repository-config"

export interface ConfigWrite {
  /** A write runs: its controls are disabled */
  writing: boolean
  /** Writes `change` to `scope`; a failure is a toast titled `errorTitle`. What the backend answered */
  write: (change: RepositoryConfigChange, scope: WriteScope, errorTitle: string) => Promise<WriteAnswer>
}

/**
 * A write of one `branchdeck.*` key of the repository at `path` (`set_repository_config`), to `scope` (the local config
 * when none), numbered among the writes of the page (`nextWriteOrder`): the answer is the settings read again, handed to
 * `showWritten`; none for a write a newer one of the key superseded (`superseded`, no error); a failure is a toast titled
 * `errorTitle`. The one call of the command, for Settings → Repository and the settings that one value of the global
 * config gives every repository (`useConfigWrite`)
 */
export function writeRepositoryConfig(
  path: string,
  change: RepositoryConfigChange,
  scope: WriteScope | undefined,
  errorTitle: string,
  showWritten: (answer: RepositoryConfig) => void,
): Promise<WriteAnswer> {
  return commands.setRepositoryConfig({ repositoryPath: path, change, ...(scope !== undefined && { scope }), order: nextWriteOrder() }).then(
    result => {
      if (result.status === "error") {
        notifyError(errorTitle, result.error)
        return { failed: result.error }
      }
      if (result.data === null) {
        return "superseded"
      }
      showWritten(result.data)
      return "taken"
    },
    (error: unknown) => {
      notifyError(errorTitle, error)
      return { failed: error }
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

/**
 * Unlink of a companion: takes back its side of a half link (`unlink_primary`: its `branchdeck.primary`, refused while
 * the primary lists it); as `addCompanion`
 */
function unlinkPrimary(path: string, showWritten: (answer: RepositoryConfig) => void): Promise<boolean> {
  return answered(commands.unlinkPrimary({ repositoryPath: path }), "Cannot unlink the primary", showWritten)
}

/**
 * The actions of the link of the repository at `path` to its primary, the one builder of every surface that offers them
 * (Settings → Landing of a companion): Unlink while the link is half or broken (`RepositoryConfig.primaryProblem`), and
 * disabled with why while the primary lists it, which removes it there; none for a repository of no primary. `run` runs
 * the write, which `showWritten` gets the settings of
 */
export function primaryLinkItems(
  path: string,
  config: RepositoryConfig,
  run: (write: () => Promise<boolean>) => void,
  showWritten: (answer: RepositoryConfig) => void,
): MenuItem[] {
  if (config.primary === null) {
    return []
  }
  const primary = repositoryName(config.primary)
  return [
    {
      id: "unlink-primary",
      label: "Unlink",
      disabled: config.primaryProblem === null,
      description:
        config.primaryProblem === null
          ? `${primary} lists this repository as a companion: remove it there`
          : `Takes back this repository's side of the link: it names ${primary} as its primary no more`,
      onSelect: () => run(() => unlinkPrimary(path, showWritten)),
    },
  ]
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
