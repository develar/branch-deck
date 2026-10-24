import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { createStore } from "zustand/vanilla"
import type { Commit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure } from "@/lib/errors"
import { whenSessionViewAt } from "@/model/store"
import type { RemoteStatus } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"

/**
 * A branch an unapply kept (`{prefix}/unapplied/<name>`), as the last sync listed it: its commits are not in HEAD, it
 * lands nothing until it is reapplied ([Unapply Branch](../../spec/branch/unapply.spec.md)). Not a row of the branch
 * table: no robot, no landing, no move of the user. Its commits load on the first expansion
 */
export interface UnappliedBranch {
  /** The full name, `{prefix}/unapplied/<name>` */
  readonly name: string
  /** `<name>`, the branch it was (a `-N` suffix when that name was taken) */
  readonly branch: string
  readonly tip: string
  /** The subject of its tip, without the grouping prefix */
  readonly subject: string
  readonly checkedOut: boolean
  /** Its remote status when it was pushed; null when it is not on the remote */
  readonly remoteStatus: RemoteStatus | null
  readonly commits: readonly Commit[] | undefined
  readonly isLoadingCommits: boolean
  readonly loadError: string | undefined
}

export interface UnappliedBranchesState {
  /** Newest first, as the sync listed them */
  order: readonly string[]
  branches: Readonly<Record<string, UnappliedBranch>>
}

const EMPTY: UnappliedBranchesState = { order: [], branches: {} }

export const unappliedBranchesStore = createStore<UnappliedBranchesState>(() => EMPTY)

export function useUnappliedBranchOrder(): readonly string[] {
  return useStore(
    unappliedBranchesStore,
    useShallow(state => state.order),
  )
}

export function useUnappliedBranch(name: string): UnappliedBranch | undefined {
  return useStore(unappliedBranchesStore, state => state.branches[name])
}

export function getUnappliedBranch(name: string): UnappliedBranch | undefined {
  return unappliedBranchesStore.getState().branches[name]
}

/** What a sync reports of an unapplied branch */
export type ListedUnappliedBranch = Pick<UnappliedBranch, "name" | "branch" | "tip" | "subject" | "checkedOut" | "remoteStatus">

/** The unapplied branches of a sync (`UnappliedBranchesFound`): the commits loaded for a tip that stayed are kept */
export function setUnappliedBranches(listed: readonly ListedUnappliedBranch[]): void {
  const previous = unappliedBranchesStore.getState().branches
  const branches: Record<string, UnappliedBranch> = {}
  for (const info of listed) {
    const kept = previous[info.name]
    const sameTip = kept?.tip === info.tip
    branches[info.name] = {
      ...info,
      commits: sameTip ? kept.commits : undefined,
      isLoadingCommits: sameTip ? kept.isLoadingCommits : false,
      loadError: sameTip ? kept.loadError : undefined,
    }
  }
  unappliedBranchesStore.setState({ order: listed.map(info => info.name), branches })
}

/** The list of another repository is not this one's */
export function resetUnappliedBranches(): void {
  unappliedBranchesStore.setState(EMPTY)
}

function update(name: string, patch: (branch: UnappliedBranch) => Partial<UnappliedBranch>): void {
  unappliedBranchesStore.setState(state => {
    const branch = state.branches[name]
    return branch === undefined ? state : { branches: { ...state.branches, [name]: { ...branch, ...patch(branch) } } }
  })
}

/** Loads the commits of an unapplied branch once (a no-op when they are loaded or loading) */
export async function loadUnappliedCommits(name: string): Promise<void> {
  const branch = getUnappliedBranch(name)
  if (branch === undefined || branch.commits !== undefined || branch.isLoadingCommits) {
    return
  }
  update(name, () => ({ isLoadingCommits: true, loadError: undefined }))
  const result = await commands.getUnappliedBranchCommits(getSelectedProject()?.path ?? "", name)
  update(name, () => (result.status === "ok" ? { commits: result.data, isLoadingCommits: false } : { loadError: result.error.message, isLoadingCommits: false }))
}

/**
 * Deletes an unapplied branch and resolves once the table has the patch of the session that removed it (no sync
 * follows); throws on a failure
 */
export async function deleteUnappliedBranch(name: string): Promise<void> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const result = await commands.deleteUnappliedBranch({ repositoryPath, branchName: name })
  if (result.status !== "ok") {
    throw commandFailure(result.error)
  }
  await whenSessionViewAt(repositoryPath, result.data)
}
