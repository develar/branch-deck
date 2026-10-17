import { useEffect, useRef, useState } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { RepositoryConfig } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

export type RepositoryConfigState = { status: "loading" } | { status: "loaded"; config: RepositoryConfig } | { status: "error"; message: string }

export interface RepositoryConfigRead {
  state: RepositoryConfigState
  /** Reads again, after a failed read */
  retry: () => void
  /** The settings a write answered, read again by the backend */
  setConfig: (config: RepositoryConfig) => void
}

/**
 * The `branchdeck.*` settings of the repository at `path` (`get_repository_config`), read when the page shows, again
 * when the window gets the focus (the user may have run `git config` or `bd` meanwhile) and on Retry. A read again keeps
 * the settings shown until it answers; an answer for another path, or one a later read or write overtook, is dropped.
 */
export function useRepositoryConfig(path: string): RepositoryConfigRead {
  const [generation, setGeneration] = useState(0)
  const [read, setRead] = useState<{ path: string; state: RepositoryConfigState } | null>(null)
  // the last read or write: an older answer must not take its settings back
  const latest = useRef(0)
  useWindowFocus(() => setGeneration(it => it + 1))

  useEffect(() => {
    const id = ++latest.current
    const answer = (state: RepositoryConfigState) => {
      if (latest.current === id) {
        setRead({ path, state })
      }
    }
    commands.getRepositoryConfig({ repositoryPath: path }).then(
      result => answer(result.status === "ok" ? { status: "loaded", config: result.data } : { status: "error", message: result.error.message }),
      (error: unknown) => answer({ status: "error", message: getErrorDetails(error) }),
    )
  }, [path, generation])

  return {
    state: read !== null && read.path === path ? read.state : { status: "loading" },
    retry: () => setGeneration(it => it + 1),
    setConfig: config => {
      latest.current++
      setRead({ path, state: { status: "loaded", config } })
    },
  }
}
