import type { PatronusPushMode, PullRequest } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
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
 * "air, bazel, … side-fix and side-test": the branches of a whole stack, root first, its first two and its last two
 * (the side branches come last, and they are what only the whole stack lands); all of them up to five
 */
export function formatWholeStackMembers(members: readonly string[]): string {
  if (members.length <= 5) {
    return members.length <= 1 ? members.join("") : `${members.slice(0, -1).join(", ")} and ${members.at(-1)}`
  }
  return `${members.slice(0, 2).join(", ")}, … ${members.at(-2)} and ${members.at(-1)}`
}

/**
 * What one robot of the whole stack lands, the hover of its start and the title of its popover: "Lands 7 branches · 14
 * commits into master in one Safe Push: air, bazel, … side-fix and side-test"; a Dry Run checks them: "Checks 7 branches
 * · 14 commits on master in one Dry Run: …"
 */
export function describeWholeStackLanding(members: readonly string[], commitCount: number, target: string, mode: PatronusPushMode): string {
  const size = formatBranchesAndCommits(members.length, commitCount)
  const names = formatWholeStackMembers(members)
  return mode === "dryRun" ? `Checks ${size} on ${target} in one Dry Run: ${names}` : `Lands ${size} into ${target} in one Safe Push: ${names}`
}

/** The prefix of the readiness key of a whole stack: a branch name has no ":" (git refuses it in a ref) */
const STACK_READINESS_PREFIX = "stack:"

/**
 * "stack:air": the key of the review readiness of the whole stack of `root` among those of the branches, asked as its
 * stack branch with the pull request of the whole stack (`ReviewReadinessBranch::source`)
 */
export function stackReadinessKey(root: string): string {
  return `${STACK_READINESS_PREFIX}${root}`
}

/** The root of a readiness key of a whole stack (`stackReadinessKey`); null for a branch's */
export function stackRootOfReadinessKey(key: string): string | null {
  return key.startsWith(STACK_READINESS_PREFIX) ? key.slice(STACK_READINESS_PREFIX.length) : null
}

/** What a sentence calls the subject of a readiness key: "the whole air stack", or the branch's name */
export function formatReadinessSubject(key: string): string {
  const root = stackRootOfReadinessKey(key)
  return root === null ? key : formatWholeStack(root)
}

/**
 * "Approvals count in #12, the pull request of the whole stack": Patronus counts only the approvals of the pull request
 * whose comment started the robot, so those of the whole stack count for its Safe Push alone
 */
export function formatWholeStackApprovals(pullRequest: number): string {
  return `Approvals count in #${pullRequest}, the pull request of the whole stack`
}

/**
 * Why the approvals of the open pull request of the whole stack don't help the Safe Push of a piece of it (a branch and
 * what it is built on): that one starts from the branch's own pull request, or through the API
 */
export function describeWholeStackApprovalsForPiece(root: string, pullRequest: number, branchName: string): string {
  return `#${pullRequest}, the pull request of ${formatWholeStack(root)}, is open: its approvals count only for the Safe Push of the whole stack, not for this Safe Push of ${branchName}`
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
