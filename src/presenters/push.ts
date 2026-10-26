// The pushes of several branches and Push Up to Here in words: what the view says a push up to a commit sends and why
// it can't (`BranchPush.upTo`, `domain_policy::push::up_to_push`), a line per branch of a push of several. Rust decides
// which commits offer it, what goes first and what stops it; the press is the intent `Push` with the scope `upTo`
// (`pushUpTo` of `src/lib/push-run.ts`), which Rust plans again on the facts of its run.
import type { Reason } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatForcePushPart } from "@/lib/remote-status"
import { formatReason } from "@/presenters/reasons"
import type { Branch, BranchSyncState } from "@/stores/branch-sync"
import { pushOrdersIn } from "@/stores/branch-sync"

/** One push of a Push Up to Here, root first */
export interface PushUpToStep {
  readonly name: string
  /** The copy of the commit it pushes instead of the tip; undefined pushes the whole branch */
  readonly tip?: string
  /** The commits the push sends, of `total` the branch has for the remote */
  readonly sent: number
  readonly total: number
}

/** What Push Up to Here of a commit pushes, or why it can't and the branch that stands in the way (`show`) */
export type PushUpTo = { readonly ok: true; readonly steps: readonly PushUpToStep[] } | { readonly ok: false; readonly reason: string; readonly show?: string }

const NO_COMMITS: readonly string[] = []

/**
 * The commits of `name` that offer Push Up to Here as the view has it: those it names, else every one the remote lacks;
 * a list of the state (or one constant), so a selector of it renders again only when it changes
 */
export function pushUpToCommitsIn(state: BranchSyncState, name: string): readonly string[] {
  const remote = state.branches[name]?.remoteStatus
  return remote == null ? NO_COMMITS : (state.pushes[name]?.upTo?.commits ?? remote.unpushedCommits)
}

/** The branch a reason that stops a Push Up to Here names, which its popover reveals */
function namedBranch(reason: Reason): string | undefined {
  switch (reason.code) {
    case "parked":
      return reason.with ?? undefined
    case "held":
      return reason.hold.kind === "span" ? reason.hold.by.branch : reason.branch
    case "cantPush":
    case "dropsRemote":
      return reason.branch
    default:
      return undefined
  }
}

/**
 * Push Up to Here of the commit `hash` (its copy) of the branch `name` as the view has it (`BranchPush.upTo`): the
 * branches it pushes first, whole, root first (those of Push Stack unless the view names others), then the branch up to
 * the commit, the last one the remote lacks pushing it whole; or why it can't, in the words of the reason. Null for a
 * commit that offers none: the remote has it, or a merged Safe Push landed it
 */
export function pushUpToIn(state: BranchSyncState, name: string, hash: string): PushUpTo | null {
  const remote = state.branches[name]?.remoteStatus
  if (remote == null || !pushUpToCommitsIn(state, name).includes(hash)) {
    return null
  }
  const upTo = state.pushes[name]?.upTo
  if (upTo?.blocker != null) {
    const show = namedBranch(upTo.blocker)
    const reason = formatReason(upTo.blocker, branch => state.branches[branch]?.statusText ?? "")
    return show === undefined ? { ok: false, reason } : { ok: false, reason, show }
  }
  const ancestors = upTo?.ancestors ?? pushOrdersIn(state, name).stack.slice(0, -1)
  const steps: PushUpToStep[] = ancestors.map(ancestor => {
    const total = state.branches[ancestor]?.remoteStatus?.unpushedCommits.length ?? 0
    return { name: ancestor, sent: total, total }
  })
  const at = remote.unpushedCommits.indexOf(hash)
  const total = remote.unpushedCommits.length
  // the last commit the remote lacks is the tip: the branch's own push
  steps.push(at === total - 1 ? { name, sent: total, total } : { name, tip: hash, sent: at + 1, total })
  return { ok: true, steps }
}

/**
 * One line of a push of several branches, in the words of a push: "login · 2 commits · creates the remote branch",
 * "auth · 1 commit · force push" for a rebased one, with what a force push replaces or drops (`formatForcePushPart`);
 * "login · 2 of 5 commits" for a part of the branch (`sent` of its commits for the remote)
 */
export function formatStackPushLine(branch: Branch, sent: number | null = null): string {
  const action = branch.remoteStatus?.action.kind
  const parts = [branch.name]
  const ahead = branch.remoteStatus?.commitsAhead ?? 0
  if (sent !== null && sent < ahead) {
    parts.push(`${sent} of ${formatCommitCount(ahead)}`)
  } else if (ahead > 0) {
    parts.push(formatCommitCount(ahead))
  }
  if (action === "create") {
    parts.push("creates the remote branch")
  } else if (action === "force" && branch.remoteStatus != null) {
    parts.push(formatForcePushPart(branch.remoteStatus, branch.stack?.effectiveBase ?? null))
  }
  return parts.join(" · ")
}

/** "Pushes 3 branches, parents first:" and a line per push (`formatStackPushLine`); "Pushes login · 2 of 5 commits" alone */
export function describePushUpTo(steps: readonly PushUpToStep[], branchOf: (name: string) => Branch | undefined): string {
  const lines = steps.flatMap(step => {
    const branch = branchOf(step.name)
    return branch === undefined ? [] : [formatStackPushLine(branch, step.sent)]
  })
  return steps.length === 1 ? `Pushes ${lines[0] ?? steps[0]!.name}` : `Pushes ${steps.length} branches, parents first:\n${lines.join("\n")}`
}
