import { useEffect, useState } from "react"
import type { DiskOverview } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

export type DiskUsageState = { status: "loading" } | { status: "loaded"; overview: DiskOverview } | { status: "error"; message: string }

export interface DiskUsageRead {
  state: DiskUsageState
  /** A read again is running while `state` shows the last answer */
  refreshing: boolean
  /** Reads again; `fresh` walks every slot again (Measure Again), else the bytes the pools keep */
  reload: (fresh: boolean) => void
}

/**
 * What the lease pools of this Mac and of the hosts hold (`disk_usage`), read when the dialog opens (`generation`) and on
 * `reload`. A read again keeps the answer shown until it answers; the answer of a read that a later one overtook is
 * dropped.
 */
export function useDiskUsage(repositoryPath: string, generation: number): DiskUsageRead {
  const [request, setRequest] = useState({ seq: 0, fresh: false })
  const [read, setRead] = useState<{ key: string; state: DiskUsageState; seq: number } | null>(null)
  const key = `${repositoryPath}\n${generation}`

  useEffect(() => {
    let current = true
    const answered = (state: DiskUsageState) => {
      if (current) {
        setRead({ key, state, seq: request.seq })
      }
    }
    commands.diskUsage({ repositoryPath, fresh: request.fresh }).then(
      result => answered(result.status === "error" ? { status: "error", message: result.error.message } : { status: "loaded", overview: result.data }),
      (error: unknown) => answered({ status: "error", message: getErrorDetails(error) }),
    )
    return () => {
      current = false
    }
  }, [key, repositoryPath, request])

  const shown = read !== null && read.key === key ? read : null
  return {
    state: shown?.state ?? { status: "loading" },
    refreshing: shown !== null && shown.seq !== request.seq,
    reload: fresh => setRequest(it => ({ seq: it.seq + 1, fresh })),
  }
}
