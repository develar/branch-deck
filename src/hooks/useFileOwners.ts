import { useEffect, useRef, useState } from "react"
import { commands } from "@/lib/bindings"

const NO_OWNERS: ReadonlyMap<string, string> = new Map()

/**
 * The owners of paths by the `OWNERSHIP` files at `revision` (the baseline), read once per path for a review window;
 * another repository or revision reads them again, none while `revision` is unknown
 */
export function useFileOwners(repositoryPath: string, revision: string | null, paths: readonly string[]): ReadonlyMap<string, string> {
  const key = revision === null ? null : `${repositoryPath}\n${revision}`
  const [read, setRead] = useState<{ key: string | null; owners: ReadonlyMap<string, string> }>({ key, owners: NO_OWNERS })
  const asked = useRef<{ key: string | null; paths: Set<string> }>({ key, paths: new Set() })
  useEffect(() => {
    if (key === null || revision === null) {
      return
    }
    if (asked.current.key !== key) {
      asked.current = { key, paths: new Set() }
    }
    const known = asked.current.paths
    const missing = paths.filter(path => !known.has(path))
    if (missing.length === 0) {
      return
    }
    for (const path of missing) {
      known.add(path)
    }
    void commands.getFileOwners({ repositoryPath, baselineBranch: revision, paths: missing }).then(result => {
      if (result.status === "ok" && result.data.length > 0) {
        setRead(previous => ({ key, owners: new Map([...(previous.key === key ? previous.owners : []), ...result.data.map(owner => [owner.path, owner.owner] as const)]) }))
      }
    })
  }, [repositoryPath, revision, key, paths])
  return read.key === key ? read.owners : NO_OWNERS
}
