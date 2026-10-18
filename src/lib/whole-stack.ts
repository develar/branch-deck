import type { PullRequest } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { joinNames } from "@/lib/text"
import type { StackBranch } from "@/stores/branch-sync"

/**
 * "the whole air stack": how a sentence names the whole stack of a root, its stack branch `{prefix}/stack/<root>` and
 * its one robot, as `bd` names it (`RobotSource::subject`)
 */
export function formatWholeStack(root: string): string {
  return `the whole ${root} stack`
}

/** "Whole air stack": the whole stack at the start of a sentence ("Whole air stack merged into master") */
export function formatWholeStackStart(root: string): string {
  return `Whole ${root} stack`
}

/**
 * Why the stack branch of `root` can't be pushed or landed now, null while the sync built it (`Built`): only a built
 * stack branch has the commits of the stack as they are; the stack then lands in pieces
 */
export function getStackBranchBlocker(root: string, stack: StackBranch | undefined): string | null {
  if (stack === undefined) {
    return `The ${root} stack has no stack branch: only a stack that forks builds one`
  }
  switch (stack.state) {
    case "Built":
      return stack.tip === null ? "The stack branch is not built yet: sync first" : null
    case "Blocked":
      return stack.blockedBy === null ? "A branch of the stack is blocked: fix it first, then sync" : `Blocked by ${stack.blockedBy.member}: fix it first, then sync`
    case "Conflict": {
      const conflict = stack.conflict
      const files = conflict === null || conflict.files.length === 0 ? "" : ` in ${joinNames(conflict.files)}`
      const at = conflict === null ? "" : ` at ${formatShortHash(conflict.commit)}`
      return `The stack branch conflicts${at}${files}: the stack lands in pieces`
    }
    case "Stale":
      return stack.error ?? "The stack branch is not up to date: sync first"
  }
}

/**
 * Why an open pull request of the whole stack no longer matches it ("#12 no longer matches the air stack: auth
 * landed"), null while it does: a Safe Push of a branch landed the root in a piece (`landedBy`, as the app saw the
 * merge), or the stack branch is kept only for its remote branch (`members` empty: the stack no longer forks). The
 * pull request shows a stack that is no more; it is never closed by itself, the user decides ("Close Pull Request #12").
 */
export function describeOrphanedStackPullRequest(root: string, pullRequest: PullRequest | undefined, stack: StackBranch | undefined, landedBy: string | null): string | null {
  if (pullRequest?.state !== "open") {
    return null
  }
  if (landedBy !== null) {
    return `#${pullRequest.number} no longer matches the ${root} stack: ${landedBy} landed`
  }
  return stack === undefined || stack.members.length === 0 ? `#${pullRequest.number} no longer matches the ${root} stack: it no longer forks` : null
}
