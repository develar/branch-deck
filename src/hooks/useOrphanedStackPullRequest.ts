import { describeOrphanedStackPullRequest, stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { useLandedState } from "@/stores/branch-sync"
import { useStackPullRequest } from "@/stores/pull-requests"

/**
 * Why the pull request of the whole stack of `root` no longer matches it, as Rust decides it (its close among the
 * actions of the stack, what landed the root: `describeOrphanedStackPullRequest`); null while it does
 */
export function useOrphanedStackPullRequest(root: string): string | null {
  return describeOrphanedStackPullRequest(root, useStackPullRequest(root), useRowPart(stackReadinessKey(root), "actions"), useLandedState(root))
}
