import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { LandedBranch, LandedCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { mergeLandedReport } from "@/lib/landed"
import { recordLanded } from "@/stores/activity"
import { useSelectedProjectPath } from "@/stores/repository"
import { useRepositoryStateField } from "@/stores/settings/repositoryState"

export interface LandedState {
  /** The repository of the last check */
  repositoryPath: string | null
  /** Local commits the fetched baseline already has: the next update drops them */
  landing: readonly LandedBranch[]
  /** Commits that landed with an update and that the user has not dismissed */
  landed: readonly LandedBranch[]
  /** The upstream of the baseline they landed in ("origin/master"); null before the first check */
  upstream: string | null
}

const empty: readonly LandedBranch[] = []

export const landedStore = createStore<LandedState>(() => ({
  repositoryPath: null,
  landing: empty,
  landed: empty,
  upstream: null,
}))

/**
 * The landing and the report of a sync (`baselineStatus`); a report of an update made elsewhere goes to Activity, and the
 * toast of a merge it lands says so in place (`src/lib/landing-toast.ts`)
 */
export function setLandedStatus(repositoryPath: string, landing: readonly LandedBranch[], landed: readonly LandedBranch[], upstream: string): void {
  landedStore.setState({ repositoryPath, landing, landed, upstream })
  recordLanded(repositoryPath, landed, upstream)
}

/** An applied update: its commits are in the report at once, before the sync that follows; nothing is incoming */
export function addLandedUpdate(repositoryPath: string, found: readonly LandedBranch[], upstream: string): void {
  landedStore.setState(state => {
    const own = state.repositoryPath === repositoryPath
    const report = own ? state.landed : empty
    return { repositoryPath, landing: empty, landed: found.length === 0 ? report : mergeLandedReport(report, found), upstream }
  })
}

/**
 * The user dismissed the report of `repositoryPath`: this window shows none of it at once (`dismissLanded` of
 * `src/lib/landed-run.ts` clears it for the other windows and `bd`)
 */
export function clearLandedReport(repositoryPath: string): void {
  landedStore.setState(state => (state.repositoryPath === repositoryPath ? { landed: empty } : state))
}

function isWindowFocused(): boolean {
  return typeof document === "undefined" || document.hasFocus()
}

/**
 * The island shows what landed up to `landedAt` on the view of the epoch `viewEpoch`: if the window has focus, the
 * session hears that this window sees it (`landed_seen`), and its first successful sync at least 10 s later, with the
 * window focused, hides what was seen for the repository (`landedHiddenUntil`, `domain_policy::landed_island`). Called
 * when the island shows and when the window gains focus; the session keeps the time of the first sighting of the same
 * `landedAt`.
 */
export function markLandedSeen(repositoryPath: string, landedAt: number, viewEpoch: number | null): void {
  if (isWindowFocused()) {
    void commands.landedSeen({ repositoryPath, landedAt, epoch: viewEpoch }).then(result => {
      if (result.status === "error") {
        logError("Cannot tell the session that the window sees the Landed island", result.error)
      }
    })
  }
}

function useOwnState<T>(select: (state: LandedState) => T, fallback: T): T {
  const repositoryPath = useSelectedProjectPath()
  return useStore(landedStore, state => (state.repositoryPath === repositoryPath ? select(state) : fallback))
}

/** The undismissed report of the selected repository */
export function useLandedReport(): readonly LandedBranch[] {
  return useOwnState(state => state.landed, empty)
}

/** What the island shows: the branches of the undismissed report that landed after those a sync hid */
export function useLandedIsland(): readonly LandedBranch[] {
  const hiddenUntil = useRepositoryStateField(useSelectedProjectPath(), "landedHiddenUntil") ?? 0
  const report = useLandedReport()
  return useMemo(() => (hiddenUntil === 0 ? report : report.filter(branch => branch.landedAt > hiddenUntil)), [report, hiddenUntil])
}

/** What the fetched baseline already has, for the Update button */
export function useLanding(): readonly LandedBranch[] {
  return useOwnState(state => state.landing, empty)
}

export interface BranchLanded {
  branch: LandedBranch
  /** true: the baseline has the commits, the update is still to come; false: landed with an update */
  incoming: boolean
}

/** The landed commits of an active branch: incoming ones first, else the undismissed report */
export function useBranchLanded(name: string): BranchLanded | null {
  // each selector returns a branch of the store, so the store notifies only when it changes
  const incoming = useOwnState(state => state.landing.find(branch => branch.name === name), undefined)
  const landed = useOwnState(state => state.landed.find(branch => branch.name === name), undefined)
  if (incoming !== undefined) {
    return { branch: incoming, incoming: true }
  }
  return landed === undefined ? null : { branch: landed, incoming: false }
}

/** The incoming landed commit with this id, for the commit row */
export function useIncomingLandedCommit(id: string): LandedCommit | null {
  return useOwnState(state => findCommit(state.landing, id), null)
}

function findCommit(branches: readonly LandedBranch[], id: string): LandedCommit | null {
  for (const branch of branches) {
    const commit = branch.commits.find(it => it.id === id)
    if (commit !== undefined) {
      return commit
    }
  }
  return null
}
