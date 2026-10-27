// The hooks of the pushes of a branch: what the view orders them to send (`RepositoryView.pushes`), whether Rust lets
// the push run (its `actions`), and the push button they render, in the words of `src/presenters/push-button.ts`
import { useMemo } from "react"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { useLandedIn } from "@/hooks/useLandedIn"
import { gitHubRepositoryOf } from "@/lib/pull-request"
import { pushBranch, pushStack, pushWithChildren } from "@/lib/push-run"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import type { PushButtonRuns, PushButtonState, PushRetarget, UnpushedChildren, UnpushedParent } from "@/presenters/push-button"
import { childrenPushEnabled, getPushButtonState, getPushRetargets, landingPushes, parentOf, pushBlockerOf, retargetCandidates } from "@/presenters/push-button"
import type { Branch, ChildrenPushOrder } from "@/stores/branch-sync"
import { pushOrdersIn, useBranchSync, useBranchSyncField, useIsStackBranchPushing } from "@/stores/branch-sync"
import { usePatronusAvailable } from "@/stores/patronus"
import { pullRequestsStore, usePullRequest, usePullRequestAvailability } from "@/stores/pull-requests"
import { useLandRun } from "@/stores/safe-push"

/**
 * Safe Push or Create Pull Request is there to push the branch, and no live pull request shows it: a push of a branch
 * with one updates it (`isFirstPushInLanding`)
 */
export function useLandingPushes(branchName: string): boolean {
  const patronus = usePatronusAvailable()
  const gitHub = gitHubRepositoryOf(usePullRequestAvailability()) !== null
  return landingPushes(patronus, gitHub, usePullRequest(branchName)?.state)
}

/** What the pushes the description of the push button of `name` offers run: the push alone, the stack, the children */
export function pushRunsOf(name: string): PushButtonRuns {
  return { alone: () => void pushBranch(name), stack: () => void pushStack(name), children: () => void pushWithChildren(name) }
}

/** The push button of a branch row and of the branch preview */
export function usePushButton(branch: Branch, isPushing: boolean): PushButtonState {
  const unpushedParent = useUnpushedParent(branch)
  const unpushedChildren = useUnpushedChildren(branch.name)
  const landedIn = useLandedIn(branch.name)
  const actions = useRowPart(branch.name, "actions")
  const retargets = usePushRetargets(retargetCandidates(branch.name, unpushedParent))
  return useMemo(() => {
    const runs = pushRunsOf(branch.name)
    return getPushButtonState(branch, {
      isPushing,
      blocker: pushBlockerOf(actions),
      childrenEnabled: childrenPushEnabled(actions),
      unpushedParent,
      unpushedChildren,
      landedIn,
      retargets,
      runs,
    })
  }, [branch, isPushing, actions, unpushedParent, unpushedChildren, landedIn, retargets])
}

/** The pull requests into a virtual branch a push of `names` retargets first (`getPushRetargets`), from the stores */
function usePushRetargets(names: readonly string[]): readonly PushRetarget[] {
  // the base they go into is the baseline: a new one renders again, and the selector below reads it
  useBranchSyncField("baselineBranch", baseline => baseline)
  // a string, so the store notifies only when a retarget changes
  const retargets = useStore(pullRequestsStore, () => JSON.stringify(getPushRetargets(names)))
  return useMemo(() => JSON.parse(retargets) as PushRetarget[], [retargets])
}

/** The nearest branch a stacked branch is built on that has work for the remote, and the stack pushes of the branch */
export function useUnpushedParent(branch: Branch): UnpushedParent | null {
  const name = branch.name
  // each selector returns branch records (compared one by one) or a number, so the store notifies only when they change
  const parent = useBranchSync(state => parentOf(state, pushOrdersIn(state, name)))
  const stackSize = useBranchSync(state => pushOrdersIn(state, name).stack.length)
  const buttonStack = useBranchSync(useShallow(state => pushOrdersIn(state, name).button.flatMap(it => state.branches[it] ?? [])))
  return useMemo(() => {
    if (parent === undefined) {
      return null
    }
    return { name: parent.name, onRemote: parent.remoteStatus?.exists === true, stackSize, buttonStack }
  }, [parent, stackSize, buttonStack])
}

/**
 * The branches stacked on a branch, while Push with Children of it has more to push than the branch (`ChildrenPush`,
 * as Rust orders it), and of a root the stack branch of its whole stack while it goes along
 */
export function useUnpushedChildren(branchName: string): UnpushedChildren | null {
  // a string, so the store notifies only when the push changes
  const key = useBranchSync(state => {
    const children = pushOrdersIn(state, branchName).children
    if (children === null || !children.offers) {
      return ""
    }
    const stackBranch = children.stackBranch?.kind === "along" ? (state.stackBranches[branchName]?.branchName ?? null) : null
    return JSON.stringify({ names: children.order.filter(name => name !== branchName), pushOrder: children.order, stackBranch } satisfies UnpushedChildren)
  })
  return useMemo(() => (key === "" ? null : (JSON.parse(key) as UnpushedChildren)), [key])
}

/** What Push with Children of a branch sends */
export interface ChildrenPush {
  /** The pushes as Rust orders them (`ChildrenPush`); null with nothing stacked on the branch to push */
  children: ChildrenPushOrder | null
  /** The commits the branches of the push send */
  pushCommits: number
}

/** Push with Children of `name` as the table has it now: the order of Rust and the commits it sends */
export function useChildrenPush(name: string): ChildrenPush {
  // a string, so the store notifies only when the push changes
  const key = useBranchSync(state => {
    const children = pushOrdersIn(state, name).children
    const pushCommits = (children?.order ?? []).reduce((sum, branch) => sum + (state.branches[branch]?.remoteStatus?.commitsAhead ?? 0), 0)
    return JSON.stringify({ children, pushCommits } satisfies ChildrenPush)
  })
  return useMemo(() => JSON.parse(key) as ChildrenPush, [key])
}

/**
 * The stack branch of the whole stack of `root` is being pushed, and not as the push step of the start of its Safe
 * Push, which shows the push in its own button ("Pushing 45%", the header's slot of an island, the root's slot of a
 * small stack): the pushes of the stack show it then (the island's header push, the root's push button), one indicator
 */
export function useStackBranchPushAlone(root: string): boolean {
  const pushing = useIsStackBranchPushing(root)
  const startPushes = useLandRun(stackReadinessKey(root))?.step === "pushing"
  return pushing && !startPushes
}
