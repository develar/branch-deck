import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import type { DiskOverview, DiskUsageEvent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

export type DiskUsageState =
  | { status: "loading" }
  | { status: "loaded"; overview: DiskOverview; measured: number; measuring: number; done: boolean; ms: number | null }
  | { status: "error"; message: string }

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
    const events = new Channel<DiskUsageEvent>()
    events.onmessage = event => {
      if (current) {
        setRead(previous => {
          const state = previous?.key === key && previous.seq === request.seq ? previous.state : null
          const next = receiveDiskEvent(state, event)
          return next === null ? previous : { key, state: next, seq: request.seq }
        })
      }
    }
    void commands.diskUsage({ repositoryPath, fresh: request.fresh }, events).then(
      result => {
        if (!current || (result.status === "error" && result.error.code === "canceled")) {
          return
        }
        if (result.status === "error") {
          setRead({ key, state: { status: "error", message: result.error.message }, seq: request.seq })
        }
      },
      (error: unknown) => {
        if (current) {
          setRead({ key, state: { status: "error", message: getErrorDetails(error) }, seq: request.seq })
        }
      },
    )
    return () => {
      current = false
    }
  }, [key, repositoryPath, request])

  const shown = read !== null && read.key === key ? read : null
  return {
    state: shown?.state ?? { status: "loading" },
    refreshing: shown !== null && (shown.seq !== request.seq || (shown.state.status === "loaded" && !shown.state.done)),
    reload: fresh => setRequest(it => ({ seq: it.seq + 1, fresh })),
  }
}

function receiveDiskEvent(previous: DiskUsageState | null, event: DiskUsageEvent): DiskUsageState | null {
  switch (event.type) {
    case "cached":
      return { status: "loaded", overview: event.overview, measured: 0, measuring: event.measuring, done: false, ms: null }
    case "slot":
      if (previous?.status !== "loaded") {
        return previous
      }
      return {
        ...previous,
        overview: {
          machines: previous.overview.machines.map(machine =>
            machine.alias === event.machine
              ? {
                  ...machine,
                  pools: machine.pools.map(pool =>
                    pool.path === event.pool ? { ...pool, slots: pool.slots.map(slot => (slot.path === event.slot.path ? event.slot : slot)) } : pool,
                  ),
                }
              : machine,
          ),
        },
        measured: event.measured,
        measuring: event.measuring,
      }
    case "planning":
      return previous?.status === "loaded" ? { ...previous, measured: 0, measuring: event.measuring } : previous
    case "machine":
      if (previous?.status !== "loaded") {
        return previous
      }
      return {
        ...previous,
        overview: {
          machines: previous.overview.machines.some(machine => machine.alias === event.machine.alias)
            ? previous.overview.machines.map(machine => (machine.alias === event.machine.alias ? event.machine : machine))
            : [...previous.overview.machines, event.machine],
        },
      }
    case "done":
      return previous?.status === "loaded" ? { ...previous, done: true, ms: event.ms } : previous
  }
}
