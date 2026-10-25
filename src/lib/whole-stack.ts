import type { PatronusPushMode, PullRequest, StackBranchConflict } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
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

/** The branches of a whole stack merged Safe Pushes landed already (`names`), and who landed them (`by`, "deploy") */
export interface StackLandedPart {
  readonly names: readonly string[]
  readonly by: readonly string[]
}

/**
 * What one robot of the whole stack lands, the hover of its start and the title of its popover: "Lands 7 branches · 14
 * commits into master in one Safe Push: air, bazel, … side-fix and side-test"; a Dry Run checks them: "Checks 7 branches
 * · 14 commits on master in one Dry Run: …". After merged Safe Pushes landed branches of it (`landed`), what is left of
 * it, and who landed the others: "Lands 2 branches · 3 commits into master in one Safe Push: side-fix and side-test;
 * air, bazel and deploy landed with deploy"
 */
export function describeWholeStackLanding(members: readonly string[], commitCount: number, target: string, mode: PatronusPushMode, landed: StackLandedPart | null = null): string {
  const size = formatBranchesAndCommits(members.length, commitCount)
  const names = formatWholeStackMembers(members)
  const rest = landed === null || landed.names.length === 0 ? "" : `; ${formatWholeStackMembers(landed.names)} landed with ${joinNames(landed.by)}`
  return mode === "dryRun" ? `Checks ${size} on ${target} in one Dry Run: ${names}${rest}` : `Lands ${size} into ${target} in one Safe Push: ${names}${rest}`
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

/** "The stack branch conflicts at 01234567 in a.kt: the stack lands in pieces": the commit and its files, when known */
export function formatStackBranchConflict(conflict: StackBranchConflict | null): string {
  const files = conflict === null || conflict.files.length === 0 ? "" : ` in ${joinNames(conflict.files)}`
  const at = conflict === null ? "" : ` at ${formatShortHash(conflict.commit)}`
  return `The stack branch conflicts${at}${files}: the stack lands in pieces`
}

/**
 * "user-name/stack/air · the whole stack branch · 2 commits": the stack branch as a line of what a stack push sends, as
 * `formatStackPushLine` says a branch
 */
export function formatStackBranchPushLine(stack: StackBranch): string {
  const parts = [stack.branchName, "the whole stack branch"]
  const remote = stack.remoteStatus
  if (remote !== null && remote.commitsAhead > 0) {
    parts.push(formatCommitCount(remote.commitsAhead))
  }
  if (remote?.exists !== true) {
    parts.push("creates the remote branch")
  }
  return parts.join(" · ")
}

/**
 * "Also pushes the whole stack branch user-name/stack/air, the head of #12": what Push with Children of the root adds,
 * the hover of its menu item
 */
export function describeStackBranchAlong(stack: StackBranch, pullRequest: PullRequest | undefined): string {
  const head = pullRequest?.state === "open" ? `, the head of #${pullRequest.number}` : ""
  return `Also pushes the whole stack branch ${stack.branchName}${head}`
}

/**
 * "Pushes only its stack branch user-name/stack/air: the branches of the stack stay as they are on the remote.": what
 * the start of the whole stack pushes, and the members it leaves on the remote, said only of those that are there
 * ("auth stays as it is on the remote."); nothing of them while none is
 */
export function describeStackBranchPush(stackBranch: string, members: readonly string[], onRemote: readonly string[]): string {
  const pushes = `Pushes only its stack branch ${stackBranch}`
  if (onRemote.length === 0) {
    return `${pushes}.`
  }
  if (onRemote.length === members.length) {
    return `${pushes}: the branches of the stack stay as they are on the remote.`
  }
  return `${pushes}: ${joinNames(onRemote)} ${onRemote.length === 1 ? "stays as it is" : "stay as they are"} on the remote.`
}

/** "the whole stack branch of air": the stack branch of the whole stack of a root, as the push of it names it */
export function formatWholeStackBranch(root: string): string {
  return `the whole stack branch of ${root}`
}

/**
 * "Pushing the whole stack": the name of the push of the stack branch alone (`pushWholeStack`: a step of the restart of
 * its robot, of Request Review… and of the creation of its pull request) where the pushes of the stack show, the push of
 * the island's header and of the root's row of a small stack, with its percent ("Pushing the whole stack 45%")
 */
export const STACK_BRANCH_PUSHING = "Pushing the whole stack"

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
