import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"

interface LandedStacksState {
  /** By repository and branch prefix, then by the root of a stack: the branch whose merged Safe Push landed the root */
  readonly landedBy: Readonly<Record<string, Readonly<Record<string, string>>>>
}

/**
 * The roots of the stacks a Safe Push of a branch landed in a piece, as the app saw the merge: the pull request of the
 * whole stack then no longer matches it, and says why ("#12 no longer matches the air stack: auth landed",
 * `describeOrphanedStackPullRequest`). In memory only: after a restart the reason is told without the branch.
 */
export const landedStacksStore = createStore<LandedStacksState>(() => ({ landedBy: {} }))

function keyOf(repositoryPath: string, branchPrefix: string): string {
  return `${repositoryPath}\n${branchPrefix}`
}

function currentKey(): string {
  return keyOf(getSelectedProject()?.path ?? "", getEffectiveBranchPrefix())
}

/** The merged Safe Push of `branch` landed `root`, the root of a stack whose whole stack has a pull request */
export function rememberLandedStackRoot(root: string, branch: string): void {
  const key = currentKey()
  landedStacksStore.setState(state => ({ landedBy: { ...state.landedBy, [key]: { ...state.landedBy[key], [root]: branch } } }))
}

/** A new pull request of the whole stack of `root` matches the stack as it is: what landed before is no reason any more */
export function forgetLandedStackRoot(root: string, repositoryPath: string, branchPrefix: string): void {
  // the repository the pull request was created in, also after a switch: the root of the same name there is another stack
  const key = keyOf(repositoryPath, branchPrefix)
  if (landedStacksStore.getState().landedBy[key]?.[root] === undefined) {
    return
  }
  landedStacksStore.setState(state => {
    const { [root]: _, ...rest } = state.landedBy[key] ?? {}
    return { landedBy: { ...state.landedBy, [key]: rest } }
  })
}

/** The branch whose merged Safe Push landed the root of the stack, null when the app didn't see one */
export function useStackLandedBy(root: string): string | null {
  const key = keyOf(useSelectedProjectPath() ?? "", useEffectiveBranchPrefix())
  return useStore(landedStacksStore, state => state.landedBy[key]?.[root] ?? null)
}

/** Non-hook variant of `useStackLandedBy`, for menus built on open */
export function getStackLandedBy(root: string): string | null {
  return landedStacksStore.getState().landedBy[currentKey()]?.[root] ?? null
}

/** Forgets the landings (tests) */
export function resetLandedStacks(): void {
  landedStacksStore.setState({ landedBy: {} })
}
