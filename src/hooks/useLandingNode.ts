import { getNextStepFacts, nextStepSources, useNextStepStores } from "@/hooks/useBranchNextStep"
import { isForkingStack, stackFactsOf } from "@/hooks/useStackNextStep"
import type { LandingNode, LandingNodeFacts } from "@/lib/landing-node"
import { getLandingNode } from "@/lib/landing-node"
import { formatLandingSubject } from "@/lib/patronus"
import { getPushAction } from "@/lib/push-summary"
import { blockingBranchOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getStackBranch, stackMainLineIn } from "@/stores/branch-sync"
import { getBranchRobot, getLandingSpan, getStackRobot, isPatronusKnown } from "@/stores/patronus"
import { useShowReviewReadiness } from "@/stores/review-readiness"

/** What decides the node of a branch now, from the stores */
function landingFactsOf(branch: Branch): LandingNodeFacts {
  const landing = getLandingSpan(branch.name)
  return {
    next: getNextStepFacts(branch),
    status: branch.status,
    statusText: branch.statusText,
    blockedBy: blockingBranchOf(branch),
    // a dry run checks the branches of its span, it lands none
    landingWith: landing === null || landing.robot.pushMode === "dryRun" ? null : formatLandingSubject(landing),
    robotDryRun: getBranchRobot(branch.name)?.pushMode === "dryRun",
    patronusKnown: isPatronusKnown(),
    pushPending: isPushPending(branch),
  }
}

/** The remote lacks work of the branch */
function isPushPending(branch: Branch): boolean {
  const action = getPushAction(branch)
  return action !== null && action !== "upToDate"
}

/**
 * The node of the whole stack of `root` (the header of its island): of its stack branch while the stack forks, else of
 * the last branch of its straight chain, whose Safe Push lands the whole stack; null for a branch that heads no stack
 */
function computeStackNode(root: string): LandingNode | null {
  const stack = getStackBranch(root)
  if (isForkingStack(stack)) {
    return getLandingNode({
      next: stackFactsOf(stack),
      status: "Unchanged",
      statusText: "",
      blockedBy: null,
      landingWith: null,
      robotDryRun: getStackRobot(root)?.pushMode === "dryRun",
      patronusKnown: isPatronusKnown(),
      pushPending: false,
    })
  }
  const leaf = mainLineLeaf(root)
  return leaf === null ? null : getLandingNodeNow(leaf)
}

/** The last branch of the straight chain of `root`, whose Safe Push lands the whole stack; null for a lone branch */
function mainLineLeaf(root: string): string | null {
  const mainLine = stackMainLineIn(branchSyncStore.getState(), root)
  return mainLine.length < 2 ? null : (mainLine.at(-1) ?? null)
}

/**
 * The branch whose node the header of the island of `root` shows (the last of its straight chain), whose robot its hover
 * shows; null while the stack forks: the node is of its stack branch, the robot of the whole stack
 */
export function getStackNodeBranch(root: string): string | null {
  return isForkingStack(getStackBranch(root)) ? null : mainLineLeaf(root)
}

/** By key (a branch, `stack:<root>`): the node shown, and the store states it was computed for */
const shownNodes = new Map<string, { sources: readonly unknown[]; node: LandingNode | null }>()

/** Computed once per change of the stores the next step reads; the same object while the node stays */
function cachedNode(key: string, compute: () => LandingNode | null): LandingNode | null {
  const sources = nextStepSources()
  const shown = shownNodes.get(key)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.node
  }
  const node = compute()
  const kept = shown !== undefined && shown.node?.kind === node?.kind && shown.node?.label === node?.label ? shown.node : node
  shownNodes.set(key, { sources, node: kept })
  return kept
}

/** The node of the branch now (`getLandingNode`); null without the branch */
export function getLandingNodeNow(name: string): LandingNode | null {
  return cachedNode(name, () => {
    const branch = getBranch(name)
    return branch === undefined ? null : getLandingNode(landingFactsOf(branch))
  })
}

/** The node of the whole stack of `root` now; null for a branch that heads no stack */
export function getStackLandingNodeNow(root: string): LandingNode | null {
  return cachedNode(`stack:${root}`, () => computeStackNode(root))
}

/**
 * The node of a branch in the table, re-rendering when it changes; a surface of the readiness of the branch as its
 * slot is (`useShowReviewReadiness`)
 */
export function useLandingNode(name: string): LandingNode | null {
  useShowReviewReadiness(name)
  return useNextStepStores(() => getLandingNodeNow(name))
}

/** The node of the whole stack of `root`, for the header of its island */
export function useStackLandingNode(root: string): LandingNode | null {
  return useNextStepStores(() => getStackLandingNodeNow(root))
}

/** `getStackNodeBranch`, re-rendering when it changes */
export function useStackNodeBranch(root: string): string | null {
  return useNextStepStores(() => getStackNodeBranch(root))
}
