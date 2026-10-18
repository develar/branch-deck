import type { PatronusPushMode, PullRequest } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { dropsRemoteCommits, getPushAction } from "@/lib/push-summary"
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
 * What Push with Children of the root of a stack that forks does with the stack branch of its whole stack
 * (`getStackBranchAlong`): `along` pushes it in the same atomic push, after the branches, so the pull request of the
 * whole stack follows them; any other kind leaves it as it is, and says why
 */
export type StackBranchAlong =
  /** Built, on the remote or with its pull request open, its remote lacking commits of it */
  | { readonly kind: "along" }
  /** On the remote as it is */
  | { readonly kind: "upToDate" }
  /** Neither on the remote nor with an open pull request: the pull request or the Safe Push of the whole stack puts it there */
  | { readonly kind: "unpublished" }
  /** Not built by the sync (`getStackBranchBlocker`): never pushed */
  | { readonly kind: "notBuilt"; readonly reason: string }
  /** Its remote branch has commits it lacks: a push would drop them, as a child that asks first in its row */
  | { readonly kind: "dropsRemote" }

/**
 * Whether Push with Children of `root` takes the stack branch of its whole stack along (`StackBranchAlong`): only one
 * the sync built, once it is on the remote or its pull request is open (a stack branch is no branch to publish by
 * itself: its pull request or its Safe Push puts it on the remote), while its remote lacks commits of it and has none
 * it lacks. `bd push <root> --with-children` follows the same rule. Null for a stack that doesn't fork, which has none.
 */
export function getStackBranchAlong(root: string, stack: StackBranch | undefined, pullRequest: PullRequest | undefined): StackBranchAlong | null {
  if (stack === undefined || stack.members.length === 0) {
    return null
  }
  const remote = stack.remoteStatus
  if (remote?.exists !== true && pullRequest?.state !== "open") {
    return { kind: "unpublished" }
  }
  const reason = getStackBranchBlocker(root, stack)
  if (reason !== null) {
    return { kind: "notBuilt", reason }
  }
  // the remote not compared yet, with its pull request open: leased as absent, a remote branch that is there is fetched
  if (remote === null) {
    return { kind: "along" }
  }
  if (getPushAction({ remoteStatus: remote }) === "upToDate") {
    return { kind: "upToDate" }
  }
  return dropsRemoteCommits(remote) ? { kind: "dropsRemote" } : { kind: "along" }
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
