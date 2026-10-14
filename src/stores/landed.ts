import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { LandedBranch, LandedCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { mergeLandedReport } from "@/lib/landed"
import { useSelectedProject } from "@/stores/repository"

export interface LandedState {
  /** The repository of the last check */
  repositoryPath: string | null
  /** Local commits the fetched baseline already has: the next update drops them */
  landing: readonly LandedBranch[]
  /** Commits that landed with an update and that the user has not dismissed */
  landed: readonly LandedBranch[]
}

const empty: readonly LandedBranch[] = []

export const landedStore = createStore<LandedState>(() => ({
  repositoryPath: null,
  landing: empty,
  landed: empty,
}))

/** The landing and the report of a sync (`baselineStatus`) */
export function setLandedStatus(repositoryPath: string, landing: readonly LandedBranch[], landed: readonly LandedBranch[]): void {
  landedStore.setState({ repositoryPath, landing, landed })
}

/** An applied update: its commits are in the report at once, before the sync that follows; nothing is incoming */
export function addLandedUpdate(repositoryPath: string, found: readonly LandedBranch[]): void {
  landedStore.setState((state) => {
    const report = state.repositoryPath === repositoryPath ? state.landed : empty
    return { repositoryPath, landing: empty, landed: found.length === 0 ? report : mergeLandedReport(report, found) }
  })
}

/** The user has seen the report: cleared for the app, other windows and `bd` */
export async function dismissLanded(repositoryPath: string): Promise<void> {
  landedStore.setState(state => state.repositoryPath === repositoryPath ? { landed: empty } : state)
  const result = await invokeCommand(() => commands.dismissLanded(repositoryPath))
  if (result.status !== "ok") {
    notifyError("Failed to Dismiss the Landed Commits", result.error)
  }
}

function useOwnState<T>(select: (state: LandedState) => T, fallback: T): T {
  const repositoryPath = useSelectedProject()?.path
  return useStore(landedStore, state => state.repositoryPath === repositoryPath ? select(state) : fallback)
}

/** The undismissed report of the selected repository */
export function useLandedReport(): readonly LandedBranch[] {
  return useOwnState(state => state.landed, empty)
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
