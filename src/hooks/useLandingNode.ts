import { useMemo, useSyncExternalStore } from "react"
import { useStore } from "zustand"
import type { StepInputs } from "@/hooks/useBranchNextStep"
import { getStepInputs, stepActionsOf, useStepInputs } from "@/hooks/useBranchNextStep"
import type { LandingNodeKind, NextStep, NextStepView, RunningOperation, WholeStackView } from "@/lib/bindings"
import { formatWholeStack, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart, useRowPart } from "@/model/store"
import type { LandingNode } from "@/presenters/landing-node"
import { presentLandingNode, presentNodeKind } from "@/presenters/landing-node"
import type { FoundMove, RunningMove } from "@/presenters/running-move"
import { findRunningMove, presentOperationMoves } from "@/presenters/running-move"
import {
  branchSyncStore,
  getBranch,
  getStackAncestors,
  getStackBranch,
  getWholeStack,
  isBranchPushing,
  useBranchSyncEntry,
  useBranchSyncField,
  useIsBranchPushing,
  useWholeStack,
} from "@/stores/branch-sync"

/*
 * The node of a row as Rust decides it (`nextSteps` of the view, `domain_policy::landing_node`) in the page's words
 * (`presentLandingNode`): a row reads its part of the view (`useRowPart(key, "landing")`, one kept reference) and what
 * the words read beside it, each a narrow selector; the move that runs on the row (its push too) is laid over it: the
 * moves of the operations of the session (`RepositoryView.operations`: a start or a restart of a robot, the creation of
 * a pull request, a review request, from its admission on).
 */

/** The moves that run: of the operations of the session */
function runningMoves(): readonly RunningMove[] {
  return presentOperationMoves(branchSyncStore.getState().operations)
}

/** The branches a Safe Push of `key` lands beside it: the members of a whole stack, the ancestors of a branch */
function landedBeside(key: string): readonly string[] {
  const root = stackRootOfReadinessKey(key)
  if (root !== null) {
    return getStackBranch(root)?.members ?? []
  }
  return getStackAncestors(key)
}

/** The running move of the row of the subject `key` (a branch, `stack:<root>`) now: its own, or one that lands it */
function runningMoveNow(key: string): FoundMove | null {
  const moves = runningMoves()
  return moves.length === 0 ? null : findRunningMove(key, moves, landedBeside)
}

/** A push of the branch `key` runs; never of a whole stack, whose node asks for no push */
function pushingNow(key: string): boolean {
  return stackRootOfReadinessKey(key) === null && isBranchPushing(key)
}

/** The step a node words as its slot says it (`NodeLabel.step`); undefined for a node with words of its own */
function wordedStep(view: NextStepView | undefined): NextStep | undefined {
  const label = view?.node.label
  return label?.kind === "step" ? label.step : undefined
}

/** What the words of a node read beside the node */
interface NodeInputs {
  /** What the words of the step it says read (`useStepInputs`) */
  readonly step: StepInputs
  /** The status text of the row: why the sync failed where Rust names no problem */
  readonly statusText: string
  readonly move: FoundMove | null
  readonly pushing: boolean
}

/** The node of the subject `key` of its part of the view in the page's words, with the move that runs on its row */
function presentNode(key: string, view: NextStepView, inputs: NodeInputs): LandingNode {
  const root = stackRootOfReadinessKey(key)
  return presentLandingNode(view.node, {
    step: {
      subject: root === null ? key : formatWholeStack(root),
      target: inputs.step.target,
      startDescription: null,
      busy: null,
      forStart: false,
      actions: stepActionsOf(key, inputs.step, { openPullRequest: () => {}, retarget: () => {}, createPullRequest: null }),
    },
    statusText: inputs.statusText,
    move: inputs.move,
    pushing: inputs.pushing,
  })
}

/** The node of the subject `key` now (a branch, `stack:<root>`), in its words; null before the view has it */
export function getLandingNodeNow(key: string): LandingNode | null {
  const view = getRowPart(key, "landing")
  if (view === undefined) {
    return null
  }
  return presentNode(key, view, {
    step: getStepInputs(key, wordedStep(view)),
    statusText: getBranch(key)?.statusText ?? "",
    move: runningMoveNow(key),
    pushing: pushingNow(key),
  })
}

/**
 * The kind of the node of the subject `key` now, as its row shows it (`presentNodeKind`), without its words: what the
 * filter chips, F2 and the states of a stack count; null before the view has it
 */
