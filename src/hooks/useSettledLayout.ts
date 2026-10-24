import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { getBranchNextStepNow, useNextStepStores } from "@/hooks/useBranchNextStep"
import type { ColumnKey } from "@/lib/branch-columns"
import { useBranchSync } from "@/stores/branch-sync"
import { isPatronusPending } from "@/stores/patronus"
import { isPullRequestsLoading } from "@/stores/pull-requests"
import { useSelectedProjectPath } from "@/stores/repository"
import { reviewRequestsStore } from "@/stores/review-requests"
import type { RepositoryStateEntry } from "@/stores/settings/repositoryState"
import { getRepositoryState, updateRepositoryState, useRepositoryStateField } from "@/stores/settings/repositoryState"
import { getShownBranchTableOrder } from "@/stores/stack-collapse"

/*
 * The facts of the remote come after the first rows: Patronus, the pull requests, the readiness of each row, the review
 * requests ([Next Step](../../spec/branch/next-step.spec.md#facts-without-an-answer)). Until they are in, the branch view
 * reserves the layout it had once they were in the last time (`repositoryState.settledLayout`): the widths of the
 * branch columns and the height of Your Turn, so an answer that fills them as before moves nothing under the pointer.
 * Once they are in, the view follows its content, and its layout is kept for the next start and a switch back.
 */

export type SettledLayout = NonNullable<RepositoryStateEntry["settledLayout"]>

/** The repositories whose facts were all in once in this window: their layout follows the content from then on */
const settledStore = createStore<Readonly<Record<string, true>>>(() => ({}))

/** The facts of the remote the rows wait for at a start or a switch are in: Patronus, GitHub, the step of each shown row */
function isRemoteKnownNow(): boolean {
  return !isPatronusPending() && !isPullRequestsLoading() && getShownBranchTableOrder().every(name => getBranchNextStepNow(name)?.kind !== "pending")
}

/**
 * Whether the facts the branch view of the selected repository waits for are in: its first sync in this window ended,
 * Patronus and GitHub answered, the review requests too, and no shown row waits for an answer (`pending`); once true for
 * a repository, it stays true in this window (a new branch that waits for its readiness moves its own column only)
 */
export function useLayoutSettled(): boolean {
  const path = useSelectedProjectPath() ?? ""
  const latched = useStore(settledStore, state => state[path] === true)
  const synced = useBranchSync(state => path !== "" && state.syncedRepositoryPath === path)
  const reviewRequests = useStore(reviewRequestsStore, state => (state.repositories[path]?.availability ?? null) !== null)
  const known = useNextStepStores(() => latched || (synced && reviewRequests && isRemoteKnownNow()))
  const settled = latched || (path !== "" && synced && reviewRequests && known)
  useEffect(() => {
    if (settled && !latched && path !== "") {
      settledStore.setState({ [path]: true })
    }
  }, [settled, latched, path])
  return settled
}

/** The layout the selected repository had once its facts were in the last time; undefined before its first */
export function useSettledLayout(): SettledLayout | undefined {
  return useRepositoryStateField(useSelectedProjectPath(), "settledLayout")
}

/** Keeps what the view measures once settled: the widths of the tracks (`tracks`), or the height of Your Turn */
export function saveSettledLayout(path: string, patch: Partial<SettledLayout>): void {
  const before = getRepositoryState(path)?.settledLayout ?? { tracks: {}, yourTurn: 0 }
  const next: SettledLayout = { ...before, ...patch }
  if (JSON.stringify(next) !== JSON.stringify(before) || getRepositoryState(path)?.settledLayout === undefined) {
    updateRepositoryState(path, state => ({ ...state, settledLayout: next }))
  }
}

/** The widths of `tracks` as the layout keeps them: whole pixels, the empty tracks left out */
export function settledTracks(tracks: Partial<Record<ColumnKey, number>>): Record<string, number> {
  return Object.fromEntries(Object.entries(tracks).flatMap(([key, width]) => (width === undefined || width <= 0 ? [] : [[key, Math.ceil(width)]])))
}

/** Forgets which repositories settled (tests) */
export function resetSettledLayout(): void {
  settledStore.setState({}, true)
}
