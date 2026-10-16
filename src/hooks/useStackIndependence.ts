import { useCallback, useEffect, useState } from "react"
import type { StackIndependence } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { useBranchTip } from "@/stores/branch-sync"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"

export type StackIndependenceState = { status: "idle" } | { status: "loading" } | { status: "loaded"; independence: StackIndependence } | { status: "error"; message: string }

export interface StackIndependenceCheck {
  state: StackIndependenceState
  /** Checks again, e.g. after a move into the branch, before the sync shows the new tip */
  recheck: () => void
}

/**
 * Whether the stacked branch builds on the baseline without its parent (`get_stack_independence`), asked when `enabled`
 * (the Safe Push form of a stacked branch is open) and again when the local tip of the branch changes. A trial build
 * of the branch alone, objects only: no lock, nothing the sync reads.
 */
export function useStackIndependence(branchName: string, enabled: boolean): StackIndependenceCheck {
  const [generation, setGeneration] = useState(0)
  const localTip = useBranchTip(branchName)
  // an answer counts only for the question it answers: another tip or a recheck is loading again
  const key = enabled ? `${branchName}\n${localTip ?? ""}\n${generation}` : null
  const [answer, setAnswer] = useState<{ key: string; state: StackIndependenceState } | null>(null)

  useEffect(() => {
    if (key === null) {
      return
    }
    let current = true
    const answerWith = (state: StackIndependenceState) => {
      if (current) {
        setAnswer({ key, state })
      }
    }
    commands.getStackIndependence({ repositoryPath: getSelectedProject()?.path ?? "", branchPrefix: getEffectiveBranchPrefix(), branchName }).then(
      result => answerWith(result.status === "ok" ? { status: "loaded", independence: result.data } : { status: "error", message: result.error.message }),
      (error: unknown) => answerWith({ status: "error", message: error instanceof Error ? error.message : String(error) }),
    )
    return () => {
      current = false
    }
  }, [key, branchName])

  const recheck = useCallback(() => setGeneration(it => it + 1), [])
  const state: StackIndependenceState = key === null ? { status: "idle" } : answer?.key === key ? answer.state : { status: "loading" }
  return { state, recheck }
}
