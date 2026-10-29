import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { LandedBranch, LandedCommit } from "@/lib/bindings"
import { mergeLandedReport } from "@/lib/landed"
import { recordLanded } from "@/stores/activity"
import { useSelectedProjectPath } from "@/stores/repository"
import { updateRepositoryState, useRepositoryStateField } from "@/stores/settings/repositoryState"

/** The island stays at least this long after the user saw it: the sync that follows an update keeps it */
export const ISLAND_MIN_SEEN_MS = 10_000

export interface LandedState {
  /** The repository of the last check */
  repositoryPath: string | null
  /** Local commits the fetched baseline already has: the next update drops them */
  landing: readonly LandedBranch[]
  /** Commits that landed with an update and that the user has not dismissed */
  landed: readonly LandedBranch[]
  /** The upstream of the baseline they landed in ("origin/master"); null before the first check */
  upstream: string | null
  /**
   * When the user saw the island (it showed while the window had focus), ms, with the newest `landedAt` it showed then;
   * null until they have. A sync hides what they saw (`hideAfterSync`).
   */
  seenAt: { readonly time: number; readonly landedAt: number } | null
}

const empty: readonly LandedBranch[] = []

export const landedStore = createStore<LandedState>(() => ({
  repositoryPath: null,
  landing: empty,
  landed: empty,
  upstream: null,
  seenAt: null,
}))

/**
 * The landing and the report of a sync (`baselineStatus`); a report of an update made elsewhere goes to Activity, and the
 * toast of a merge it lands says so in place (`src/lib/landing-toast.ts`)
 */
export function setLandedStatus(repositoryPath: string, landing: readonly LandedBranch[], landed: readonly LandedBranch[], upstream: string): void {
  landedStore.setState(state => ({ repositoryPath, landing, landed, upstream, seenAt: state.repositoryPath === repositoryPath ? state.seenAt : null }))
  recordLanded(repositoryPath, landed, upstream)
}

/** An applied update: its commits are in the report at once, before the sync that follows; nothing is incoming */
export function addLandedUpdate(repositoryPath: string, found: readonly LandedBranch[], upstream: string): void {
  landedStore.setState(state => {
    const own = state.repositoryPath === repositoryPath
    const report = own ? state.landed : empty
    return { repositoryPath, landing: empty, landed: found.length === 0 ? report : mergeLandedReport(report, found), upstream, seenAt: own ? state.seenAt : null }
  })
}

/**
 * The user dismissed the report of `repositoryPath`: this window shows none of it at once (`dismissLanded` of
 * `src/lib/landed-run.ts` clears it for the other windows and `bd`)
 */
export function clearLandedReport(repositoryPath: string): void {
  landedStore.setState(state => (state.repositoryPath === repositoryPath ? { landed: empty, seenAt: null } : state))
}

function isWindowFocused(): boolean {
  return typeof document === "undefined" || document.hasFocus()
}

/**
 * The island shows what landed up to `landedAt`: if the window has focus, the user sees it from now (a newer `landedAt`
 * starts again). Called when the island shows and when the window gains focus.
 */
export function markLandedSeen(repositoryPath: string, landedAt: number): void {
  const { repositoryPath: own, seenAt } = landedStore.getState()
  if (own === repositoryPath && seenAt?.landedAt !== landedAt && isWindowFocused()) {
    landedStore.setState({ seenAt: { time: Date.now(), landedAt } })
  }
}

/**
 * After a successful sync of the repository: the island hides what the user saw, if they saw it at least
 * `ISLAND_MIN_SEEN_MS` ago and the window has focus. Remembered for the repository, so it stays hidden after a restart;
 * what lands later shows again. The report, the chips and the "New" marks stay until Dismiss.
 */
export function hideAfterSync(repositoryPath: string): void {
  const { repositoryPath: own, seenAt } = landedStore.getState()
  if (own !== repositoryPath || seenAt === null || Date.now() - seenAt.time < ISLAND_MIN_SEEN_MS || !isWindowFocused()) {
    return
  }
  landedStore.setState({ seenAt: null })
  updateRepositoryState(repositoryPath, state => ({ ...state, landedHiddenUntil: seenAt.landedAt }))
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