export function getNodeKindNow(key: string): LandingNodeKind | null {
  const view = getRowPart(key, "landing")
  return view === undefined ? null : presentNodeKind(view.node, runningMoveNow(key), pushingNow(key))
}

/**
 * The subject whose node is the node of the whole stack `whole` of `root`: its stack branch (`stack:<root>`) while the
 * stack forks, else the last branch of its straight chain that is not parked, whose Safe Push lands the whole stack;
 * null for a branch that heads no stack
 */
function wholeStackNodeKey(root: string, whole: WholeStackView | null): string | null {
  if (whole === null) {
    return null
  }
  return whole.kind === "chain" ? whole.leaf : stackReadinessKey(root)
}

/**
 * The last branch of the straight chain of `root` that is not parked, whose Safe Push lands the whole stack
 * (`WholeStackView.chain`); null for a lone branch and a stack that forks
 */
export function getStackNodeBranch(root: string): string | null {
  const whole = getWholeStack(root)
  return whole?.kind === "chain" ? whole.leaf : null
}

/** The node of the whole stack of `root` now (`wholeStackNodeKey`): the header of its island, a unit of the overview */
export function getStackLandingNodeNow(root: string): LandingNode | null {
  const key = wholeStackNodeKey(root, getWholeStack(root))
  return key === null ? null : getLandingNodeNow(key)
}

/** The kind of the node of the whole stack of `root` now: `getStackLandingNodeNow` without its words */
export function getStackNodeKindNow(root: string): LandingNodeKind | null {
  const key = wholeStackNodeKey(root, getWholeStack(root))
  return key === null ? null : getNodeKindNow(key)
}

/**
 * Renders again with `select` when a store a node reads changes: the view with the operations of the session. For what
 * reads the nodes of many rows: the filter chips, the states of a stack, the overview
 */
export function useNodeStores<T>(select: () => T): T {
  return useStore(branchSyncStore, select)
}

/** The states of the stores a node reads (`useNodeStores`), for what keeps the nodes of many rows it computed */
export function nodeSources(): readonly unknown[] {
  return [branchSyncStore.getState()]
}

/** No subscription: what a hook reads is the same until another hook of its component renders it again */
function subscribeToNothing(): () => void {
  return () => {}
}

/** Whether an operation of the session has a move (`RunningOperation.moves`) */
function hasMoves(operations: Readonly<Record<string, RunningOperation>>): boolean {
  return Object.values(operations).some(operation => operation.moves.length > 0)
}

/**
 * The running move of the row of `key` (`runningMoveNow`), re-rendering only when it changes: a selector of its text.
 * While no operation of the session moves, the row has none whatever else the store holds: the hook waits for the
 * operations alone, and reads the rest of the store only while a move runs
 */
function useRunningMove(key: string): FoundMove | null {
  const moving = useBranchSyncField("operations", hasMoves)
  const read = () => {
    const move = key === "" ? null : runningMoveNow(key)
    return move === null ? "" : JSON.stringify(move)
  }
  const encoded = useSyncExternalStore(moving ? branchSyncStore.subscribe : subscribeToNothing, read, read)
  return useMemo(() => (encoded === "" ? null : (JSON.parse(encoded) as FoundMove)), [encoded])
}

/** The node of the subject `key` ("" for none), re-rendering when its part of the view or what its words read changes */
function useNodeOf(key: string): LandingNode | null {
  const view = useRowPart(key, "landing")
  const step = useStepInputs(key, wordedStep(view))
  const statusText = useBranchSyncEntry("branches", key, branch => branch?.statusText ?? "")
  const move = useRunningMove(key)
  const pushing = useIsBranchPushing(key) && stackRootOfReadinessKey(key) === null
  return view === undefined ? null : presentNode(key, view, { step, statusText, move, pushing })
}

/**
 * The node of a branch in the table, re-rendering when it changes; a surface of the readiness of the branch as its
 * slot is (`useShowReviewReadiness`)
 */
export function useLandingNode(name: string): LandingNode | null {
  useShowReviewReadiness(name)
  return useNodeOf(name)
}

/** The node of the whole stack of `root`, for the header of its island (`wholeStackNodeKey`) */
export function useStackLandingNode(root: string): LandingNode | null {
  return useNodeOf(wholeStackNodeKey(root, useWholeStack(root)) ?? "")
}

/** `getStackNodeBranch`, re-rendering when it changes */
export function useStackNodeBranch(root: string): string | null {
  const whole = useWholeStack(root)
  return whole?.kind === "chain" ? whole.leaf : null
}
