import { useEffect, useRef, useState } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { RepositoryConfig } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import type { RepositoryConfigKey } from "@/lib/repository-config"
import { ConfigWrites } from "@/lib/repository-config"

export type RepositoryConfigState = { status: "loading" } | { status: "loaded"; config: RepositoryConfig } | { status: "error"; message: string }

export interface RepositoryConfigRead {
  state: RepositoryConfigState
  /** Reads again, after a failed read */
  retry: () => void
  /**
   * A write of `key` starts: the function it returns takes the settings the write answered, read again by the backend
   * (null for a failed write). Writes that overlap give only the setting each wrote, then the settings are read again
   * (`ConfigWrites`)
   */
  startWrite: (key: RepositoryConfigKey) => (answer: RepositoryConfig | null) => void
}

/**
 * The `branchdeck.*` settings of the repository at `path` (`get_repository_config`), read when the page shows, again
 * when the window gets the focus (the user may have run `git config` or `bd` meanwhile) and on Retry. A read again keeps
 * the settings shown until it answers; an answer for another path, or one a later read or write overtook, is dropped,
 * and an older write that answers last never takes back a newer one.
 */
export function useRepositoryConfig(path: string): RepositoryConfigRead {
  const [generation, setGeneration] = useState(0)
  const [read, setRead] = useState<{ path: string; state: RepositoryConfigState } | null>(null)
  // the last read or write: an older answer must not take its settings back
  const latest = useRef(0)
  const writes = useRef(new ConfigWrites())
  // the settings the page shows, which an answer of overlapping writes adds its setting to
  const shown = useRef<{ path: string; config: RepositoryConfig } | null>(null)
  useWindowFocus(() => setGeneration(it => it + 1))

  useEffect(() => {
    const id = ++latest.current
    const answer = (state: RepositoryConfigState) => {
      if (latest.current === id) {
        shown.current = state.status === "loaded" ? { path, config: state.config } : null
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
    startWrite: key => {
      // a read started before the write may lack it
      latest.current++
      writes.current.start()
      return answer => {
        latest.current++
        const { config, readAgain } = writes.current.finish(key, answer, shown.current?.path === path ? shown.current.config : null)
        if (config !== null) {
          shown.current = { path, config }
          setRead({ path, state: { status: "loaded", config } })
        }
        if (readAgain) {
          setGeneration(it => it + 1)
        }
      }
    },
  }
}
