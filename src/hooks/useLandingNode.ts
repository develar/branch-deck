import { useStore } from "zustand"
import { getStepInputs, stepActionsOf } from "@/hooks/useBranchNextStep"
import type { NextStepView } from "@/lib/bindings"
import type { FoundMove, RunningMove } from "@/lib/running-move"
import { findRunningMove, safePushStartMoves } from "@/lib/running-move"
import { formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart } from "@/model/store"
import type { LandingNode } from "@/presenters/landing-node"
import { presentLandingNode } from "@/presenters/landing-node"
import { branchSyncStore, getBranch, getStackAncestors, getStackBranch, getWholeStack, isBranchPushing } from "@/stores/branch-sync"
import { getOperationMoves, operationsStore } from "@/stores/operations"
import { getSafePushStarts, safePushStore } from "@/stores/safe-push"

/*
 * The node of a row as Rust decides it (`nextSteps` of the view, `domain_policy::landing_node`) in the page's words
 * (`presentLandingNode`), with the move that runs on the row laid over it (its push too): the page's local state until
 * the operations move into Rust (s10).
 */

/** The moves that run: of the operations that declare one, and of the starts of a Safe Push or a Dry Run */
function runningMoves(): readonly RunningMove[] {
  return [...getOperationMoves(), ...safePushStartMoves(getSafePushStarts())]
}

/** The branches a Safe Push of `key` lands beside it: the members of a whole stack, the ancestors of a branch */
function landedBeside(key: string): readonly string[] {
  const root = stackRootOfReadinessKey(key)
  if (root !== null) {
    return getStackBranch(root)?.members ?? []
  }
  return getStackAncestors(key)
}

/** The node of the subject `key` (a branch, `stack:<root>`) of its part of the view now, with the move that runs on its row */
function nodeOf(key: string, view: NextStepView): LandingNode {
  const root = stackRootOfReadinessKey(key)
  const inputs = getStepInputs(key, view.step)
  const move: FoundMove | null = findRunningMove(key, runningMoves(), landedBeside)
  return presentLandingNode(view.node, {
    step: {
      subject: root === null ? key : formatWholeStack(root),
      target: inputs.target,
      startDescription: null,
      busy: null,
      forStart: false,
      actions: stepActionsOf(key, inputs, { openPullRequest: () => {}, retarget: () => {}, createPullRequest: null }),
    },
    statusText: root === null ? (getBranch(key)?.statusText ?? "") : "",
    move,
    pushing: root === null && isBranchPushing(key),
  })
}

/**
 * The states of the stores a node reads: the view (its parts, the rows, the pushes that run), the operations and the
 * starts that run (the move)
 */
export function nodeSources(): readonly unknown[] {
  return [branchSyncStore.getState(), operationsStore.getState(), safePushStore.getState()]
}

/** By key (a branch, `stack:<root>`): the node shown, and the store states it was computed for */
const shownNodes = new Map<string, { sources: readonly unknown[]; node: LandingNode | null }>()

/** Computed once per change of the stores the node reads; the same object while the node stays */
function cachedNode(key: string, compute: () => LandingNode | null): LandingNode | null {
  const sources = nodeSources()
  const shown = shownNodes.get(key)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.node
  }
  const node = compute()
  const same = shown?.node != null && node !== null && JSON.stringify(shown.node) === JSON.stringify(node)
  const kept = same || (shown?.node === null && node === null) ? shown!.node : node
  shownNodes.set(key, { sources, node: kept })
  return kept
}

/** The node of the branch now; null before the view has it */
export function getLandingNodeNow(name: string): LandingNode | null {
  return cachedNode(name, () => {
    const view = getRowPart(name, "landing")
    return view === undefined ? null : nodeOf(name, view)
  })
}

/**
 * The last branch of the straight chain of `root` that is not parked, whose Safe Push lands the whole stack
 * (`WholeStackView.chain`); null for a lone branch and a stack that forks
 */
export function getStackNodeBranch(root: string): string | null {
  const whole = getWholeStack(root)
  return whole?.kind === "chain" ? whole.leaf : null
}

/**
 * The node of the whole stack of `root` now (the header of its island): of its stack branch while the stack forks, else
 * of the last branch of its straight chain, whose Safe Push lands the whole stack; null for a branch that heads no stack
 */
export function getStackLandingNodeNow(root: string): LandingNode | null {
  const leaf = getStackNodeBranch(root)
  if (leaf !== null) {
    return getLandingNodeNow(leaf)
  }
  const key = stackReadinessKey(root)
  return cachedNode(key, () => {
    const view = getWholeStack(root)?.kind === "forking" ? getRowPart(key, "landing") : undefined
    return view === undefined ? null : nodeOf(key, view)
  })
}

/** Renders again with `select` when a store the node reads changes */
export function useNodeStores<T>(select: () => T): T {
  useStore(branchSyncStore, select)
  useStore(safePushStore, select)
  return useStore(operationsStore, select)
}

/**
 * The node of a branch in the table, re-rendering when it changes; a surface of the readiness of the branch as its
 * slot is (`useShowReviewReadiness`)
 */
export function useLandingNode(name: string): LandingNode | null {
  useShowReviewReadiness(name)
  return useNodeStores(() => getLandingNodeNow(name))
}

/** The node of the whole stack of `root`, for the header of its island */
export function useStackLandingNode(root: string): LandingNode | null {
  return useNodeStores(() => getStackLandingNodeNow(root))
}

/** `getStackNodeBranch`, re-rendering when it changes */
export function useStackNodeBranch(root: string): string | null {
  return useStore(branchSyncStore, () => getStackNodeBranch(root))
}
