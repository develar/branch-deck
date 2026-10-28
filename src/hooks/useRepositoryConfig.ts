import { useEffect, useState } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { RepositoryConfig } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { newer } from "@/lib/repository-config"

export type RepositoryConfigState = { status: "loading" } | { status: "loaded"; config: RepositoryConfig } | { status: "error"; message: string }

export interface RepositoryConfigRead {
  state: RepositoryConfigState
  /** Reads again, after a failed read */
  retry: () => void
  /** Shows the settings a write answered, read again by the backend, unless the page shows newer ones (`newer`) */
  showWritten: (answer: RepositoryConfig) => void
}

/** What the page shows, and for which repository */
interface ShownRead {
  path: string
  state: RepositoryConfigState
}

/** `shown` after an answer of the settings at `path`: an older answer than the settings shown never takes them back */
function withAnswer(shown: ShownRead | null, path: string, answer: RepositoryConfig): ShownRead {
  if (shown !== null && shown.path === path && shown.state.status === "loaded" && !newer(shown.state.config, answer)) {
    return shown
  }
  return { path, state: { status: "loaded", config: answer } }
}

/**
 * The `branchdeck.*` settings of the repository at `path` (`get_repository_config`), read when the page shows, again
 * when the window gets the focus (the user may have run `git config` or `bd` meanwhile) and on Retry. A read again keeps
 * the settings shown until it answers; the answer of a read that a later read or a switch of the repository overtook is
 * dropped, and a read or a write that answers last with fewer writes than the settings shown (`seq`) never takes a
 * newer write back.
 */
export function useRepositoryConfig(path: string): RepositoryConfigRead {
  const [generation, setGeneration] = useState(0)
  const [read, setRead] = useState<ShownRead | null>(null)
  useWindowFocus(() => setGeneration(it => it + 1))

  useEffect(() => {
    let current = true
    const failed = (message: string) => {
      if (current) {
        setRead({ path, state: { status: "error", message } })
      }
    }
    commands.getRepositoryConfig({ repositoryPath: path }).then(
      result => {
        if (result.status === "error") {
          failed(result.error.message)
        } else if (current) {
          setRead(shown => withAnswer(shown, path, result.data))
        }
      },
      (error: unknown) => failed(getErrorDetails(error)),
    )
    return () => {
      current = false
    }
  }, [path, generation])

  return {
    state: read !== null && read.path === path ? read.state : { status: "loading" },
    retry: () => setGeneration(it => it + 1),
    // a write for a repository the page no longer shows changes nothing here
    showWritten: answer => setRead(shown => (shown !== null && shown.path === path ? withAnswer(shown, path, answer) : shown)),
  }
}
